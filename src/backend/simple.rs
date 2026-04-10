use cranelift::prelude::*;
use cranelift_module::{default_libcall_names, DataDescription, DataId, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};
use mantis_parser::ast::{
    Block, BlockItem, Declaration, Expr, FieldInit, Program, Statement,
    TypeDef, TypeDefBody, TypeExpr,
};
use std::collections::HashMap;

use super::scope_env::{
    ScopeEnv, StructFieldInfo, StructLayout, TypeRegistry, VarInfo,
};

/// Resolve a type name string to its Cranelift IR type.
/// For now, only primitive numeric types are supported.
fn resolve_type_name_to_cl_type(name: &str) -> types::Type {
    match name {
        "i8" | "u8" => types::I8,
        "i16" | "u16" => types::I16,
        "i32" | "u32" | "f32" => types::I32,
        "i64" | "u64" | "f64" => types::I64,
        "bool" => types::I8,
        // Struct types and pointers are represented as I64 (pointer-sized).
        _ => types::I64,
    }
}

/// Resolve a TypeExpr to a Cranelift type, consulting the type registry for struct types.
fn resolve_type_expr_cl_type(ty: &TypeExpr, type_registry: &TypeRegistry) -> types::Type {
    match ty {
        TypeExpr::Named(ident) => {
            let name = ident.name.as_str();
            // If it's a known struct, it's represented as a pointer (I64).
            if type_registry.get_struct(name).is_some() {
                types::I64
            } else {
                resolve_type_name_to_cl_type(name)
            }
        }
        TypeExpr::Generic(_, _) => types::I64, // generics treated as pointer-sized for now
        TypeExpr::Ref(_, _) => types::I64,     // references are pointers
        _ => types::I64,
    }
}

