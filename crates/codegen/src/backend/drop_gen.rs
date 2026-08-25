//! RAII drop synthesis.
//!
//! Ensures every owned value has a `Drop` implementation registered in
//! `trait_registry` before `scope::drop_variable` emits its call:
//!
//! 1. Explicit user impls (`impl Drop for Foo`, incl. generic
//!    `impl[T] Drop for Box[T]`) are instantiated on first use with the
//!     generics inferred from the concrete struct's fields.
//! 2. Structs without a user Drop get a synthesized drop that recursively
//!    drops owned fields (fields whose own type has a Drop). Borrowed or raw
//!    pointer fields have no Drop and are left alone.
//!
//! With this in place, `Box[T]`/`Vec[T]`/`String` free their memory when the
//! owning variable goes out of scope — no manual `.free()` required.

use std::collections::HashMap;
use std::rc::Rc;

use cranelift::prelude::{types, AbiParam, FunctionBuilder, FunctionBuilderContext, InstBuilder};
use cranelift_module::{Linkage, Module};
use linear_map::LinearMap;
use cranelift_object::ObjectModule;

use mantis_parser::ast::TypeExpr as MsTokenType;
use mantis_parser::token::Span;

use crate::ms::MsContext;
use crate::registries::functions::{FunctionType, MsDeclaredFunction};
use crate::registries::modules::MsResolved;
use crate::registries::structs::{MsStructFieldValue, MsStructType};
use crate::registries::types::{
    MsGenericTemplateInner, MsNativeType, MsType, MsTypeId, MsTypeWithId, TypeNameWithGenerics,
};
use crate::registries::variable::MsVar;

use super::compile_function::instantiate_generic_function;

/// Make sure a Drop impl is registered for `ty_id`. Returns true when the
/// type owns resources (i.e. dropping a value of it does something).
pub fn ensure_drop_registered(
    ty_id: MsTypeId,
    ms_ctx: &mut MsContext,
    module: &mut ObjectModule,
) -> bool {
    if ms_ctx
        .current_module
        .trait_registry
        .get_implementation(ty_id, "Drop")
        .is_some()
    {
        return true;
    }

    let Some(ty) = ms_ctx.current_module.type_registry.get_from_type_id(ty_id) else {
        return false;
    };
    let MsType::Struct(s) = &ty else {
        // Primitives/enums without an explicit Drop own nothing by default.
        return false;
    };

    // Base template name (strip `[...]` from instantiations).
    let Some(full_name) = ms_ctx.current_module.type_registry.name_of(ty_id) else {
        return false;
    };
    let base_name = match full_name.find('[') {
        Some(i) => full_name[..i].to_string(),
        None => full_name.clone(),
    };

    // ── 1. User-written generic Drop impl: infer generics from fields ────
    if let Some(templates) = ms_ctx
        .current_module
        .trait_generic_templates
        .registry
        .get(base_name.as_str())
    {
        let drop_tmpl = templates
            .iter()
            .find(|t| t.decl.name.as_ref().and_then(|n| n.as_name()) == Some("drop"))
            .cloned();
        if let Some(drop_tmpl) = drop_tmpl {
            if let Some(real_types) =
                infer_generics_from_fields(ms_ctx, &base_name, s, &drop_tmpl.generics)
            {
                // Make `Self` resolve while building the instantiated signature.
                ms_ctx.current_module.add_alias(
                    TypeNameWithGenerics::new("Self".into(), vec![]),
                    MsTypeWithId {
                        id: ty_id,
                        ty: ty.clone(),
                    },
                );
                let fn_expr = MsTokenType::Named(mantis_parser::ast::Ident::new(
                    format!("{}Drop", base_name),
                    Span::new(0, 0),
                ));
                let func =
                    instantiate_generic_function(drop_tmpl, real_types, &fn_expr, ms_ctx, module);
                ms_ctx.current_module.trait_registry.add_function(
                    "Drop",
                    ty_id,
                    "drop".into(),
                    func,
                );
                return true;
            }
        }
    }

    // ── 2. Synthesize a field-wise drop ──────────────────────────────────
    let mut field_drops: Vec<(i64, Rc<MsDeclaredFunction>)> = Vec::new();
    let mut field_list: Vec<MsStructFieldValue> = s.field_list();
    field_list.sort_by_key(|f| f.offset);
    for field in field_list.iter() {
        if field.ty == ty_id {
            continue; // self-referential guard
        }
        if ensure_drop_registered(field.ty, ms_ctx, module) {
            if let Some(f) = ms_ctx
                .current_module
                .trait_registry
                .find_method_implementation(field.ty, "Drop", "drop")
            {
                field_drops.push((field.offset as i64, f));
            }
        }
    }
    if field_drops.is_empty() {
        return false; // nothing owned inside
    }

    let func = synthesize_struct_drop(ty_id, &field_drops, module);
    ms_ctx
        .current_module
        .trait_registry
        .add_function("Drop", ty_id, "drop".into(), func.clone());
    ms_ctx
        .current_module
        .fn_registry
        .add_function(format!("__autodrop_{}_{}", base_name, ty_id.0), func);
    true
}

