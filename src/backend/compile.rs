use std::{collections::HashMap, path::PathBuf, rc::Rc};

use cranelift::{
    codegen::Context,
    prelude::{settings, types, AbiParam, Configurable, FunctionBuilder, FunctionBuilderContext},
};
use cranelift_module::{default_libcall_names, DataDescription, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};
use mantis_parser::ast::{
    Block, Declaration, FnDecl, Program, TypeDef, TypeDefBody, TypeExpr, ImplBlock, TraitDef,
};

use crate::{
    backend::compile_function::TraitFunctionFor,
    ms::MsContext,
    registries::{
        functions::{MsFunctionRegistry, MsGenericFunction},
        modules::{resolve_module_by_word, MsResolved},
        types::{MsGenericTemplate, MsTypeRegistry, TypeNameWithGenerics},
    },
};

use super::compile_function::{compile_function, random_string};

pub fn resolve_type_term(ty: &TypeExpr, ms_ctx: &MsContext) {
    match ty {
        TypeExpr::Generic(_, _) => todo!(),
        _ => todo!(),
    };
}

pub fn compile_binary(
    program: Program,
    include_dirs: Vec<String>,
    module_name: &str,
    auto_drop: bool,
) -> anyhow::Result<Vec<u8>> {
    let data_description = DataDescription::new();
    let mut flag_builder = settings::builder();
    flag_builder.set("preserve_frame_pointers", "true");
    flag_builder.set("is_pic", "true");
    flag_builder.set("use_colocated_libcalls", "false");

    let isa_builder = cranelift_native::builder().map_err(|x| anyhow::anyhow!(x))?;
    let isa = isa_builder.finish(settings::Flags::new(flag_builder))?;
    let libcalls = default_libcall_names();
    let mut module = ObjectModule::new(ObjectBuilder::new(isa.clone(), module_name, libcalls)?);
    let mut fbx = FunctionBuilderContext::new();
    let mut ctx = module.make_context();
    let mut ms_ctx = MsContext::new(0);
    ms_ctx.disable_auto_drop = !auto_drop;

    for declaration in program.declarations {
        match declaration {
            Declaration::Function(function_decl) => {
                compile_function(
                    function_decl,
                    &mut module,
                    &mut ctx,
                    &mut fbx,
                    &mut ms_ctx,
                    None,
                    None,
                );
            }
            Declaration::TypeDef(typedef) => {
                let name = &typedef.name;
                match &typedef.definition {
                    TypeDefBody::Alias(_) | TypeDefBody::Struct(_) | TypeDefBody::Enum(_) => {
                        match name {
                            TypeExpr::Generic(base, generics) => {
                                let generics = generics
                                    .iter()
                                    .map(|x| x.as_name().unwrap().to_string())
                                    .collect::<Vec<String>>();
                                let resolved_ty = match &typedef.definition {
                                    TypeDefBody::Alias(ty) => ty.clone(),
                                    // For struct/enum defs, resolve differently
                                    _ => name.clone(),
                                };
                                let template = Rc::new(
                                    ms_ctx.current_module.resolve_with_generics(&resolved_ty, &generics),
                                );
                                let key = base.as_name().unwrap();
                                log::info!("template generated aliased {} -> {:?}", key, template);
                                ms_ctx
                                    .current_module
                                    .type_templates
                                    .registry
                                    .insert(key.into(), template.clone());
                            }
                            TypeExpr::Named(ident) => {
                                let alias = ident.name.as_str();
                                match &typedef.definition {
                                    TypeDefBody::Alias(target_ty) => {
                                        if let Some(MsResolved::Type(resolved)) =
                                            ms_ctx.current_module.resolve(target_ty)
                                        {
                                            ms_ctx
                                                .current_module
                                                .type_registry
                                                .add_alias(alias, resolved.id);
                                        } else {
                                            log::warn!("found an undefined type, creating type");
                                            todo!("add types to ms_context");
                                        }
                                    }
                                    _ => {
                                        // struct/enum definitions handled during resolve
                                        todo!("struct/enum type registration");
                                    }
                                }
                            }
                            _ => todo!(),
                        }
                    }
                };
            }
            Declaration::Use(_use_decl) => {
                todo!("use decl should compile the modules");
            }
            Declaration::Import(_import_decl) => {
                todo!("import decl should compile the modules");
            }
            Declaration::Trait(trait_def) => {
                let trait_name = trait_def.name.as_name().unwrap();
                let functions = trait_def.methods;

                ms_ctx
                    .current_module
                    .trait_templates
                    .registry
                    .insert(trait_name.into(), functions);
                ms_ctx
                    .current_module
                    .trait_registry
                    .registry
                    .insert(trait_name.into(), Default::default());

                log::info!("Added functions of trait {}", trait_name);
            }
            Declaration::Impl(impl_block) => {
                if impl_block.generics.is_empty() {
                    let for_type = impl_block.for_type.as_ref().expect("impl block should have a for_type");
                    let ty = ms_ctx.current_module.resolve(for_type).unwrap().ty().unwrap();
                    ms_ctx
                        .current_module
                        .add_alias(TypeNameWithGenerics::new("Self".into(), vec![]), ty.clone());

                    let trait_name = impl_block.trait_name.as_name().unwrap();
                    let mut fn_registry = MsFunctionRegistry::default();

                    for function in impl_block.methods {
                        let trait_fn_for = TraitFunctionFor {
                            trait_name,
                            on_type: &ty,
                        };

                        let fn_name = random_string(24);
                        let decl = compile_function(
                            function,
                            &mut module,
                            &mut ctx,
                            &mut fbx,
                            &mut ms_ctx,
                            Some(trait_fn_for),
                            Some(&fn_name),
                        );
                    }
                } else {
                    let generics: Vec<Box<str>> = impl_block
                        .generics
                        .iter()
                        .map(|g| g.name.clone().into_boxed_str())
                        .collect();

                    for function in impl_block.methods {
                        let func_name: Box<str> = function
                            .name
                            .as_ref()
                            .and_then(|n| n.as_name())
                            .unwrap()
                            .into();
                        let template = MsGenericFunction {
                            decl: Rc::new(function),
                            generics: generics.clone(),
                        };

                        let for_type = impl_block.for_type.as_ref().unwrap();
                        let ty_name = TypeNameWithGenerics::from_type(for_type).unwrap().name;

                        let registry = if let Some(registry) = ms_ctx
                            .current_module
                            .trait_generic_templates
                            .registry
                            .get_mut(&ty_name)
                        {
                            registry
                        } else {
                            ms_ctx
                                .current_module
                                .trait_generic_templates
                                .registry
                                .insert(ty_name.clone(), Default::default());

                            ms_ctx
                                .current_module
                                .trait_generic_templates
                                .registry
                                .get_mut(&ty_name)
                                .unwrap()
                        };

                        registry.push(template);
                    }
                }
                ms_ctx.current_module.clear_aliases();
            }
        }
    }

    let object_product = module.finish();

    let bytes = object_product.emit()?;

    Ok(bytes)
}

pub fn compile_main_fn(
    module: &mut ObjectModule,
    ctx: &mut Context,
    fbx: &mut FunctionBuilderContext,
    ms_ctx: &mut MsContext,
) {
    ctx.func.signature.params.push(AbiParam::new(types::I32)); // argv
    ctx.func.signature.params.push(AbiParam::new(types::I64)); // char** argc
    ctx.func.signature.returns.push(AbiParam::new(types::I32)); // exit code

    let func_id = module
        .declare_function("main", Linkage::Preemptible, &ctx.func.signature)
        .unwrap();
}