/// Extract the struct type name from a TypeExpr if it names a known struct.
fn struct_type_name_from_expr(ty: &TypeExpr, type_registry: &TypeRegistry) -> Option<String> {
    match ty {
        TypeExpr::Named(ident) => {
            if type_registry.get_struct(&ident.name).is_some() {
                Some(ident.name.clone())
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Register all struct type definitions from the program into the type registry.
fn register_types(program: &Program, type_registry: &mut TypeRegistry) {
    for decl in &program.declarations {
        if let Declaration::TypeDef(TypeDef {
            name,
            definition: TypeDefBody::Struct(struct_def),
            ..
        }) = decl
        {
            let type_name = match name {
                TypeExpr::Named(ident) => ident.name.clone(),
                TypeExpr::Generic(base, _) => {
                    // For generic structs, register the base name (un-monomorphized).
                    base.as_name().unwrap_or("unknown").to_string()
                }
                _ => continue,
            };

            let mut fields = Vec::new();
            let mut offset: u32 = 0;
            for param in &struct_def.fields {
                let cl_type = resolve_type_expr_cl_type(&param.ty, type_registry);
                let size = cl_type.bytes();
                let stn = struct_type_name_from_expr(&param.ty, type_registry);
                fields.push(StructFieldInfo {
                    name: param.name.name.clone(),
                    cl_type,
                    offset,
                    struct_type_name: stn,
                });
                offset += size;
            }

            let layout = StructLayout {
                fields,
                total_size: offset,
            };
            log::info!("Registered struct type '{}': {:?}", type_name, layout);
            type_registry.register_struct(type_name, layout);
        }
    }
}

pub fn compile_binary(program: Program, module_name: &str) -> anyhow::Result<Vec<u8>> {
    let mut flag_builder = settings::builder();
    flag_builder.set("is_pic", "true");
    let isa_builder = cranelift_native::builder().map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let isa = isa_builder
        .finish(settings::Flags::new(flag_builder))
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    let builder = ObjectBuilder::new(isa, module_name, default_libcall_names())
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let mut module = ObjectModule::new(builder);

    let mut ctx = module.make_context();
    let mut builder_context = FunctionBuilderContext::new();

    let mut functions = HashMap::new();
    let mut data_objects = HashMap::new();

    // ── Pass 0: register struct types ────────────────────────────────────
    let mut type_registry = TypeRegistry::new();
    register_types(&program, &mut type_registry);

    // Register implicit 'malloc' for #init
    let mut malloc_sig = module.make_signature();
    malloc_sig.params.push(AbiParam::new(types::I64));
    malloc_sig.returns.push(AbiParam::new(types::I64));
    let malloc_id = module.declare_function("malloc", Linkage::Import, &malloc_sig)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    functions.insert("malloc".to_string(), malloc_id);

    // ── Pass 1: declare all functions ────────────────────────────────────
    for decl in &program.declarations {
        if let Declaration::Function(f) = decl {
            let mut sig = module.make_signature();
            for _ in &f.params {
                sig.params.push(AbiParam::new(types::I64));
            }
            let is_main = f.name.as_ref().unwrap().as_name() == Some("main");
            if is_main {
                sig.returns.push(AbiParam::new(types::I32));
            } else if let Some(_) = &f.return_type {
                sig.returns.push(AbiParam::new(types::I64));
            }

            let name = f.name.as_ref().unwrap().as_name().unwrap();
            let linkage = if f.is_extern {
                Linkage::Import
            } else {
                Linkage::Export
            };
            let func_id = module
                .declare_function(name, linkage, &sig)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            functions.insert(name.to_string(), func_id);
        }
    }

    for decl in &program.declarations {
        if let Declaration::Function(f) = decl {
            let name = f.name.as_ref().unwrap().as_name().unwrap();
            println!("Compiling function: {}, is_extern: {}", name, f.is_extern);

            if f.is_extern {
                continue;
            }
            let func_id = functions[name];

            ctx.func.signature.params.clear();
            ctx.func.signature.returns.clear();
            for _ in &f.params {
                ctx.func.signature.params.push(AbiParam::new(types::I64));
            }
            let is_main = name == "main";
            if is_main {
                ctx.func.signature.returns.push(AbiParam::new(types::I32));
            } else if let Some(_) = &f.return_type {
                ctx.func.signature.returns.push(AbiParam::new(types::I64));
            }

            let mut builder = FunctionBuilder::new(&mut ctx.func, &mut builder_context);
            let entry_block = builder.create_block();
            builder.append_block_params_for_function_params(entry_block);
            builder.switch_to_block(entry_block);

            let mut env = ScopeEnv::new();
            let block_params = builder.block_params(entry_block).to_vec();
            for (i, param) in f.params.iter().enumerate() {
                let var = builder.declare_var(types::I64);
                builder.def_var(var, block_params[i]);
                let info = VarInfo::new(var, types::I64, param.mutable);
                env.insert_var_shadow(param.name.name.clone(), info);
            }

            if let Some(body) = &f.body {
                let has_returned = compile_block(
                    &mut builder,
                    &mut module,
                    body,
                    &functions,
                    &mut data_objects,
                    &mut env,
                    is_main,
                    &type_registry,
                )?;
                if !has_returned {
                    if is_main {
                        let zero = builder.ins().iconst(types::I32, 0);
                        builder.ins().return_(&[zero]);
                    } else {
                        let zero = builder.ins().iconst(types::I64, 0);
                        builder.ins().return_(&[zero]);
                    }
                }
            } else {
                if is_main {
                    let zero = builder.ins().iconst(types::I32, 0);
                    builder.ins().return_(&[zero]);
                } else {
                    let zero = builder.ins().iconst(types::I64, 0);
                    builder.ins().return_(&[zero]);
                }
            }

            builder.seal_all_blocks();
            builder.finalize();
            module
                .define_function(func_id, &mut ctx)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            module.clear_context(&mut ctx);
        }
    }

    let object_product = module.finish();
    Ok(object_product.emit()?)
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Block compilation
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn compile_block(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    block: &Block,
    functions: &HashMap<String, cranelift_module::FuncId>,
    data_objects: &mut HashMap<String, DataId>,
    env: &mut ScopeEnv,
    is_main: bool,
    type_registry: &TypeRegistry,
) -> anyhow::Result<bool> {
    let mut has_returned = false;
    for item in &block.items {
        if has_returned {
            break;
        }
        match item {
            BlockItem::Statement(Statement::Let { name, value, mutable, ty, .. }) => {
                let val = compile_expr(builder, module, value, functions, data_objects, env, type_registry)?;
                let cl_ty = builder.func.dfg.value_type(val);
                let var = builder.declare_var(cl_ty);
                builder.def_var(var, val);

                // Determine if this variable holds a struct pointer.
                let struct_type_name = detect_struct_type(value, ty, type_registry);
                let info = if let Some(stn) = struct_type_name {
                    VarInfo::new_struct(var, *mutable, stn)
                } else {
                    VarInfo::new(var, cl_ty, *mutable)
                };
                env.insert_var_shadow(name.name.clone(), info);
            }
            BlockItem::Statement(Statement::Expr { expr, .. }) => {
                if let Expr::Binary {
                    op: mantis_parser::ast::BinOp::Assign,
                    lhs,
                    rhs,
                    ..
                } = expr
                {
                    // ── Field assignment: `a.b = expr` ──────────────────
                    if let Expr::Field { .. } = &**lhs {
                        compile_field_assign(builder, module, lhs, rhs, functions, data_objects, env, type_registry)?;
                        continue;
                    }

                    // ── Simple variable assignment: `a = expr` ──────────
                    if let Expr::Ident(id) = &**lhs {
                        let var_info = env.resolve_mut(&id.name)
                            .map_err(|e| anyhow::anyhow!("{}", e))?;
                        let var = var_info.variable;
                        let val =
                            compile_expr(builder, module, rhs, functions, data_objects, env, type_registry)?;
                        builder.def_var(var, val);
                        continue;
                    }
                }
                compile_expr(builder, module, expr, functions, data_objects, env, type_registry)?;
            }
            BlockItem::Statement(Statement::Return { value, .. }) => {
                let ret_val = if let Some(expr) = value {
                    compile_expr(builder, module, expr, functions, data_objects, env, type_registry)?
                } else {
                    if is_main {
                        builder.ins().iconst(types::I32, 0)
                    } else {
                        builder.ins().iconst(types::I64, 0)
                    }
                };

                let ret_cast = if is_main && builder.func.dfg.value_type(ret_val) == types::I64 {
                    builder.ins().ireduce(types::I32, ret_val)
                } else {
                    ret_val
                };
                builder.ins().return_(&[ret_cast]);
                has_returned = true;
            }
            BlockItem::Statement(Statement::Break { .. }) => {
                let tgt = env.current_break_target()
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
                builder.ins().jump(tgt, &[]);
                has_returned = true;
            }
            BlockItem::Statement(Statement::Continue { .. }) => {
                let tgt = env.current_continue_target()
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
                builder.ins().jump(tgt, &[]);
                has_returned = true;
            }
            BlockItem::IfChain(if_chain) => {
                let end_block = builder.create_block();

                let mut arms = Vec::new();
                arms.push(&if_chain.if_block);
                for elif in &if_chain.elif_blocks {
                    arms.push(elif);
                }

                for arm in arms.iter() {
                    let cond_val = compile_expr(
                        builder,
                        module,
                        &arm.condition,
                        functions,
                        data_objects,
                        env,
                        type_registry,
                    )?;

                    let then_block = builder.create_block();
                    let else_block = builder.create_block();

                    builder
                        .ins()
                        .brif(cond_val, then_block, &[], else_block, &[]);

                    builder.switch_to_block(then_block);
                    env.enter_scope();
                    let ret = compile_block(
                        builder,
                        module,
                        &arm.body,
                        functions,
                        data_objects,
                        env,
                        is_main,
                        type_registry,
                    )?;
                    env.exit_scope();
                    if !ret {
                        builder.ins().jump(end_block, &[]);
                    }

                    builder.switch_to_block(else_block);
                }

                if let Some(else_body) = &if_chain.else_block {
                    env.enter_scope();
                    let ret = compile_block(
                        builder,
                        module,
                        else_body,
                        functions,
                        data_objects,
                        env,
                        is_main,
                        type_registry,
                    )?;
                    env.exit_scope();
                    if !ret {
                        builder.ins().jump(end_block, &[]);
                    }
                } else {
                    builder.ins().jump(end_block, &[]);
                }

                builder.switch_to_block(end_block);
            }
            BlockItem::Loop(loop_block) => {
                let header_block = builder.create_block();
                let body_block = builder.create_block();
                let exit_block = builder.create_block();

                builder.ins().jump(header_block, &[]);

                builder.switch_to_block(header_block);
                builder.ins().jump(body_block, &[]);

                builder.switch_to_block(body_block);

                env.continue_targets.push(header_block);
                env.break_targets.push(exit_block);
                env.enter_scope();
                let ret = compile_block(
                    builder,
                    module,
                    &loop_block.body,
                    functions,
                    data_objects,
                    env,
                    is_main,
                    type_registry,
                )?;
                env.exit_scope();
                env.break_targets.pop();
                env.continue_targets.pop();

                if !ret {
                    builder.ins().jump(header_block, &[]);
                }

                builder.switch_to_block(exit_block);
            }
            BlockItem::Block(b) => {
                env.enter_scope();
                let ret = compile_block(builder, module, b, functions, data_objects, env, is_main, type_registry)?;
                env.exit_scope();
                if ret {
                    has_returned = true;
                }
            }
            BlockItem::Match(_) => {
                return Err(anyhow::anyhow!("Match unsupported in simple backend"));
            }
        }
    }
    Ok(has_returned)
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Helpers: struct type detection and field assignment
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Detect whether an expression produces a struct value based on:
/// 1. The init expression being a StructInit, or
/// 2. An explicit type annotation naming a known struct.
fn detect_struct_type(
    value: &Expr,
    ty_annotation: &Option<TypeExpr>,
    type_registry: &TypeRegistry,
) -> Option<String> {
    // From the init expression itself.
    if let Expr::StructInit { ty, .. } = value {
        if let Some(name) = ty.as_name() {
            if type_registry.get_struct(name).is_some() {
                return Some(name.to_string());
            }
        }
    }
    // From a CompilerCall like #init(Foo)
    if let Expr::CompilerCall { name, args, .. } = value {
        if name == "init" {
            if let Some(arg) = args.first() {
                if let Expr::Ident(id) = arg {
                    if type_registry.get_struct(&id.name).is_some() {
                        return Some(id.name.clone());
                    }
                }
                // Handle TypeExpr if the parser wraps in TypeExpr
                if let Expr::TypeExpr(TypeExpr::Named(id)) = arg {
                    if type_registry.get_struct(&id.name).is_some() {
                        return Some(id.name.clone());
                    }
                }
            }
        }
    }
    // From an explicit type annotation.
    if let Some(ty_expr) = ty_annotation {
        if let Some(name) = ty_expr.as_name() {
            if type_registry.get_struct(name).is_some() {
                return Some(name.to_string());
            }
        }
    }
    None
}

/// Resolve a field access chain (`a.b.c`) to its (pointer, field_info) at the leaf.
/// Returns the cranelift Value holding the pointer to the field, plus the StructFieldInfo
/// so the caller knows the type to load/store.
fn resolve_field_chain(
    builder: &mut FunctionBuilder,
    expr: &Expr,
    env: &ScopeEnv,
    type_registry: &TypeRegistry,
) -> anyhow::Result<(Value, StructFieldInfo)> {
    match expr {
        Expr::Field { object, field, .. } => {
            // Recursively resolve the object part.
            let (parent_ptr, parent_struct_name) = resolve_object_ptr(builder, object, env, type_registry)?;
            let layout = type_registry.get_struct(&parent_struct_name)
                .ok_or_else(|| anyhow::anyhow!("'{}' is not a struct type", parent_struct_name))?;
            let field_info = layout.get_field(&field.name)
                .ok_or_else(|| anyhow::anyhow!("struct '{}' has no field '{}'", parent_struct_name, field.name))?
                .clone();

            let field_ptr = builder.ins().iadd_imm(parent_ptr, field_info.offset as i64);
            Ok((field_ptr, field_info))
        }
        _ => Err(anyhow::anyhow!("Expected field access expression, got {:?}", expr)),
    }
}

/// Resolve the "object" part of a field access to a (pointer Value, struct type name).
/// Handles both simple identifiers and nested field access (e.g. `a.b` in `a.b.c`).
fn resolve_object_ptr(
    builder: &mut FunctionBuilder,
    expr: &Expr,
    env: &ScopeEnv,
    type_registry: &TypeRegistry,
) -> anyhow::Result<(Value, String)> {
    match expr {
        Expr::Ident(id) => {
            let var_info = env.resolve(&id.name)
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            let struct_name = var_info.struct_type_name.as_ref()
                .ok_or_else(|| anyhow::anyhow!("variable '{}' is not a struct", id.name))?
                .clone();
            let ptr = builder.use_var(var_info.variable);
            Ok((ptr, struct_name))
        }
        Expr::Field { .. } => {
            // Nested field: resolve the chain, get the pointer, and determine the
            // struct type of the resulting field.
            let (field_ptr, field_info) = resolve_field_chain(builder, expr, env, type_registry)?;
            let nested_struct_name = field_info.struct_type_name
                .ok_or_else(|| anyhow::anyhow!("field '{}' is not a struct type", field_info.name))?;
                
            // Since struct fields store a pointer to the nested struct, we need to load it.
            let inner_ptr = builder.ins().load(types::I64, MemFlags::new(), field_ptr, 0);
            Ok((inner_ptr, nested_struct_name))
        }
        _ => Err(anyhow::anyhow!("Expected identifier or field access in object position, got {:?}", expr)),
    }
}

/// Compile a field assignment: `lhs.field = rhs`
fn compile_field_assign(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    lhs: &Expr,
    rhs: &Expr,
    functions: &HashMap<String, cranelift_module::FuncId>,
    data_objects: &mut HashMap<String, DataId>,
    env: &ScopeEnv,
    type_registry: &TypeRegistry,
) -> anyhow::Result<()> {
    let (field_ptr, field_info) = resolve_field_chain(builder, lhs, env, type_registry)?;
    let val = compile_expr(builder, module, rhs, functions, data_objects, env, type_registry)?;
    builder.ins().store(MemFlags::new(), val, field_ptr, 0);
    Ok(())
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Expression compilation
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn compile_expr(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    expr: &Expr,
    functions: &HashMap<String, cranelift_module::FuncId>,
    data_objects: &mut HashMap<String, DataId>,
    env: &ScopeEnv,
    type_registry: &TypeRegistry,
) -> anyhow::Result<Value> {
    match expr {
        Expr::IntLit { value, .. } => Ok(builder.ins().iconst(types::I64, *value)),
        Expr::FloatLit { value, .. } => {
            let float_bytes = value.to_bits();
            let bits = builder.ins().iconst(types::I64, float_bytes as i64);
            Ok(builder.ins().bitcast(types::F64, MemFlags::new(), bits))
        }
        Expr::CharLit { value, .. } => Ok(builder.ins().iconst(types::I64, *value as i64)),
        Expr::BoolLit { value, .. } => {
            Ok(builder.ins().iconst(types::I64, if *value { 1 } else { 0 }))
        }
        Expr::Ident(id) => {
            let var_info = env.resolve(&id.name)
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            Ok(builder.use_var(var_info.variable))
        }
        Expr::Cast { expr, ty, .. } => {
            let val = compile_expr(builder, module, expr, functions, data_objects, env, type_registry)?;
            let to_type_name = ty.as_name().unwrap_or("i64");
            let from_type = builder.func.dfg.value_type(val);
            if to_type_name == "i32" && from_type == types::I64 {
                Ok(builder.ins().ireduce(types::I32, val))
            } else if to_type_name == "i64" && from_type == types::I32 {
                Ok(builder.ins().sextend(types::I64, val))
            } else {
                Ok(val)
            }
        }
        Expr::Binary { op, lhs, rhs, .. } => {
            let lhs_val = compile_expr(builder, module, lhs, functions, data_objects, env, type_registry)?;
            let rhs_val = compile_expr(builder, module, rhs, functions, data_objects, env, type_registry)?;
            use mantis_parser::ast::BinOp;
            match op {
                BinOp::Add => Ok(builder.ins().iadd(lhs_val, rhs_val)),
                BinOp::Sub => Ok(builder.ins().isub(lhs_val, rhs_val)),
                BinOp::Mul => Ok(builder.ins().imul(lhs_val, rhs_val)),
                BinOp::Div => Ok(builder.ins().sdiv(lhs_val, rhs_val)),
                BinOp::Mod => Ok(builder.ins().srem(lhs_val, rhs_val)),
                BinOp::Eq => {
                    let flag = builder.ins().icmp(
                        cranelift::codegen::ir::condcodes::IntCC::Equal,
                        lhs_val,
                        rhs_val,
                    );
                    Ok(builder.ins().uextend(types::I64, flag))
                }
                BinOp::NotEq => {
                    let flag = builder.ins().icmp(
                        cranelift::codegen::ir::condcodes::IntCC::NotEqual,
                        lhs_val,
                        rhs_val,
                    );
                    Ok(builder.ins().uextend(types::I64, flag))
                }
                BinOp::Gt => {
                    let flag = builder.ins().icmp(
                        cranelift::codegen::ir::condcodes::IntCC::SignedGreaterThan,
                        lhs_val,
                        rhs_val,
                    );
                    Ok(builder.ins().uextend(types::I64, flag))
                }
                BinOp::Lt => {
                    let flag = builder.ins().icmp(
                        cranelift::codegen::ir::condcodes::IntCC::SignedLessThan,
                        lhs_val,
                        rhs_val,
                    );
                    Ok(builder.ins().uextend(types::I64, flag))
                }
                BinOp::GtEq => {
                    let flag = builder.ins().icmp(
                        cranelift::codegen::ir::condcodes::IntCC::SignedGreaterThanOrEqual,
                        lhs_val,
                        rhs_val,
                    );
                    Ok(builder.ins().uextend(types::I64, flag))
                }
                BinOp::LtEq => {
                    let flag = builder.ins().icmp(
                        cranelift::codegen::ir::condcodes::IntCC::SignedLessThanOrEqual,
                        lhs_val,
                        rhs_val,
                    );
                    Ok(builder.ins().uextend(types::I64, flag))
                }
                BinOp::Shr => Ok(builder.ins().sshr(lhs_val, rhs_val)),
                BinOp::Shl => Ok(builder.ins().ishl(lhs_val, rhs_val)),
                BinOp::BitAnd => Ok(builder.ins().band(lhs_val, rhs_val)),
                BinOp::BitOr => Ok(builder.ins().bor(lhs_val, rhs_val)),
                BinOp::BitXor => Ok(builder.ins().bxor(lhs_val, rhs_val)),
                _ => Err(anyhow::anyhow!("Unsupported binary op {:?}", op)),
            }
        }
        Expr::StringLit { value, .. } => {
            let data_id = if let Some(&id) = data_objects.get(value) {
                id
            } else {
                let name = format!("str_{}", data_objects.len());
                let id = module
                    .declare_data(&name, Linkage::Local, false, false)
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
                let mut desc = DataDescription::new();

                let mut data = value.as_bytes().to_vec();
                data.push(0);
                desc.define(data.into_boxed_slice());
                module
                    .define_data(id, &desc)
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
                data_objects.insert(value.clone(), id);
                id
            };

            let global = module.declare_data_in_func(data_id, builder.func);
            Ok(builder
                .ins()
                .symbol_value(module.isa().pointer_type(), global))
        }
        Expr::Call { callee, args, .. } => {
            let Expr::Ident(ref id) = **callee else {
                return Err(anyhow::anyhow!("Callee must be ident"));
            };
            let func_id = functions
                .get(&id.name)
                .ok_or_else(|| anyhow::anyhow!("Function not found: {}", id.name))?;
            let local_func = module.declare_func_in_func(*func_id, builder.func);

            let mut arg_values = Vec::new();
            for arg in args {
                arg_values.push(compile_expr(
                    builder,
                    module,
                    arg,
                    functions,
                    data_objects,
                    env,
                    type_registry,
                )?);
            }

            let call = builder.ins().call(local_func, &arg_values);
            let results = builder.inst_results(call);
            if results.is_empty() {
                Ok(builder.ins().iconst(types::I64, 0))
            } else {
                Ok(results[0])
            }
        }

        // ── Compiler calls ───────────────────────────────────────────────
        Expr::CompilerCall { name, args, .. } => {
            if name == "init" {
                if let Some(arg) = args.first() {
                    let type_name = if let Expr::Ident(id) = arg {
                        &id.name
                    } else if let Expr::TypeExpr(TypeExpr::Named(id)) = arg {
                        &id.name
                    } else {
                        return Err(anyhow::anyhow!("Expected type ident for #init"));
                    };

                    let layout = type_registry.get_struct(type_name)
                        .ok_or_else(|| anyhow::anyhow!("Unknown struct type '{}'", type_name))?;

                    let malloc_id = functions.get("malloc")
                        .ok_or_else(|| anyhow::anyhow!("malloc missing"))?;
                    let local_malloc = module.declare_func_in_func(*malloc_id, builder.func);
                    
                    let size_val = builder.ins().iconst(types::I64, layout.total_size as i64);
                    let call = builder.ins().call(local_malloc, &[size_val]);
                    return Ok(builder.inst_results(call)[0]);
                }
            }
            Err(anyhow::anyhow!("Unsupported compiler call {}", name))
        }

        // ── Struct initialization ────────────────────────────────────────
        // `Point { x: 10, y: 20 }`
        // Allocates a stack slot of the struct's total size, stores each
        // field value at its computed offset, and returns a pointer (i64).
        Expr::StructInit { ty, fields, .. } => {
            let type_name = ty.as_name()
                .ok_or_else(|| anyhow::anyhow!("Struct init requires a named type, got {:?}", ty))?;
            let layout = type_registry.get_struct(type_name)
                .ok_or_else(|| anyhow::anyhow!("Unknown struct type '{}'", type_name))?
                .clone();

            // Allocate a stack slot for the struct.
            let slot = builder.create_sized_stack_slot(StackSlotData::new(
                StackSlotKind::ExplicitSlot,
                layout.total_size,
                0,
            ));
            let base_ptr = builder.ins().stack_addr(types::I64, slot, 0);

            // Store each field at its offset.
            for field_init in fields {
                let field_name = &field_init.name.name;
                let field_info = layout.get_field(field_name)
                    .ok_or_else(|| anyhow::anyhow!(
                        "struct '{}' has no field '{}'", type_name, field_name
                    ))?;
                let val = compile_expr(
                    builder, module, &field_init.value,
                    functions, data_objects, env, type_registry,
                )?;
                let field_ptr = builder.ins().iadd_imm(base_ptr, field_info.offset as i64);
                builder.ins().store(MemFlags::new(), val, field_ptr, 0);
            }

            Ok(base_ptr)
        }

        // ── Field access ─────────────────────────────────────────────────
        // `p.x` — load the value from the struct pointer + field offset.
        Expr::Field { .. } => {
            let (field_ptr, field_info) = resolve_field_chain(builder, expr, env, type_registry)?;

            // If the field is itself a struct, load the inner struct pointer.
            if field_info.struct_type_name.is_some() {
                Ok(builder.ins().load(types::I64, MemFlags::new(), field_ptr, 0))
            } else {
                // Load the primitive value from memory.
                let val = builder.ins().load(field_info.cl_type, MemFlags::new(), field_ptr, 0);
                // If the field type is smaller than I64, extend it so the rest of
                // the compiler (which treats everything as I64) works uniformly.
                if field_info.cl_type.bits() < 64 {
                    Ok(builder.ins().sextend(types::I64, val))
                } else {
                    Ok(val)
                }
            }
        }

        // ── Unary operations ─────────────────────────────────────────────
        Expr::Unary { op, operand, .. } => {
            let val = compile_expr(builder, module, operand, functions, data_objects, env, type_registry)?;
            use mantis_parser::ast::UnaryOp;
            match op {
                UnaryOp::Neg => Ok(builder.ins().ineg(val)),
                UnaryOp::Deref => {
                    // Dereference a pointer: load the i64 at the address.
                    Ok(builder.ins().load(types::I64, MemFlags::new(), val, 0))
                }
                UnaryOp::AddrOf => {
                    // For now, addr-of is a no-op (variables are already pointers
                    // for structs; for scalars this needs stack spilling later).
                    Ok(val)
                }
            }
        }

        // ── Pointer assignment ───────────────────────────────────────────
        Expr::PointerAssign { target, value, .. } => {
            let ptr = compile_expr(builder, module, target, functions, data_objects, env, type_registry)?;
            let val = compile_expr(builder, module, value, functions, data_objects, env, type_registry)?;
            builder.ins().store(MemFlags::new(), val, ptr, 0);
            Ok(val)
        }

        _ => Err(anyhow::anyhow!("Unsupported expr {:?}", expr)),
    }
}
