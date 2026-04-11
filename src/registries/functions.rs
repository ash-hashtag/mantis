use std::{collections::HashMap, rc::Rc};

use cranelift_module::FuncId;
use linear_map::LinearMap;
use mantis_parser::ast::FnDecl as FunctionDecl;

use crate::frontend::tokens::MsFunctionDeclaration;

use super::{
    types::{MsType, MsTypeId, TypeNameWithGenerics},
    MsRegistry, MsRegistryExt,
};

#[derive(Clone, Debug, Copy)]
pub enum FunctionType {
    Extern,
    Private,
    Public,
}

#[derive(Clone, Debug)]
pub struct MsFunctionType {
    pub arguments: Vec<MsType>,
    pub rets: MsType,
    pub fn_type: FunctionType,
}

#[derive(Clone, Debug)]
pub struct MsDeclaredFunction {
    pub arguments: LinearMap<Box<str>, MsTypeId>, // var_name -> type
    pub rets: Option<MsTypeId>,
    pub fn_type: FunctionType,
    pub func_id: FuncId,
}

impl From<&MsFunctionDeclaration> for MsFunctionType {
    fn from(value: &MsFunctionDeclaration) -> Self {
        Self {
            arguments: value.arguments.iter().map(|x| x.var_type.clone()).collect(),
            rets: value.return_type.clone(),
            fn_type: value.fn_type,
        }
    }
}

#[derive(Debug)]
pub struct MsFunctionRegistry {
    pub registry: HashMap<Box<str>, Rc<MsDeclaredFunction>>,
}

impl Default for MsFunctionRegistry {
    fn default() -> Self {
        let registry = HashMap::new();
        Self { registry }
    }
}

impl MsFunctionRegistry {
    pub fn add_function(&mut self, name: impl Into<Box<str>>, func: Rc<MsDeclaredFunction>) {
        let name = name.into();
        if let Some(_existing) = self.registry.get(&name) {
            return;
        }
        log::info!("adding function to registry {}", name);
        self.registry.insert(name, func);
    }
}


#[derive(Debug, Clone)]
pub struct MsGenericFunction {
    pub decl: Rc<FunctionDecl>,
    pub generics: Vec<Box<str>>,
}
impl MsGenericFunction {
    pub(crate) fn generate(&self, _real_types: Vec<super::modules::MsResolved>) -> FunctionDecl {
        todo!()
    }
}

#[derive(Debug, Clone)]
pub struct MsInstantiation {
    pub template: MsGenericFunction,
    pub instantiation_name: String,
    pub real_types: Vec<super::modules::MsResolved>,
}

#[derive(Default, Debug)]
pub struct MsFunctionTemplates {
    pub registry: HashMap<Box<str>, MsGenericFunction>,
}

#[derive(Default, Debug)]
pub struct MsTraitTemplates {
    pub registry: HashMap<Box<str>, Vec<FunctionDecl>>,
}

#[derive(Default, Debug)]
pub struct MsTraitGenericTemplates {
    pub registry: HashMap<Box<str>, Vec<MsGenericFunction>>,
}
