use cranelift::prelude::*;
use cranelift_module::{default_libcall_names, DataDescription, Linkage, Module, DataId};
use cranelift_object::{ObjectBuilder, ObjectModule};
use mantis_parser::ast::{Program, Declaration, Expr, Statement, BlockItem};
use std::collections::HashMap;

pub fn compile_binary(program: Program, module_name: &str) -> anyhow::Result<Vec<u8>> {
    let mut flag_builder = settings::builder();
    flag_builder.set("is_pic", "true");
    let isa_builder = cranelift_native::builder().map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let isa = isa_builder.finish(settings::Flags::new(flag_builder)).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    
    let builder = ObjectBuilder::new(isa, module_name, default_libcall_names()).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let mut module = ObjectModule::new(builder);
    
    let mut ctx = module.make_context();
    let mut builder_context = FunctionBuilderContext::new();

    let mut functions = HashMap::new();
    let mut data_objects = HashMap::new();
    
    // First pass: declare all functions
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
            let linkage = if f.is_extern { Linkage::Import } else { Linkage::Export };
            let func_id = module.declare_function(name, linkage, &sig).map_err(|e| anyhow::anyhow!(e.to_string()))?;
            functions.insert(name.to_string(), func_id);
        }
    }

    // Second pass: define functions
    for decl in &program.declarations {
        if let Declaration::Function(f) = decl {
            let name = f.name.as_ref().unwrap().as_name().unwrap();
            println!("Compiling function: {}, is_extern: {}", name, f.is_extern);

            if f.is_extern { continue; }
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
            builder.seal_block(entry_block);
            
            if let Some(body) = &f.body {
                let mut has_returned = false;
                for item in &body.items {
                    if has_returned {
                        break;
                    }
                    if let BlockItem::Statement(Statement::Expr { expr, .. }) = item {
                        compile_expr(&mut builder, &mut module, expr, &functions, &mut data_objects)?;
                    } else if let BlockItem::Statement(Statement::Return { value, .. }) = item {
                        let ret_val = if let Some(expr) = value {
                            compile_expr(&mut builder, &mut module, expr, &functions, &mut data_objects)?
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
                }
                
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

            builder.finalize();
            module.define_function(func_id, &mut ctx).map_err(|e| anyhow::anyhow!(e.to_string()))?;
            module.clear_context(&mut ctx);
        }
    }
    
    let object_product = module.finish();
    Ok(object_product.emit()?)
}

fn compile_expr(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    expr: &Expr,
    functions: &HashMap<String, cranelift_module::FuncId>,
    data_objects: &mut HashMap<String, DataId>,
) -> anyhow::Result<Value> {
    match expr {
        Expr::IntLit { value, .. } => {
            Ok(builder.ins().iconst(types::I64, *value))
        }
        Expr::StringLit { value, .. } => {
            let data_id = if let Some(&id) = data_objects.get(value) {
                id
            } else {
                let name = format!("str_{}", data_objects.len());
                let id = module.declare_data(&name, Linkage::Local, false, false).map_err(|e| anyhow::anyhow!(e.to_string()))?;
                let mut desc = DataDescription::new();
                
                let mut data = value.as_bytes().to_vec();
                data.push(0);
                desc.define(data.into_boxed_slice());
                module.define_data(id, &desc).map_err(|e| anyhow::anyhow!(e.to_string()))?;
                data_objects.insert(value.clone(), id);
                id
            };
            
            let global = module.declare_data_in_func(data_id, builder.func);
            Ok(builder.ins().symbol_value(module.isa().pointer_type(), global))
        }
        Expr::Call { callee, args, .. } => {
            let Expr::Ident(ref id) = **callee else {
                return Err(anyhow::anyhow!("Callee must be ident"));
            };
            let func_id = functions.get(&id.name).ok_or_else(|| anyhow::anyhow!("Function not found: {}", id.name))?;
            let local_func = module.declare_func_in_func(*func_id, builder.func);
            
            let mut arg_values = Vec::new();
            for arg in args {
                arg_values.push(compile_expr(builder, module, arg, functions, data_objects)?);
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
