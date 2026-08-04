use std::collections::{BTreeMap, HashMap};

use cranelift::{
    codegen::{
        ir::{
            types::{I32, I64},
            AbiParam, FuncRef, InstBuilder, Signature, UserExternalName, UserFuncName, Value,
        },
        Context,
    },
    frontend::{FunctionBuilder, FunctionBuilderContext},
};

use crate::utils::{rc_str::RcStr, rc_vec::RcVec};

use super::{
    compile::{compile, MsContext},
    variable::{MsBackendVariable, MsVariable},
};

pub type LocalVariables = BTreeMap<RcStr, MsBackendVariable>;

pub struct FunctionSignature {
    pub name: RcStr,
    pub params: RcVec<MsVariable>,
    pub returns: Option<MsVariable>,
}

pub enum Expression {
    Declare(MsVariable, RcStr),
    Assign(MsVariable, MsVariable),
    Add(MsVariable, MsVariable, MsVariable),
    Sub(MsVariable, MsVariable, MsVariable),
    FunctionCall(Option<MsVariable>, Function, RcVec<MsVariable>),
    Return(MsVariable),
}

impl Expression {
    pub fn translate(
        &self,
        local_variables: &mut LocalVariables,
        fbx: &mut FunctionBuilder,
        ms_ctx: &mut MsContext,
    ) -> anyhow::Result<Option<Value>> {
        match self {
            Expression::Assign(lhs, rhs) => {
                let val = local_variables.get(&rhs.name).unwrap().c_val;
                if let Some(var) = local_variables.get_mut(&lhs.name) {
                    print!("Assigned Varible {} with {}\n", lhs.name, rhs.name);
                    fbx.def_var(var.c_var, val);
                } else {
                    print!("Declaring Varible {} with 0\n", lhs.name);
                    Expression::Declare(lhs.clone(), RcStr::from("0"))
                        .translate(local_variables, fbx, ms_ctx)
                        .unwrap();

                    return self.translate(local_variables, fbx, ms_ctx);
                }
            }
            Expression::Add(variable, lhs, rhs) => {
                if lhs.type_name == rhs.type_name {
                    let val = fbx.ins().iadd(
                        local_variables.get(&lhs.name).unwrap().c_val,
                        local_variables.get(&rhs.name).unwrap().c_val,
                    );
                    if let Some(var) = local_variables.get_mut(&variable.name) {
                        var.take_value(val, fbx);
                    } else {
                        Expression::Declare(variable.clone(), RcStr::from("0")).translate(
                            local_variables,
                            fbx,
                            ms_ctx,
                        )?;

                        if let Some(var) = local_variables.get_mut(&variable.name) {
                            var.take_value(val, fbx);
                        } else {
                            eprint!("Local Variable Not Found {}\n", variable.name);
                        }
                    }
                    return Ok(None);
                } else {
                    panic!("Invalid Types");
                }
            }
            Expression::Sub(variable, lhs, rhs) => {
                if lhs.type_name == rhs.type_name {
                    let val = fbx.ins().isub(
                        local_variables.get(&lhs.name).unwrap().c_val,
                        local_variables.get(&rhs.name).unwrap().c_val,
                    );
                    if let Some(var) = local_variables.get_mut(&variable.name) {
                        var.take_value(val, fbx);
                    } else {
                        Expression::Declare(variable.clone(), RcStr::from("0")).translate(
                            local_variables,
                            fbx,
                            ms_ctx,
                        )?;

                        if let Some(var) = local_variables.get_mut(&variable.name) {
                            var.take_value(val, fbx);
                        } else {
                            eprint!("Local Variable Not Found {}\n", variable.name);
                        }
                    }
                    return Ok(None);
                } else {
                    panic!("Invalid Types");
                }
            }
            Expression::Return(val) => {
                print!("Returning variable {}\n", val.name);
                let val = local_variables.get(&val.name).unwrap().c_val;
                fbx.ins().return_(&[val]);
                return Ok(None);
            }
            Expression::Declare(var, value) => {
                let val: i64 = value.parse().unwrap();
                let val = fbx.ins().iconst(var.backend_type, val);
                var.declare(val, local_variables, fbx);
                return Ok(Some(val));
            }
            Expression::FunctionCall(_, _, args) => {
                if let Some(var) = args.first() {
                    if let Some(val) = local_variables.get(&var.name) {
                        if let Some(print_i64_fn) = ms_ctx
                            .local_functions
                            .map
                            .get(&RcStr::from("print_i64_extern"))
                        {
                            let func_id = print_i64_fn.func_id;
                            let func_ref = FuncRef::from_bits(func_id.as_bits());
                            let values = &[val.c_val];
                            print!("Calling function {:?} {:?}\n", func_ref, values);
                            fbx.ins().call(func_ref, values);
                            return Ok(None);
                        } else {
                            eprint!("Couldn't find print_i64_extern\n");
                        }
                    } else {
                        eprint!(
                            "Couldn't find local variable {:?}\n LocalVariables: {:?} \n",
                            var, local_variables
                        );
                    }
                } else {
                    eprint!("No Args passed\n");
                }
            }
        }

        return Ok(None);
    }
}