/// Unify a generic struct template's generic parameters against the concrete
/// struct to recover the concrete generic arguments.
///
/// Primary strategy: any generic already aliased in `aliased_types`.
/// Fallback (the common owner shape): `ptr @T` — T is the element type behind
/// the pointer field.
fn infer_generics_from_fields(
    ms_ctx: &mut MsContext,
    base_name: &str,
    concrete: &MsStructType,
    generics: &[Box<str>],
) -> Option<Vec<MsResolved>> {
    let tmpl = ms_ctx
        .current_module
        .type_templates
        .registry
        .get(base_name)?
        .clone();
    match tmpl.inner_type {
        MsGenericTemplateInner::Struct(_) => {}
        _ => return None,
    }

    let mut map: HashMap<Box<str>, MsTypeId> = HashMap::new();

    // 1. Aliases recorded while compiling the surrounding expression.
    for gen_name in generics {
        let key = TypeNameWithGenerics::new(gen_name.clone(), vec![]);
        if let Some(alias) = ms_ctx.current_module.aliased_types.get(&key) {
            map.insert(gen_name.clone(), alias.id);
        }
    }

    // 2. ptr-field heuristic.
    for gen_name in generics {
        if map.contains_key(gen_name) {
            continue;
        }
        let ptr_field = concrete.get_field("ptr")?;
        if let Some(MsType::Ref(inner, _)) = ms_ctx
            .current_module
            .type_registry
            .get_from_type_id(ptr_field.ty)
        {
            let inner_id = ms_ctx
                .current_module
                .type_registry
                .get_id_from_type(&inner)
                .unwrap_or_else(|| {
                    ms_ctx
                        .current_module
                        .type_registry
                        .add_type(format!("__elem_{}", gen_name), (*inner).clone())
                });
            map.insert(gen_name.clone(), inner_id);
        } else {
            return None;
        }
    }

    let real_types = generics
        .iter()
        .map(|g| {
            let id = *map.get(g).expect("filled above");
            let ty = ms_ctx
                .current_module
                .type_registry
                .get_from_type_id(id)
                .unwrap_or(MsType::Native(MsNativeType::I64));
            MsResolved::Type(MsTypeWithId { id, ty })
        })
        .collect();
    Some(real_types)
}

/// Emit a function `fn __autodrop(self @mut T)` calling each field's drop.
fn synthesize_struct_drop(
    ty_id: MsTypeId,
    field_drops: &[(i64, Rc<MsDeclaredFunction>)],
    module: &mut ObjectModule,
) -> Rc<MsDeclaredFunction> {
    let name = format!("__autodrop_{}", ty_id.0);

    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(types::I64)); // @mut Self

    let func_id = module
        .declare_function(&name, Linkage::Local, &sig)
        .expect("declare autodrop");

    let mut ctx = cranelift::codegen::Context::new();
    ctx.func.signature = sig.clone();
    let mut fbx = FunctionBuilderContext::new();
    {
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut fbx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);

        let self_ptr = builder.block_params(entry)[0];
        for (offset, drop_fn) in field_drops {
            let field_addr = builder.ins().iadd_imm(self_ptr, *offset);
            let func_ref = module.declare_func_in_func(drop_fn.func_id, builder.func);
            builder.ins().call(func_ref, &[field_addr]);
        }
        builder.ins().return_(&[]);
        builder.finalize(module.isa().frontend_config());
    }
    module
        .define_function(func_id, &mut ctx)
        .expect("define autodrop");

    let mut args = LinearMap::new();
    args.insert("self".into(), ty_id);
    Rc::new(MsDeclaredFunction {
        arguments: args,
        rets: None,
        fn_type: FunctionType::Public,
        func_id,
    })
}

