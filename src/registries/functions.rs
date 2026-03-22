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
    pub fn add_function(&mut self, name: impl Into<Box<str>>, decl: Rc<MsDeclaredFunction>) {
        let fn_name: Box<str> = name.into();

        log::info!("adding function to registry {}", fn_name);

        if self.registry.insert(fn_name, decl).is_some() {
            panic!("Function declared twice in same module");
        }
    }
}

#[derive(Debug, Default)]
pub struct MsTraitRegistry {
    pub registry: HashMap<Box<str>, HashMap<MsTypeId, MsFunctionRegistry>>,
}

impl MsTraitRegistry {
    pub fn find_trait_for(
        &self,
        trait_name: &str,
        type_id: MsTypeId,
    ) -> Option<&MsFunctionRegistry> {
        let trait_registry = self.registry.get(trait_name)?;
        let functions = trait_registry.get(&type_id)?;

        Some(functions)
    }

    pub fn add_function(
        &mut self,
        trait_name: &str,
        type_id: MsTypeId,
        func_name: Box<str>,
        func_decl: Rc<MsDeclaredFunction>,
    ) {
        let registry = if let Some(types_to_fns) = self.registry.get_mut(trait_name) {
            types_to_fns
        } else {
            self.registry.insert(trait_name.into(), Default::default());
            self.registry.get_mut(trait_name).unwrap()
        };

        let registry = if let Some(fns) = registry.get_mut(&type_id) {
            fns
        } else {
            registry.insert(type_id, Default::default());
            registry.get_mut(&type_id).unwrap()
        };

        registry.add_function(func_name, func_decl);
    }
}

#[derive(Debug, Clone)]
pub struct MsGenericFunction {
    pub decl: Rc<FunctionDecl>,
    pub generics: Vec<Box<str>>,
}
impl MsGenericFunction {
    pub(crate) fn generate(&self, real_types: Vec<super::modules::MsResolved>) -> FunctionDecl {
        todo!()
    }
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