pub struct Function {
    pub signature: FunctionSignature,
    pub expressions: RcVec<Expression>,
    pub is_external: bool,
}

impl Function {
    pub fn declare(
        &self,
        ctx: &mut Context,
        fbx: &mut FunctionBuilderContext,
        ms_ctx: &mut MsContext,
    ) -> anyhow::Result<()> {
        ctx.func.signature.params = self
            .signature
            .params
            .iter()
            .map(|x| AbiParam::new(x.backend_type))
            .collect();
        if let Some(return_var) = &self.signature.returns {
            ctx.func
                .signature
                .returns
                .push(AbiParam::new(return_var.backend_type));
        }

        let mut builder = FunctionBuilder::new(&mut ctx.func, fbx);

        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_all_blocks();

        let mut variables = BTreeMap::new();
        let values = builder.block_params(entry_block).to_vec();
        for (value, variable) in std::iter::zip(values, self.signature.params.iter()) {
            variable.declare(value, &mut variables, &mut builder);
        }

        for expression in self.expressions.iter() {
            expression.translate(&mut variables, &mut builder, ms_ctx);
        }

        let mut has_return = false;

        if let Some(expression) = self.expressions.last() {
            match expression {
                Expression::Return(_) => has_return = true,
                _ => {}
            }
        }

        if !has_return {
            builder.ins().return_(&[]);
        }

        builder.finalize();
        Ok(())
    }
}

pub fn add_fn() -> Function {
    let i64_type_name = RcStr::from("i64");
    let a = MsVariable::new("a", i64_type_name.clone(), I64);
    let b = MsVariable::new("b", i64_type_name.clone(), I64);
    let c = MsVariable::new("c", i64_type_name.clone(), I64);
    let signature = FunctionSignature {
        name: "add".into(),
        params: RcVec::from(vec![a.clone(), b.clone()]),
        returns: Some(c.clone()),
    };

    let function_call_exp = Expression::FunctionCall(
        None,
        Function {
            expressions: RcVec::empty(),
            signature: FunctionSignature {
                name: RcStr::from("print_i64_extern"),
                params: RcVec::from(vec![]),
                returns: None,
            },
            is_external: true,
        },
        RcVec::from(vec![c.clone()]),
    );

    let expressions = RcVec::from(vec![
        Expression::Add(c.clone(), a.clone(), b.clone()),
        function_call_exp,
        Expression::Return(c),
    ]);
    Function {
        signature,
        expressions,
        is_external: false,
    }
}
pub fn sub_fn() -> Function {
    let i64_type_name = RcStr::from("i64");
    let a = MsVariable::new("a", i64_type_name.clone(), I64);
    let b = MsVariable::new("b", i64_type_name.clone(), I64);
    let c = MsVariable::new("c", i64_type_name.clone(), I64);
    let signature = FunctionSignature {
        name: "sub".into(),
        params: RcVec::from(vec![a.clone(), b.clone()]),
        returns: Some(c.clone()),
    };

    let expressions = RcVec::from(vec![
        Expression::Sub(c.clone(), a, b),
        Expression::Return(c),
    ]);
    Function {
        signature,
        expressions,
        is_external: false,
    }
}
pub fn print_i64_fn() -> Function {
    let i64_type_name = RcStr::from("i64");
    let a = MsVariable::new("a", i64_type_name.clone(), I64);
    let signature = FunctionSignature {
        name: "print_i64".into(),
        params: RcVec::from(vec![a.clone()]),
        returns: None,
    };

    let expressions = RcVec::empty();
    Function {
        signature,
        expressions,
        is_external: true,
    }
}

pub fn main_fn() -> Function {
    let argc = MsVariable::new("argc", "i32", I32);
    let argv = MsVariable::new("argv", "char*", I64);
    let exit_code = MsVariable::new("exit_code", "i32", I32);
    let signature = FunctionSignature {
        name: "main".into(),
        params: RcVec::from(vec![argc.clone(), argv.clone()]),
        returns: Some(exit_code.clone()),
    };
    let expressions = RcVec::from(vec![
        // Expression::Assign(exit_code.clone(), argc.clone()),
        Expression::Return(argc.clone()),
    ]);
    Function {
        signature,
        expressions,
        is_external: false,
    }
}

pub fn test(out_path: &str) {
    // let functions = vec![print_i64_fn(), add_fn(), sub_fn()];
    let functions = vec![main_fn()];
    let bytes = compile(RcVec::from(functions)).unwrap();

    std::fs::write(out_path, bytes).unwrap();
}
