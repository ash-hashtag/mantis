use linear_map::LinearMap;
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
    backend::compile_function::MethodFor,
    ms::MsContext,
    registries::{
        functions::{FunctionType, MsDeclaredFunction, MsFunctionRegistry, MsGenericFunction},
        modules::{resolve_module_by_word, MsResolved},
        structs::{MsEnumType, MsStructType},
        types::{
            EnumWithGenerics, MsGenericTemplate, MsGenericTemplateInner, MsType, MsTypeRegistry,
            StructWithGenerics, TypeNameWithGenerics,
        },
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

    // Register StrSlice
    {
        use crate::registries::structs::{MsStructFieldValue, MsStructType};
        use std::collections::HashMap;

        let i64_ty = ms_ctx
            .current_module
            .type_registry
            .get_from_str("i64")
            .unwrap();

        let mut fields = HashMap::new();
        fields.insert(
            "pointer".into(),
            MsStructFieldValue {
                offset: 0,
                ty: i64_ty.id,
            },
        );
        fields.insert(
            "len".into(),
            MsStructFieldValue {
                offset: 8,
                ty: i64_ty.id,
            },
        );
        let str_slice_ty = MsStructType::new(fields, 16);
        ms_ctx.current_module.type_registry.add_type(
            "StrSlice",
            MsType::Struct(Rc::new(str_slice_ty)),
        );
    }

    // Register pointer template
    {
        let i64_ty = ms_ctx
            .current_module
            .type_registry
            .get_from_str("i64")
            .unwrap();
        let template = MsGenericTemplate {
            name: "pointer".into(),
            generics: vec!["T".into()],
            inner_type: MsGenericTemplateInner::Type(TypeNameWithGenerics::new("i64".into(), vec![])),
        };
        ms_ctx
            .current_module
            .type_templates
            .registry
            .insert("pointer".into(), Rc::new(template));
    }

    // Register implicit 'malloc' and 'memcpy' if not already declared in source
    {
        use mantis_parser::ast::Declaration;
        let has_decl = |name: &str| {
            program.declarations.iter().any(|d| match d {
                Declaration::Function(f) => f.name.as_ref().and_then(|n| n.as_name()) == Some(name),
                _ => false,
            })
        };

        if !has_decl("malloc") {
            let mut malloc_sig = module.make_signature();
            malloc_sig.params.push(AbiParam::new(types::I64).sext());
            malloc_sig.returns.push(AbiParam::new(types::I64).sext());
            let malloc_id = module.declare_function("malloc", Linkage::Import, &malloc_sig).unwrap();
            ms_ctx.current_module.fn_registry.add_function("malloc", Rc::new(MsDeclaredFunction {
                func_id: malloc_id,
                arguments: Default::default(),
                rets: Some(ms_ctx.current_module.type_registry.get_from_str("i64").unwrap().id),
                fn_type: FunctionType::Extern,
            }));
        }

        if !has_decl("free") {
            let mut free_sig = module.make_signature();
            free_sig.params.push(AbiParam::new(types::I64).sext());
            let free_id = module.declare_function("free", Linkage::Import, &free_sig).unwrap();
            ms_ctx.current_module.fn_registry.add_function("free", Rc::new(MsDeclaredFunction {
                func_id: free_id,
                arguments: Default::default(),
                rets: None,
                fn_type: FunctionType::Extern,
            }));
        }

        if !has_decl("memcpy") {
            let mut memcpy_sig = module.make_signature();
            memcpy_sig.params.push(AbiParam::new(types::I64).sext()); // dest
            memcpy_sig.params.push(AbiParam::new(types::I64).sext()); // src
            memcpy_sig.params.push(AbiParam::new(types::I64).sext()); // size
            let memcpy_id = module.declare_function("memcpy", Linkage::Import, &memcpy_sig).unwrap();
            ms_ctx.current_module.fn_registry.add_function("memcpy", Rc::new(MsDeclaredFunction {
                func_id: memcpy_id,
                arguments: Default::default(),
                rets: Some(ms_ctx.current_module.type_registry.get_from_str("i64").unwrap().id),
                fn_type: FunctionType::Extern,
            }));
        }

        if !has_decl("memcmp") {
            let mut memcmp_sig = module.make_signature();
            memcmp_sig.params.push(AbiParam::new(types::I64).sext()); // s1
            memcmp_sig.params.push(AbiParam::new(types::I64).sext()); // s2
            memcmp_sig.params.push(AbiParam::new(types::I64).sext()); // n
            memcmp_sig.returns.push(AbiParam::new(types::I32).sext());
            let memcmp_id = module.declare_function("memcmp", Linkage::Import, &memcmp_sig).unwrap();
            ms_ctx.current_module.fn_registry.add_function("memcmp", Rc::new(MsDeclaredFunction {
                func_id: memcmp_id,
                arguments: Default::default(),
                rets: Some(ms_ctx.current_module.type_registry.get_from_str("i32").unwrap().id),
                fn_type: FunctionType::Extern,
            }));
        }

        if !has_decl("print") {
            let mut puts_sig = module.make_signature();
            puts_sig.params.push(AbiParam::new(types::I64).sext());
            puts_sig.returns.push(AbiParam::new(types::I32).sext());
            let puts_id = module.declare_function("puts", Linkage::Import, &puts_sig).unwrap();
            let mut print_arguments = LinearMap::new();
            print_arguments.insert("s".into(), ms_ctx.current_module.type_registry.get_from_str("i64").unwrap().id);
            ms_ctx.current_module.fn_registry.add_function("print", Rc::new(MsDeclaredFunction {
                func_id: puts_id,
                arguments: print_arguments,
                rets: Some(ms_ctx.current_module.type_registry.get_from_str("i32").unwrap().id),
                fn_type: FunctionType::Extern,
            }));
        }
    }



    for declaration in program.declarations {
        match declaration {
            Declaration::Function(function_decl) => {
                let mut auto_generics = Vec::new();
                let mut function_decl = function_decl;
                let mut was_explicit_generic = false;

                if let Some(TypeExpr::Generic(_, _)) = &function_decl.name {
                    was_explicit_generic = true;
                } else if !function_decl.is_extern {
                    for param in function_decl.params.iter() {
                        if matches!(param.ty, TypeExpr::Unknown) {
                            panic!("Function '{}' has missing type for parameter '{}'. All parameters must have explicit types.", 
                                function_decl.name.as_ref().and_then(|n| n.as_name()).unwrap_or("unknown"),
                                param.name.name
                            );
                        }
                    }
                }

                if was_explicit_generic {
                    let (name, generics) = if let Some(TypeExpr::Generic(base, generics)) =
                        &function_decl.name
                    {
                        let name = base
                            .as_name()
                            .expect("function name must be an identifier")
                            .to_string();
                        let generics = generics
                            .iter()
                            .map(|x| {
                                x.as_name()
                                    .expect("generic param must be an identifier")
                                    .into()
                            })
                            .collect::<Vec<Box<str>>>();
                        (name, generics)
                    } else {
                        let name = function_decl
                            .name
                            .as_ref()
                            .and_then(|n| n.as_name())
                            .expect("function must have a name")
                            .to_string();
                        (name, auto_generics)
                    };

                    let template = MsGenericFunction {
                        decl: Rc::new(function_decl),
                        generics,
                    };
                    ms_ctx
                        .current_module
                        .fn_templates
                        .registry
                        .insert(name.into(), template);
                } else {
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
            }
            Declaration::TypeDef(typedef) => {
                let name = &typedef.name;
                match &typedef.definition {
                    TypeDefBody::Alias(_) | TypeDefBody::Struct(_) | TypeDefBody::Enum(_) => {
                        match name {
                            TypeExpr::Generic(base, generics) => {
                                let generics = generics
                                    .iter()
                                    .map(|x| {
                                        x.as_name()
                                            .or_else(|| {
                                                if let mantis_parser::ast::TypeExpr::Generic(base, _) = x {
                                                    base.as_name()
                                                } else {
                                                    None
                                                }
                                            })
                                            .expect("generic name error")
                                            .into()
                                    })
                                    .collect::<Vec<Box<str>>>();
                                let template = match &typedef.definition {
                                    TypeDefBody::Alias(ty) => Rc::new(
                                        ms_ctx.current_module.resolve_with_generics(ty, &generics),
                                    ),
                                    TypeDefBody::Struct(struct_def) => {
                                        let mut map = linear_map::LinearMap::new();
                                        for field in &struct_def.fields {
                                            map.insert(
                                                field.name.name.clone().into_boxed_str(),
                                                TypeNameWithGenerics::from_type(&field.ty).unwrap(),
                                            );
                                        }
                                        Rc::new(MsGenericTemplate {
                                            name: base.as_name().unwrap().to_string().into(),
                                            generics: generics.clone(),
                                            inner_type: MsGenericTemplateInner::Struct(
                                                StructWithGenerics { map },
                                            ),
                                        })
                                    }
                                    TypeDefBody::Enum(enum_def) => {
                                        let mut map = linear_map::LinearMap::new();
                                        for variant in &enum_def.variants {
                                            let ty = if !variant.fields.is_empty() {
                                                Some(
                                                    TypeNameWithGenerics::from_type(
                                                        &variant.fields[0],
                                                    )
                                                    .unwrap(),
                                                )
                                            } else {
                                                None
                                            };
                                            map.insert(
                                                variant.name.name.clone().into_boxed_str(),
                                                ty,
                                            );
                                        }
                                        Rc::new(MsGenericTemplate {
                                            name: base.as_name().unwrap().to_string().into(),
                                            generics: generics.clone(),
                                            inner_type: MsGenericTemplateInner::Enum(
                                                EnumWithGenerics { map },
                                            ),
                                        })
                                    }
                                };
                                let key = base
                                    .as_name()
                                    .or_else(|| {
                                        if let mantis_parser::ast::TypeExpr::Generic(b, _) = &**base {
                                            b.as_name()
                                        } else {
                                            None
                                        }
                                    })
                                    .expect("template base name error");
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
                                    TypeDefBody::Struct(struct_def) => {
                                        let mut ms_struct = MsStructType::default();
                                        for field in &struct_def.fields {
                                            let ty = ms_ctx
                                                .current_module
                                                .resolve(&field.ty)
                                                .expect(&format!("unable to resolve field type {:?}", field.ty))
                                                .ty()
                                                .unwrap();
                                            ms_struct.add_field(field.name.name.as_str(), ty);
                                        }
                                        ms_ctx.current_module.type_registry.add_type(
                                            alias,
                                            MsType::Struct(Rc::new(ms_struct)),
                                        );
                                    }
                                    TypeDefBody::Enum(enum_def) => {
                                        let mut ms_enum = MsEnumType::default();
                                        for variant in &enum_def.variants {
                                            let ty = if !variant.fields.is_empty() {
                                                Some(
                                                    ms_ctx
                                                        .current_module
                                                        .resolve(&variant.fields[0])
                                                        .unwrap()
                                                        .ty()
                                                        .unwrap(),
                                                )
                                            } else {
                                                None
                                            };
                                            ms_enum.add_variant(variant.name.name.as_str(), ty);
                                        }
                                        ms_ctx.current_module.type_registry.add_type(
                                            alias,
                                            MsType::Enum(Rc::new(ms_enum)),
                                        );
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
                let trait_name = trait_def
                    .name
                    .as_name()
                    .or_else(|| {
                        if let mantis_parser::ast::TypeExpr::Generic(base, _) = &trait_def.name {
                            base.as_name()
                        } else {
                            None
                        }
                    })
                    .expect("trait name should be an identifier or generic with identifier base");

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
                    let for_type = if let Some(ref for_ty) = impl_block.for_type {
                        ms_ctx.current_module.resolve(for_ty).unwrap().ty().unwrap()
                    } else {
                        ms_ctx.current_module.resolve(&impl_block.trait_name).unwrap().ty().unwrap()
                    };

                    ms_ctx
                        .current_module
                        .add_alias(TypeNameWithGenerics::new("Self".into(), vec![]), for_type.clone());

                    let trait_name = if impl_block.for_type.is_some() {
                        Some(
                            impl_block
                                .trait_name
                                .as_name()
                                .or_else(|| {
                                    if let mantis_parser::ast::TypeExpr::Generic(base, _) =
                                        &impl_block.trait_name
                                    {
                                        base.as_name()
                                    } else {
                                        None
                                    }
                                })
                                .expect("impl trait name error"),
                        )
                    } else {
                        None
                    };

                    for function in impl_block.methods {
                        let method_for = MethodFor {
                            trait_name,
                            on_type: &for_type,
                        };

                        let fn_name = random_string(24);
                        let _decl = compile_function(
                            function,
                            &mut module,
                            &mut ctx,
                            &mut fbx,
                            &mut ms_ctx,
                            Some(method_for),
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
                        let name_expr = function.name.as_ref().unwrap();
                        let name = name_expr.as_name().or_else(|| {
                            if let TypeExpr::Generic(base, _) = name_expr {
                                base.as_name()
                            } else { None }
                        }).expect(&format!("method name error in {:?}", name_expr));
                        let func_name: Box<str> = name.into();
                        let template = MsGenericFunction {
                            decl: Rc::new(function),
                            generics: generics.clone(),
                        };

                        let for_type_node = impl_block.for_type.as_ref().unwrap_or(&impl_block.trait_name);
                        let ty_name = TypeNameWithGenerics::from_type(for_type_node).unwrap().name;

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

    while !ms_ctx.instantiation_queue.is_empty() {
        let insts: Vec<_> = ms_ctx.instantiation_queue.drain(..).collect();
        for inst in insts {
            // Compile the instantiation
            // Set aliases
            for (name, res) in inst.template.generics.iter().zip(inst.real_types.iter()) {
                if let Some(ty) = res.ty() {
                    ms_ctx.current_module.add_alias(
                        TypeNameWithGenerics::new(name.clone(), vec![]),
                        ty,
                    );
                }
            }

            compile_function(
                inst.template.decl.as_ref().clone(),
                &mut module,
                &mut ctx,
                &mut fbx,
                &mut ms_ctx,
                None,
                Some(&inst.instantiation_name),
            );

            ms_ctx.current_module.clear_aliases();
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

