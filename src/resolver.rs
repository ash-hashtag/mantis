use std::{cell::RefCell, collections::HashMap};

use crate::{registries::variable::MsVar, scope::MsScopes};

pub struct Resolver {
    file_path: String,
    scopes: MsScopes,
    imports: HashMap<String, RefCell<Resolver>>, // alias -> file content resolver
}

pub enum Resolved {
    Variable(MsVar),
    Resolver(RefCell<Resolver>),
    None,
}

impl Resolver {
    pub fn resolve(&mut self, path: &[&str]) -> Resolved {
        if path.len() == 0 {
            return Resolved::None;
        }

        if let Some(variable) = self.scopes.get_variable(path[0]) {
            todo!("find its methods/fields");
        }

        if let Some(resoler) = self.imports.get_mut(path[0]) {
            return resoler.get_mut().resolve(&path[1..]);
        }

        Resolved::None
    }
}
