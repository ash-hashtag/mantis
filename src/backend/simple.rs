use cranelift::prelude::*;
use cranelift_module::{default_libcall_names, DataDescription, DataId, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};
use mantis_parser::ast::{Block, BlockItem, Declaration, Expr, Program, Statement};
use std::collections::HashMap;

use super::scope_env::{ScopeEnv, VarInfo};

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

fn compile_block(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    block: &Block,
    functions: &HashMap<String, cranelift_module::FuncId>,
    data_objects: &mut HashMap<String, DataId>,
    env: &mut ScopeEnv,
    is_main: bool,
) -> anyhow::Result<bool> {
    let mut has_returned = false;
    for item in &block.items {
        if has_returned {
            break;
        }
        match item {
            BlockItem::Statement(Statement::Let { name, value, mutable, .. }) => {
                let val = compile_expr(builder, module, value, functions, data_objects, env)?;
                let cl_ty = builder.func.dfg.value_type(val);
                let var = builder.declare_var(cl_ty);
                builder.def_var(var, val);
                let info = VarInfo::new(var, cl_ty, *mutable);
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
                    if let Expr::Ident(id) = &**lhs {
                        let var_info = env.resolve_mut(&id.name)
                            .map_err(|e| anyhow::anyhow!("{}", e))?;
                        let var = var_info.variable;
                        let val =
                            compile_expr(builder, module, rhs, functions, data_objects, env)?;
                        builder.def_var(var, val);
                        continue;
                    }
                }
                compile_expr(builder, module, expr, functions, data_objects, env)?;
            }
            BlockItem::Statement(Statement::Return { value, .. }) => {
                let ret_val = if let Some(expr) = value {
                    compile_expr(builder, module, expr, functions, data_objects, env)?
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
                let ret = compile_block(builder, module, b, functions, data_objects, env, is_main)?;
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

fn compile_expr(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    expr: &Expr,
    functions: &HashMap<String, cranelift_module::FuncId>,
    data_objects: &mut HashMap<String, DataId>,
    env: &ScopeEnv,
) -> anyhow::Result<Value> {
    match expr {
        Expr::IntLit { value, .. } => Ok(builder.ins().iconst(types::I64, *value)),
        Expr::BoolLit { value, .. } => {
            Ok(builder.ins().iconst(types::I64, if *value { 1 } else { 0 }))
        }
        Expr::Ident(id) => {
            let var_info = env.resolve(&id.name)
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            Ok(builder.use_var(var_info.variable))
        }
        Expr::Cast { expr, ty, .. } => {
            let val = compile_expr(builder, module, expr, functions, data_objects, env)?;
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
            let lhs_val = compile_expr(builder, module, lhs, functions, data_objects, env)?;
            let rhs_val = compile_expr(builder, module, rhs, functions, data_objects, env)?;
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
        _ => Err(anyhow::anyhow!("Unsupported expr {:?}", expr)),
    }
}
