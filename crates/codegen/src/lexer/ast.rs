use std::{collections::HashMap, rc::Rc};

use cranelift::{
    codegen::entity::EntityRef, frontend::FunctionBuilder, prelude::Variable as CVariable,
    prelude::*,
};

use crate::utils::{rc_str::RcStr, rc_vec::RcVec};

pub struct Program {
    pub types: RcVec<Type>,
    pub functions: RcVec<FunctionDeclaration>,
}

impl Program {
    pub fn new(
        types: impl Into<RcVec<Type>>,
        functions: impl Into<RcVec<FunctionDeclaration>>,
    ) -> Self {
        Self {
            types: types.into(),
            functions: functions.into(),
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct Variable {
    pub name: RcStr,
    pub r#type: Type,
    pub is_mutable: bool,
}

#[derive(Clone, PartialEq, PartialOrd)]
pub struct Type {
    pub name: String,
    pub size: usize,
    pub inner_type: Option<Box<Type>>,
}

impl Type {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            size: 8,
            inner_type: None,
        }
    }
}

#[derive(Clone)]
pub struct FunctionSignature {
    pub name: RcStr,
    pub args: RcVec<Type>,
    pub return_type: Type,
}

impl FunctionSignature {
    pub fn new(name: impl Into<RcStr>, args: impl Into<RcVec<Type>>, return_type: Type) -> Self {
        Self {
            name: name.into(),
            args: args.into(),
            return_type,
        }
    }
}

#[derive(Clone)]
pub struct FunctionDeclaration {
    pub signature: FunctionSignature,
    pub statements: RcVec<Expression>,
}

impl FunctionDeclaration {
    pub fn new(signature: FunctionSignature, statements: impl Into<RcVec<Expression>>) -> Self {
        Self {
            signature: signature.into(),
            statements: statements.into(),
        }
    }
}

pub struct Function {}

pub struct BlockScope {
    pub variables: RcVec<Variable>,
    pub expression: RcVec<Expression>,
}

#[derive(Clone)]
pub struct AssignExpression {
    pub lhs: Variable,
    pub rhs: Variable,
}

#[derive(Clone)]
pub struct MethodCallExpression {
    pub callee: Variable,
    pub method: FunctionSignature,
    pub args: RcVec<Variable>,
}

#[derive(Clone)]
pub struct FunctionCallExpression {
    pub function: FunctionSignature,
    pub args: RcVec<Variable>,
}

#[derive(Clone)]
pub struct BinaryExpression {
    pub lhs: Variable,
    pub rhs: Variable,
    pub operation: Operation,
}

#[derive(Clone, Copy)]
pub enum Operation {
    Assign,
    Add,
    Sub,
    Mul,
    Div,
    LogicAnd,
    LogicOr,
}

#[derive(Clone)]
pub enum Expression {
    Assign(Variable, Variable),
    Binary(Variable, Operation, Variable),
    FunctionCall(FunctionCallExpression),
}

impl Expression {
    pub fn compile(
        &self,
        mut builder: FunctionBuilder,
        mut variables: HashMap<RcStr, (Variable, CVariable)>,
    ) -> anyhow::Result<()> {
        match self {
            Expression::Assign(lhs, rhs) => {
                if let Some(var) = variables.get_mut(&lhs.name) {
                } else {
                    let var = CVariable::new(1);
                    let val = Value::new(1);
                    variables.insert(lhs.name.clone(), (lhs.clone(), var));
                    builder.def_var(var, val);
                }
            }
            Expression::Binary(_, _, _) => todo!(),
            Expression::FunctionCall(_) => todo!(),
        }
        Ok(())
    }
}