/// Convenience used by scope.rs: run RAII for one variable.
pub fn drop_var_with_raii(
    var: &MsVar,
    ms_ctx: &mut MsContext,
    fbx: &mut cranelift::prelude::FunctionBuilder,
    module: &mut ObjectModule,
) {
    if var.moved || var.is_reference {
        return;
    }
    if !ensure_drop_registered(var.ty_id, ms_ctx, module) {
        return;
    }
    if let Some(function) = ms_ctx
        .current_module
        .trait_registry
        .find_method_implementation(var.ty_id, "Drop", "drop")
    {
        let func_ref = module.declare_func_in_func(function.func_id, fbx.func);
        let val = if let Some(ss) = var.stack_slot {
            fbx.ins().stack_addr(types::I64, ss, 0)
        } else {
            fbx.use_var(var.c_var)
        };
        fbx.ins().call(func_ref, &[val]);
        log::info!("RAII drop emitted for ty={:?}", var.ty_id);
    }
}

/// Ensure ALL methods of a generic struct instantiation exist for `ty_id`.
///
/// On first touch, instantiates every method template of the base type with
/// generics inferred from the concrete fields, registering results into
/// `type_fn_registry[ty_id]` (and Drop into `trait_registry`). Subsequent
/// receiver-method lookups become plain exact-id registry hits.
pub fn ensure_type_methods(
    ty_id: MsTypeId,
    ms_ctx: &mut MsContext,
    module: &mut ObjectModule,
) -> bool {
    if ms_ctx.current_module.type_fn_registry.map.get(&ty_id).is_some() {
        return true;
    }
    let Some(ty) = ms_ctx.current_module.type_registry.get_from_type_id(ty_id) else {
        return false;
    };
    let MsType::Struct(s) = &ty else { return false; };
    let Some(full_name) = ms_ctx.current_module.type_registry.name_of(ty_id) else {
        return false;
    };
    let base_name = match full_name.find('[') {
        Some(i) => full_name[..i].to_string(),
        None => full_name.clone(),
    };

    let Some(method_templates) = ms_ctx
        .current_module
        .trait_generic_templates
        .registry
        .get(base_name.as_str())
        .cloned()
    else {
        return false;
    };
    if method_templates.is_empty() {
        return false;
    }

    let generics: Vec<Box<str>> = method_templates
        .first()
        .map(|t| t.generics.clone())
        .unwrap_or_default();

    for mt in &method_templates {
        // Re-derive per-method (aliases are cleared inside instantiate).
        if let Some(rts) = infer_generics_from_fields(ms_ctx, &base_name, s, &generics) {
            ms_ctx.current_module.add_alias(
                TypeNameWithGenerics::new("Self".into(), vec![]),
                MsTypeWithId {
                    id: ty_id,
                    ty: ty.clone(),
                },
            );
            for (g, r) in generics.iter().zip(rts.iter()) {
                if let Some(t) = r.ty() {
                    ms_ctx.current_module.add_alias(
                        TypeNameWithGenerics::new(g.clone(), vec![]),
                        t,
                    );
                }
            }
            let fn_expr = MsTokenType::Named(mantis_parser::ast::Ident::new(
                format!("{}Method", base_name),
                Span::new(0, 0),
            ));
            let fname = mt
                .decl
                .name
                .as_ref()
                .and_then(|n| n.as_name())
                .unwrap_or("")
                .to_string();
            let func = instantiate_generic_function(
                mt.clone(),
                rts.clone(),
                &fn_expr,
                ms_ctx,
                module,
            );
            ms_ctx
                .current_module
                .type_fn_registry
                .add_function(ty_id, fname.as_str(), func);
        }
    }
    true
}
