use std::collections::HashMap;
use std::rc::Rc;
use super::types::{MsTypeWithId, MsTypeId};
use super::functions::{MsDeclaredFunction, MsFunctionRegistry};

#[derive(Clone, Debug)]
pub struct MsTraitMethod {
    pub name: Box<str>,
    pub args: Vec<MsTypeWithId>,
    pub ret: Option<MsTypeWithId>,
}

#[derive(Clone, Debug)]
pub struct MsTrait {
    pub name: Box<str>,
    pub methods: HashMap<Box<str>, MsTraitMethod>,
}

#[derive(Debug, Default)]
pub struct MsTraitRegistry {
    pub traits: HashMap<Box<str>, MsTrait>,
    // trait_name -> type_id -> implementation
    pub registry: HashMap<Box<str>, HashMap<MsTypeId, MsFunctionRegistry>>,
}

impl MsTraitRegistry {
    pub fn add_trait(&mut self, name: Box<str>, ms_trait: MsTrait) {
        self.traits.insert(name, ms_trait);
    }

    pub fn get_trait(&self, name: &str) -> Option<&MsTrait> {
        self.traits.get(name)
    }

    pub fn add_implementation(
        &mut self,
        type_id: MsTypeId,
        trait_name: Box<str>,
        methods: MsFunctionRegistry,
    ) {
        let trait_impls = self.registry.entry(trait_name).or_default();
        trait_impls.insert(type_id, methods);
    }

    pub fn get_implementation(
        &self,
        type_id: MsTypeId,
        trait_name: &str,
    ) -> Option<&MsFunctionRegistry> {
        self.registry.get(trait_name)?.get(&type_id)
    }

    pub fn find_trait_for(
        &self,
        trait_name: &str,
        type_id: MsTypeId,
    ) -> Option<&MsFunctionRegistry> {
        self.get_implementation(type_id, trait_name)
    }

    pub fn find_method_implementation(
        &self,
        type_id: MsTypeId,
        trait_name: &str,
        method_name: &str,
    ) -> Option<Rc<MsDeclaredFunction>> {
        let trait_impl = self.get_implementation(type_id, trait_name)?;
        trait_impl.registry.get(method_name).cloned()
    }

    pub fn add_function(
        &mut self,
        trait_name: &str,
        type_id: MsTypeId,
        func_name: Box<str>,
        func_decl: Rc<MsDeclaredFunction>,
    ) {
        self.registry
            .entry(trait_name.into())
            .or_default()
            .entry(type_id)
            .or_default()
            .add_function(func_name, func_decl);
    }
}
