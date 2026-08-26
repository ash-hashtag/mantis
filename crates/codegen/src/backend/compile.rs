use linear_map::LinearMap;
use std::{collections::HashMap, path::PathBuf, rc::Rc};

use cranelift::{
    codegen::Context,
    prelude::{settings, types, AbiParam, Configurable, FunctionBuilder, FunctionBuilderContext},
};
use cranelift_module::{default_libcall_names, DataDescription, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};
use mantis_parser::ast::{
    Block, Declaration, FnDecl, ImplBlock, Program, TraitDef, TypeDef, TypeDefBody, TypeExpr,
};

use crate::{
    backend::compile_function::MethodFor,
    ms::MsContext,
    registries::{
        functions::{FunctionType, MsDeclaredFunction, MsFunctionRegistry, MsGenericFunction},
        modules::{resolve_module_by_path, resolve_module_by_word, MsResolved},
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


fn transform_return_statements(
    block: &mut mantis_parser::ast::Block,
    ret_ty: &mantis_parser::ast::TypeExpr,
    span: mantis_parser::token::Span,
) {
    use mantis_parser::ast::*;
    let mut new_items = Vec::new();
    for item in std::mem::take(&mut block.items) {
        match item {
            BlockItem::Statement(Statement::Return { value, span: ret_span }) => {
                let self_result = Expr::Field {
                    object: Box::new(Expr::Ident(Ident::new("self", ret_span))),
                    field: Ident::new("result", ret_span),
                    span: ret_span,
                };
                let self_completed = Expr::Field {
                    object: Box::new(Expr::Ident(Ident::new("self", ret_span))),
                    field: Ident::new("completed", ret_span),
                    span: ret_span,
                };

                if let Some(val_expr) = value {
                    new_items.push(BlockItem::Statement(Statement::Expr {
                        expr: Expr::Binary {
                            op: BinOp::Assign,
                            lhs: Box::new(self_result.clone()),
                            rhs: Box::new(val_expr),
                            span: ret_span,
                        },
                        span: ret_span,
                    }));
                }
                new_items.push(BlockItem::Statement(Statement::Expr {
                    expr: Expr::Binary {
                        op: BinOp::Assign,
                        lhs: Box::new(self_completed),
                        rhs: Box::new(Expr::BoolLit { value: true, span: ret_span }),
                        span: ret_span,
                    },
                    span: ret_span,
                }));
                let poll_ready_callee = Expr::Field {
                    object: Box::new(Expr::Ident(Ident::new("Poll", ret_span))),
                    field: Ident::new("Ready", ret_span),
                    span: ret_span,
                };
                let poll_ready_call = Expr::Call {
                    callee: Box::new(poll_ready_callee),
                    args: vec![self_result],
                    span: ret_span,
                };
                new_items.push(BlockItem::Statement(Statement::Return {
                    value: Some(poll_ready_call),
                    span: ret_span,
                }));
            }
            BlockItem::IfChain(mut if_chain) => {
                transform_return_statements(&mut if_chain.if_block.body, ret_ty, span);
                for elif in &mut if_chain.elif_blocks {
                    transform_return_statements(&mut elif.body, ret_ty, span);
                }
                if let Some(else_b) = &mut if_chain.else_block {
                    transform_return_statements(else_b, ret_ty, span);
                }
                new_items.push(BlockItem::IfChain(if_chain));
            }
            BlockItem::Loop(mut loop_b) => {
                transform_return_statements(&mut loop_b.body, ret_ty, span);
                new_items.push(BlockItem::Loop(loop_b));
            }
            BlockItem::Match(mut match_b) => {
                for arm in &mut match_b.arms {
                    transform_return_statements(&mut arm.body, ret_ty, span);
                }
                new_items.push(BlockItem::Match(match_b));
            }
            BlockItem::Block(mut sub_b) => {
                transform_return_statements(&mut sub_b, ret_ty, span);
                new_items.push(BlockItem::Block(sub_b));
            }
            other => new_items.push(other),
        }
    }
    block.items = new_items;
}

fn expand_async_functions(declarations: &mut Vec<Declaration>) {
    use mantis_parser::ast::*;
    use mantis_parser::token::Span;

    let has_poll = declarations.iter().any(|d| match d {
        Declaration::TypeDef(t) => t.name.as_name() == Some("Poll"),
        _ => false,
    });

    let dummy_span = Span::new(0, 0);
    let mut synthetic_decls = Vec::new();

    if !has_poll {
        let poll_typedef = Declaration::TypeDef(TypeDef {
            name: TypeExpr::Generic(
                Box::new(TypeExpr::Named(Ident::new("Poll", dummy_span))),
                vec![TypeExpr::Named(Ident::new("T", dummy_span))],
            ),
            definition: TypeDefBody::Enum(EnumDef {
                variants: vec![
                    EnumVariant {
                        name: Ident::new("Ready", dummy_span),
                        fields: vec![TypeExpr::Named(Ident::new("T", dummy_span))],
                        span: dummy_span,
                    },
                    EnumVariant {
                        name: Ident::new("Pending", dummy_span),
                        fields: vec![],
                        span: dummy_span,
                    },
                ],
                span: dummy_span,
            }),
            span: dummy_span,
        });
        synthetic_decls.push(poll_typedef);
    }

    let mut new_declarations = Vec::with_capacity(declarations.len());

    for decl in declarations.drain(..) {
        match decl {
            Declaration::Function(mut fn_decl) if fn_decl.is_async => {
                let span = fn_decl.span;
                let fn_name = fn_decl
                    .name
                    .as_ref()
                    .and_then(|n| n.as_name())
                    .unwrap_or("anon_async")
                    .to_string();

                let future_struct_name = format!("__Future_{}", fn_name);
                let ret_ty = fn_decl
                    .return_type
                    .clone()
                    .unwrap_or_else(|| TypeExpr::Named(Ident::new("i64", span)));

                // 1. Anonymous Future struct definition
                let mut struct_fields = Vec::new();
                struct_fields.push(Param {
                    name: Ident::new("state", span),
                    mutable: true,
                    ty: TypeExpr::Named(Ident::new("u32", span)),
                    span,
                });
                struct_fields.push(Param {
                    name: Ident::new("completed", span),
                    mutable: true,
                    ty: TypeExpr::Named(Ident::new("bool", span)),
                    span,
                });
                for p in &fn_decl.params {
                    struct_fields.push(Param {
                        name: p.name.clone(),
                        mutable: true,
                        ty: p.ty.clone(),
                        span: p.span,
                    });
                }
                struct_fields.push(Param {
                    name: Ident::new("result", span),
                    mutable: true,
                    ty: ret_ty.clone(),
                    span,
                });

                let future_typedef = Declaration::TypeDef(TypeDef {
                    name: TypeExpr::Named(Ident::new(&future_struct_name, span)),
                    definition: TypeDefBody::Struct(StructDef {
                        fields: struct_fields,
                        span,
                    }),
                    span,
                });

                // 2. Synthesize 
                let self_ty = TypeExpr::Ref(
                    Box::new(TypeExpr::Named(Ident::new(&future_struct_name, span))),
                    false,
                );
                let poll_ret_ty = TypeExpr::Generic(
                    Box::new(TypeExpr::Named(Ident::new("Poll", span))),
                    vec![ret_ty.clone()],
                );

                let mut poll_body_items = Vec::new();

                let if_completed = IfChain {
                    if_block: ConditionalBlock {
                        condition: Expr::Field {
                            object: Box::new(Expr::Ident(Ident::new("self", span))),
                            field: Ident::new("completed", span),
                            span,
                        },
                        body: Block {
                            items: vec![BlockItem::Statement(Statement::Return {
                                value: Some(Expr::Call {
                                    callee: Box::new(Expr::Field {
                                        object: Box::new(Expr::Ident(Ident::new("Poll", span))),
                                        field: Ident::new("Ready", span),
                                        span,
                                    }),
                                    args: vec![Expr::Field {
                                        object: Box::new(Expr::Ident(Ident::new("self", span))),
                                        field: Ident::new("result", span),
                                        span,
                                    }],
                                    span,
                                }),
                                span,
                            })],
                            span,
                        },
                        span,
                    },
                    elif_blocks: vec![],
                    else_block: None,
                    span,
                };
                poll_body_items.push(BlockItem::IfChain(if_completed));

                for p in &fn_decl.params {
                    poll_body_items.push(BlockItem::Statement(Statement::Let {
                        mutable: true,
                        name: p.name.clone(),
                        ty: Some(p.ty.clone()),
                        value: Expr::Field {
                            object: Box::new(Expr::Ident(Ident::new("self", p.span))),
                            field: p.name.clone(),
                            span: p.span,
                        },
                        span: p.span,
                    }));
                }

                if let Some(mut body) = fn_decl.body.take() {
                    transform_return_statements(&mut body, &ret_ty, span);
                    poll_body_items.extend(body.items);
                }

                poll_body_items.push(BlockItem::Statement(Statement::Expr {
                    expr: Expr::Binary {
                        op: BinOp::Assign,
                        lhs: Box::new(Expr::Field {
                            object: Box::new(Expr::Ident(Ident::new("self", span))),
                            field: Ident::new("completed", span),
                            span,
                        }),
                        rhs: Box::new(Expr::BoolLit { value: true, span }),
                        span,
                    },
                    span,
                }));
                poll_body_items.push(BlockItem::Statement(Statement::Return {
                    value: Some(Expr::Call {
                        callee: Box::new(Expr::Field {
                            object: Box::new(Expr::Ident(Ident::new("Poll", span))),
                            field: Ident::new("Ready", span),
                            span,
                        }),
                        args: vec![Expr::Field {
                            object: Box::new(Expr::Ident(Ident::new("self", span))),
                            field: Ident::new("result", span),
                            span,
                        }],
                        span,
                    }),
                    span,
                }));

                let poll_fn = FnDecl {
                    name: Some(TypeExpr::Named(Ident::new("poll", span))),
                    params: vec![Param {
                        name: Ident::new("self", span),
                        mutable: false,
                        ty: self_ty.clone(),
                        span,
                    }],
                    return_type: Some(poll_ret_ty),
                    where_clause: vec![],
                    body: Some(Block {
                        items: poll_body_items,
                        span,
                    }),
                    is_extern: false,
                    is_async: false,
                    trailing_params: None,
                    span,
                };

                let future_impl = Declaration::Impl(ImplBlock {
                    generics: vec![],
                    trait_name: TypeExpr::Named(Ident::new(&future_struct_name, span)),
                    for_type: None,
                    methods: vec![poll_fn],
                    span,
                });

                // 3. Transform original fn into constructor returning @__Future_<name>
                fn_decl.is_async = false;
                fn_decl.return_type = Some(self_ty.clone());

                let mut ctor_body_items = Vec::new();
                let size_of_call = Expr::CompilerCall {
                    name: "size_of".to_string(),
                    args: vec![Expr::TypeExpr(TypeExpr::Named(Ident::new(
                        &future_struct_name,
                        span,
                    )))],
                    span,
                };
                let malloc_call = Expr::Call {
                    callee: Box::new(Expr::Ident(Ident::new("malloc", span))),
                    args: vec![size_of_call],
                    span,
                };
                let cast_malloc = Expr::Cast {
                    expr: Box::new(malloc_call),
                    ty: self_ty.clone(),
                    span,
                };
                ctor_body_items.push(BlockItem::Statement(Statement::Let {
                    mutable: true,
                    name: Ident::new("__fut", span),
                    ty: Some(self_ty.clone()),
                    value: cast_malloc,
                    span,
                }));

                ctor_body_items.push(BlockItem::Statement(Statement::Expr {
                    expr: Expr::Binary {
                        op: BinOp::Assign,
                        lhs: Box::new(Expr::Field {
                            object: Box::new(Expr::Ident(Ident::new("__fut", span))),
                            field: Ident::new("state", span),
                            span,
                        }),
                        rhs: Box::new(Expr::Cast {
                            expr: Box::new(Expr::IntLit { value: 0, span }),
                            ty: TypeExpr::Named(Ident::new("u32", span)),
                            span,
                        }),
                        span,
                    },
                    span,
                }));

                ctor_body_items.push(BlockItem::Statement(Statement::Expr {
                    expr: Expr::Binary {
                        op: BinOp::Assign,
                        lhs: Box::new(Expr::Field {
                            object: Box::new(Expr::Ident(Ident::new("__fut", span))),
                            field: Ident::new("completed", span),
                            span,
                        }),
                        rhs: Box::new(Expr::BoolLit { value: false, span }),
                        span,
                    },
                    span,
                }));

                for p in &fn_decl.params {
                    ctor_body_items.push(BlockItem::Statement(Statement::Expr {
                        expr: Expr::Binary {
                            op: BinOp::Assign,
                            lhs: Box::new(Expr::Field {
                                object: Box::new(Expr::Ident(Ident::new("__fut", p.span))),
                                field: p.name.clone(),
                                span: p.span,
                            }),
                            rhs: Box::new(Expr::Ident(p.name.clone())),
                            span: p.span,
                        },
                        span: p.span,
                    }));
                }

                ctor_body_items.push(BlockItem::Statement(Statement::Return {
                    value: Some(Expr::Ident(Ident::new("__fut", span))),
                    span,
                }));

                fn_decl.body = Some(Block {
                    items: ctor_body_items,
                    span,
                });

                new_declarations.push(future_typedef);
                new_declarations.push(future_impl);
                new_declarations.push(Declaration::Function(fn_decl));
            }
            other => new_declarations.push(other),
        }
    }

    synthetic_decls.extend(new_declarations);
    *declarations = synthetic_decls;
}

pub fn compile_binary(
    program: Program,
    include_dirs: Vec<String>,
    module_name: &str,
    auto_drop: bool,
    config: crate::config::MantisConfig,
) -> anyhow::Result<Vec<u8>> {
    // ── Policy enforcement ────────────────────────────────────────────
    let violations = config.check_program(&program);
    if !violations.is_empty() {
        for v in &violations {
            eprintln!("\x1b[31;1merror:\x1b[0m {}", v);
        }
        return Err(anyhow::anyhow!(
            "{} policy violation(s); adjust config.toml or the --allow-* flags",
            violations.len()
        ));
    }

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
    ms_ctx.config = config;

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
        ms_ctx
            .current_module
            .type_registry
            .add_type("StrSlice", MsType::Struct(Rc::new(str_slice_ty)));
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
            inner_type: MsGenericTemplateInner::Type(TypeNameWithGenerics::new(
                "i64".into(),
                vec![],
            )),
        };
        ms_ctx
            .current_module
            .type_templates
            .registry
            .insert("pointer".into(), Rc::new(template));
    }

    let mut include_dirs = if include_dirs.is_empty() {
        vec![".".to_string(), "std".to_string()]
    } else {
        include_dirs
    };
    if !include_dirs.contains(&"../std".to_string()) {
        include_dirs.push("../std".to_string());
    }
    if !include_dirs.contains(&"../../std".to_string()) {
        include_dirs.push("../../std".to_string());
    }
    if !include_dirs.contains(&"..".to_string()) {
        include_dirs.push("..".to_string());
    }
    if !include_dirs.contains(&"../..".to_string()) {
        include_dirs.push("../..".to_string());
    }

    if let Ok(std_env) = std::env::var("MANTIS_STD") {
        if !include_dirs.contains(&std_env) {
            include_dirs.push(std_env);
        }
    }

    if let Ok(exe_path) = std::env::current_exe() {
        let mut curr = exe_path.parent();
        while let Some(dir) = curr {
            let std_ms = dir.join("std.ms");
            let std_dir = dir.join("std");
            if std_ms.exists() || std_dir.exists() {
                let dir_str = dir.to_string_lossy().to_string();
                if !include_dirs.contains(&dir_str) {
                    include_dirs.push(dir_str);
                }
                break;
            }
            curr = dir.parent();
        }
    }

    let mut declarations = Vec::new();
    let mut visited = std::collections::HashSet::new();

    fn expand_declarations(
        decls: Vec<Declaration>,
        include_dirs: &[String],
        target: &mut Vec<Declaration>,
        visited: &mut std::collections::HashSet<String>,
    ) {
        for decl in decls {
            match decl {
                Declaration::Import(ref import_decl) => {
                    let mod_key = import_decl
                        .path
                        .iter()
                        .map(|i| i.name.as_str())
                        .collect::<Vec<_>>()
                        .join(".");
                    if !mod_key.is_empty() && visited.insert(mod_key.clone()) {
                        if let Some(entry) = resolve_module_by_path(include_dirs, &import_decl.path)
                        {
                            let content = match entry {
                                crate::registries::modules::ModuleEntry::Module(c) => c,
                                crate::registries::modules::ModuleEntry::Dir(p) => {
                                    let candidates = [
                                        p.with_extension("ms"),
                                        p.join("mod.ms"),
                                        p.join("lib.ms"),
                                    ];
                                    let mut found_c = String::new();
                                    for cand in &candidates {
                                        if let Ok(c) = std::fs::read_to_string(cand) {
                                            found_c = c;
                                            break;
                                        }
                                    }
                                    found_c
                                }
                            };
                            match mantis_parser::parse(&content) {
                                Ok(parsed) => {
                                    expand_declarations(
                                        parsed.declarations,
                                        include_dirs,
                                        target,
                                        visited,
                                    );
                                }
                                Err(err) => {
                                    panic!(
                                        "Failed to parse module for Import {:?}: {:?}",
                                        import_decl.path, err
                                    );
                                }
                            }
                        }
                    }
                }
                Declaration::Use(ref use_decl) => {
                    let mod_key = use_decl
                        .path
                        .iter()
                        .map(|i| i.name.as_str())
                        .collect::<Vec<_>>()
                        .join(".");
                    if !mod_key.is_empty() && visited.insert(mod_key.clone()) {
                        if let Some(entry) = resolve_module_by_path(include_dirs, &use_decl.path) {
                            let content = match entry {
                                crate::registries::modules::ModuleEntry::Module(c) => c,
                                crate::registries::modules::ModuleEntry::Dir(p) => {
                                    let candidates = [
                                        p.with_extension("ms"),
                                        p.join("mod.ms"),
                                        p.join("lib.ms"),
                                    ];
                                    let mut found_c = String::new();
                                    for cand in &candidates {
                                        if let Ok(c) = std::fs::read_to_string(cand) {
                                            found_c = c;
                                            break;
                                        }
                                    }
                                    found_c
                                }
                            };
                            match mantis_parser::parse(&content) {
                                Ok(parsed) => {
                                    expand_declarations(
                                        parsed.declarations,
                                        include_dirs,
                                        target,
                                        visited,
                                    );
                                }
                                Err(err) => {
                                    panic!(
                                        "Failed to parse module for Use {:?}: {:?}",
                                        use_decl.path, err
                                    );
                                }
                            }
                        }
                    }
                }
                other => target.push(other),
            }
        }
    }

    expand_declarations(
        program.declarations,
        &include_dirs,
        &mut declarations,
        &mut visited,
    );

    // Expand async functions into Rust-style Future anonymous structs
    expand_async_functions(&mut declarations);

    // Register implicit 'malloc' and 'memcpy' if not already declared in source
    {
        use mantis_parser::ast::Declaration;
        let has_decl = |name: &str| {
            declarations.iter().any(|d| match d {
                Declaration::Function(f) => f.name.as_ref().and_then(|n| n.as_name()) == Some(name),
                _ => false,
            })
        };

        if !has_decl("malloc") {
            let mut malloc_sig = module.make_signature();
            malloc_sig.params.push(AbiParam::new(types::I64).sext());
            malloc_sig.returns.push(AbiParam::new(types::I64).sext());
            let malloc_id = module
                .declare_function("malloc", Linkage::Import, &malloc_sig)
                .unwrap();
            ms_ctx.current_module.fn_registry.add_function(
                "malloc",
                Rc::new(MsDeclaredFunction {
                    func_id: malloc_id,
                    arguments: Default::default(),
                    rets: Some(
                        ms_ctx
                            .current_module
                            .type_registry
                            .get_from_str("i64")
                            .unwrap()
                            .id,
                    ),
                    fn_type: FunctionType::Extern,
                }),
            );
        }

        if !has_decl("free") {
            let mut free_sig = module.make_signature();
            free_sig.params.push(AbiParam::new(types::I64).sext());
            let free_id = module
                .declare_function("free", Linkage::Import, &free_sig)
                .unwrap();
            ms_ctx.current_module.fn_registry.add_function(
                "free",
                Rc::new(MsDeclaredFunction {
                    func_id: free_id,
                    arguments: Default::default(),
                    rets: None,
                    fn_type: FunctionType::Extern,
                }),
            );
        }

        if !has_decl("memcpy") {
            let mut memcpy_sig = module.make_signature();
            memcpy_sig.params.push(AbiParam::new(types::I64).sext()); // dest
            memcpy_sig.params.push(AbiParam::new(types::I64).sext()); // src
            memcpy_sig.params.push(AbiParam::new(types::I64).sext()); // size
            let memcpy_id = module
                .declare_function("memcpy", Linkage::Import, &memcpy_sig)
                .unwrap();
            ms_ctx.current_module.fn_registry.add_function(
                "memcpy",
                Rc::new(MsDeclaredFunction {
                    func_id: memcpy_id,
                    arguments: Default::default(),
                    rets: Some(
                        ms_ctx
                            .current_module
                            .type_registry
                            .get_from_str("i64")
                            .unwrap()
                            .id,
                    ),
                    fn_type: FunctionType::Extern,
                }),
            );
        }

        if !has_decl("memcmp") {
            let mut memcmp_sig = module.make_signature();
            memcmp_sig.params.push(AbiParam::new(types::I64).sext()); // s1
            memcmp_sig.params.push(AbiParam::new(types::I64).sext()); // s2
            memcmp_sig.params.push(AbiParam::new(types::I64).sext()); // n
            memcmp_sig.returns.push(AbiParam::new(types::I32).sext());
            let memcmp_id = module
                .declare_function("memcmp", Linkage::Import, &memcmp_sig)
                .unwrap();
            ms_ctx.current_module.fn_registry.add_function(
                "memcmp",
                Rc::new(MsDeclaredFunction {
                    func_id: memcmp_id,
                    arguments: Default::default(),
                    rets: Some(
                        ms_ctx
                            .current_module
                            .type_registry
                            .get_from_str("i32")
                            .unwrap()
                            .id,
                    ),
                    fn_type: FunctionType::Extern,
                }),
            );
        }

        if !has_decl("print") {
            let mut puts_sig = module.make_signature();
            puts_sig.params.push(AbiParam::new(types::I64).sext());
            puts_sig.returns.push(AbiParam::new(types::I32).sext());
            let puts_id = module
                .declare_function("puts", Linkage::Import, &puts_sig)
                .unwrap();
            let mut print_arguments = LinearMap::new();
            print_arguments.insert(
                "s".into(),
                ms_ctx
                    .current_module
                    .type_registry
                    .get_from_str("i64")
                    .unwrap()
                    .id,
            );
            ms_ctx.current_module.fn_registry.add_function(
                "print",
                Rc::new(MsDeclaredFunction {
                    func_id: puts_id,
                    arguments: print_arguments,
                    rets: Some(
                        ms_ctx
                            .current_module
                            .type_registry
                            .get_from_str("i32")
                            .unwrap()
                            .id,
                    ),
                    fn_type: FunctionType::Extern,
                }),
            );
        }

        if !has_decl("exit") {
            let mut exit_sig = module.make_signature();
            exit_sig.params.push(AbiParam::new(types::I32).sext());
            let exit_id = module
                .declare_function("exit", Linkage::Import, &exit_sig)
                .unwrap();
            ms_ctx.current_module.fn_registry.add_function(
                "exit",
                Rc::new(MsDeclaredFunction {
                    func_id: exit_id,
                    arguments: Default::default(),
                    rets: None,
                    fn_type: FunctionType::Extern,
                }),
            );
        }
    }

    // Pass 1: Process all TypeDefs and Traits
    for declaration in &declarations {
        match declaration {
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
                                                if let mantis_parser::ast::TypeExpr::Generic(
                                                    base,
                                                    _,
                                                ) = x
                                                {
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
                                        if let mantis_parser::ast::TypeExpr::Generic(b, _) = &**base
                                        {
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
                                        if let Some(ty) = ms_ctx
                                            .current_module
                                            .resolve(target_ty)
                                            .and_then(|r| r.ty())
                                        {
                                            ms_ctx
                                                .current_module
                                                .type_registry
                                                .add_alias(alias, ty.id);
                                        }
                                    }
                                    TypeDefBody::Struct(struct_def) => {
                                        let mut ms_struct = MsStructType::default();
                                        for field in &struct_def.fields {
                                            let ty = ms_ctx
                                                .current_module
                                                .resolve(&field.ty)
                                                .and_then(|r| r.ty())
                                                .unwrap_or_else(|| {
                                                    if let TypeExpr::Ref(inner, is_mut) = &field.ty
                                                    {
                                                        let inner_ty = ms_ctx
                                                            .current_module
                                                            .resolve(inner)
                                                            .and_then(|r| r.ty())
                                                            .unwrap_or_else(|| {
                                                                let void_ty = ms_ctx
                                                                    .current_module
                                                                    .type_registry
                                                                    .get_from_str("void")
                                                                    .unwrap();
                                                                void_ty
                                                            });
                                                        let ref_ty = MsType::Ref(
                                                            Box::new(inner_ty.ty),
                                                            *is_mut,
                                                        );
                                                        let id = ms_ctx
                                                            .current_module
                                                            .type_registry
                                                            .get_or_add_type(ref_ty.clone());
                                                        crate::registries::types::MsTypeWithId {
                                                            id,
                                                            ty: ref_ty,
                                                        }
                                                    } else {
                                                        panic!(
                                                            "unable to resolve field type {:?}",
                                                            field.ty
                                                        );
                                                    }
                                                });
                                            ms_struct.add_field(field.name.name.as_str(), ty);
                                        }
                                        ms_ctx
                                            .current_module
                                            .type_registry
                                            .add_type(alias, MsType::Struct(Rc::new(ms_struct)));
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
                                        ms_ctx
                                            .current_module
                                            .type_registry
                                            .add_type(alias, MsType::Enum(Rc::new(ms_enum)));
                                    }
                                }
                            }
                            _ => todo!(),
                        }
                    }
                }
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

                let functions = trait_def.methods.clone();

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
            _ => {}
        }
    }

    // Pass 2: Pre-declare non-generic function signatures
    // Materialize statics after their types are known. Static initializers are
    // deliberately restricted to compile-time zero-valued struct literals for
    // now; this keeps their address stable and prevents hidden runtime work.
    for declaration in &declarations {
        if let Declaration::Static(static_decl) = declaration {
            let ty = ms_ctx
                .current_module
                .resolve(&static_decl.ty)
                .and_then(|r| r.ty())
                .unwrap_or_else(|| {
                    panic!(
                        "undefined type for static '{}' at {}",
                        static_decl.name.name,
                        static_decl.ty.span()
                    )
                });
            match (&ty.ty, &static_decl.value) {
                (MsType::Struct(sty), mantis_parser::ast::Expr::StructInit { fields, .. })
                    if fields.is_empty() =>
                {
                    let data_id = module
                        .declare_data(
                            &static_decl.name.name,
                            Linkage::Local,
                            !static_decl.is_const,
                            false,
                        )
                        .unwrap_or_else(|e| {
                            panic!(
                                "invalid static '{}' at {}: {}",
                                static_decl.name.name, static_decl.span, e
                            )
                        });
                    let mut description = DataDescription::new();
                    description.define_zeroinit(sty.size().max(1));
                    module
                        .define_data(data_id, &description)
                        .unwrap_or_else(|e| {
                            panic!(
                                "cannot define static '{}' at {}: {}",
                                static_decl.name.name, static_decl.span, e
                            )
                        });
                    ms_ctx.globals.insert(
                        static_decl.name.name.clone().into_boxed_str(),
                        crate::ms::MsGlobal {
                            data_id,
                            ty_id: ty.id,
                            is_const: static_decl.is_const,
                        },
                    );
                }
                (MsType::Struct(_), mantis_parser::ast::Expr::StructInit { .. }) => {
                    panic!("static struct '{}' at {} currently requires an empty compile-time initializer", static_decl.name.name, static_decl.span);
                }
                _ => panic!(
                    "initializer for static '{}' at {} is not a supported compile-time constant",
                    static_decl.name.name,
                    static_decl.value.span()
                ),
            }
        }
    }

    // Pass 2: Pre-declare non-generic function signatures
    for declaration in &declarations {
        if let Declaration::Function(function_decl) = declaration {
            let is_generic = function_decl
                .name
                .as_ref()
                .map_or(false, |n| matches!(n, TypeExpr::Generic(_, _)))
                || !function_decl.where_clause.is_empty()
                || (!function_decl.is_extern
                    && function_decl
                        .params
                        .iter()
                        .any(|p| matches!(p.ty, TypeExpr::Unknown)));
            if !is_generic {
                let mut proto = function_decl.clone();
                proto.body = None;
                compile_function(
                    proto,
                    &mut module,
                    &mut ctx,
                    &mut fbx,
                    &mut ms_ctx,
                    None,
                    None,
                );
            }
        }
    }

    // Pass 3: Process function definitions and impl blocks
    for declaration in declarations {
        match declaration {
            Declaration::Function(function_decl) => {
                let mut auto_generics = Vec::new();
                let mut function_decl = function_decl;
                let mut was_explicit_generic = false;

                if let Some(TypeExpr::Generic(_, _)) = &function_decl.name {
                    was_explicit_generic = true;
                } else if !function_decl.where_clause.is_empty() {
                    was_explicit_generic = true;
                }

                if !function_decl.is_extern {
                    for param in function_decl.params.iter_mut() {
                        if matches!(param.ty, TypeExpr::Unknown) {
                            let gen_name = format!("_{}", param.name.name);
                            param.ty = TypeExpr::Named(mantis_parser::ast::Ident::new(
                                &gen_name, param.span,
                            ));
                            auto_generics.push(gen_name.into_boxed_str());
                        }
                    }
                }

                if was_explicit_generic || !auto_generics.is_empty() {
                    let (name, generics) =
                        if let Some(TypeExpr::Generic(base, generics)) = &function_decl.name {
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
            Declaration::TypeDef(_) => {}
            Declaration::Use(_) | Declaration::Import(_) => {}
            Declaration::Trait(_) => {}
            Declaration::Static(_) => {}
            Declaration::Impl(impl_block) => {
                if impl_block.generics.is_empty() {
                    let for_type = if let Some(ref for_ty) = impl_block.for_type {
                        ms_ctx.current_module.resolve(for_ty).unwrap().ty().unwrap()
                    } else {
                        ms_ctx
                            .current_module
                            .resolve(&impl_block.trait_name)
                            .unwrap()
                            .ty()
                            .unwrap()
                    };

                    ms_ctx.current_module.add_alias(
                        TypeNameWithGenerics::new("Self".into(), vec![]),
                        for_type.clone(),
                    );

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

                    let for_type_node = impl_block
                        .for_type
                        .as_ref()
                        .unwrap_or(&impl_block.trait_name);

                    for mut function in impl_block.methods {
                        for param in function.params.iter_mut() {
                            if param.name.name.as_str() == "self"
                                && matches!(param.ty, TypeExpr::Unknown)
                            {
                                param.ty = for_type_node.clone();
                            }
                        }

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

                    let for_type_node = impl_block
                        .for_type
                        .as_ref()
                        .unwrap_or(&impl_block.trait_name);

                    for mut function in impl_block.methods {
                        for param in function.params.iter_mut() {
                            if param.name.name.as_str() == "self"
                                && matches!(param.ty, TypeExpr::Unknown)
                            {
                                param.ty = for_type_node.clone();
                            }
                        }

                        let name_expr = function.name.as_ref().unwrap();
                        let name = name_expr
                            .as_name()
                            .or_else(|| {
                                if let TypeExpr::Generic(base, _) = name_expr {
                                    base.as_name()
                                } else {
                                    None
                                }
                            })
                            .expect(&format!("method name error in {:?}", name_expr));
                        let func_name: Box<str> = name.into();
                        let template = MsGenericFunction {
                            decl: Rc::new(function),
                            generics: generics.clone(),
                        };

                        let for_type_node = impl_block
                            .for_type
                            .as_ref()
                            .unwrap_or(&impl_block.trait_name);
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
                    ms_ctx
                        .current_module
                        .add_alias(TypeNameWithGenerics::new(name.clone(), vec![]), ty);
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
    println!("Finished compile_binary, wrote {} bytes", bytes.len());
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
        .declare_function("main", Linkage::Export, &ctx.func.signature)
        .unwrap();
}
