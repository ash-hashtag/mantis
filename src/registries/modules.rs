use std::{
    collections::HashMap,
    fs::FileType,
    path::{Path, PathBuf},
    rc::Rc,
};

use linear_map::LinearMap;
use mantis_parser::ast::TypeExpr as Type;

use crate::{
    backend::compile_function::random_string,
    native::instructions::Either,
    registries::{structs::MsStructType, types::StructWithGenerics},
};

use super::{
    functions::{
        MsDeclaredFunction, MsFunctionRegistry, MsFunctionTemplates, MsTraitGenericTemplates,
        MsTraitRegistry, MsTraitTemplates,
    },
    types::{
        EnumWithGenerics, MsGenericTemplate, MsGenericTemplateInner, MsType, MsTypeId,
        MsTypeNameRegistry, MsTypeRegistry, MsTypeTemplates, MsTypeWithId, TypeNameWithGenerics,
    },
};

#[derive(Default, Debug)]
pub struct MsModuleRegistry {
    pub registry: HashMap<Box<str>, MsModule>, // path of module -> Registries
}

#[derive(Default, Debug)]
pub struct MsModule {
    pub fn_registry: MsFunctionRegistry,
    pub fn_templates: MsFunctionTemplates,
    pub trait_registry: MsTraitRegistry,
    pub trait_templates: MsTraitTemplates,
    pub type_registry: MsTypeNameRegistry,
    pub type_templates: MsTypeTemplates,
    pub type_fn_registry: MsTypeFunctionRegistry,
    pub trait_generic_templates: MsTraitGenericTemplates,
    pub submodules: HashMap<Box<str>, MsModule>,
    pub aliased_types: HashMap<TypeNameWithGenerics, MsTypeWithId>,
}

#[derive(Debug, Default)]
pub struct MsTypeFunctionRegistry {
    pub map: HashMap<MsTypeId, MsFunctionRegistry>,
}

impl MsTypeFunctionRegistry {
    pub fn add_function(
        &mut self,
        ty: MsTypeId,
        fn_name: impl Into<Box<str>>,
        func: Rc<MsDeclaredFunction>,
    ) {
        let ty = if let Some(ty) = self.map.get_mut(&ty) {
            ty
        } else {
            self.map.insert(ty.clone(), Default::default());
            self.map.get_mut(&ty).unwrap()
        };
        ty.add_function(fn_name, func);
    }
}

#[derive(Debug, Clone)]
pub enum MsResolved {
    Function(Rc<MsDeclaredFunction>),
    Type(MsTypeWithId),
    TypeRef(MsTypeWithId, bool),
    Generic(Rc<MsGenericTemplate>),
    EnumUnwrap(MsTypeWithId, Box<str>), // enum_ty and variant name
}

impl MsResolved {
    pub fn ty(&self) -> Option<MsTypeWithId> {
        match self {
            MsResolved::Type(ms_type) => Some(ms_type.clone()),
            MsResolved::TypeRef(ms_type, _) => Some(ms_type.clone()),
            _ => None,
        }
    }

    pub fn is_reference(&self) -> bool {
        matches!(self, MsResolved::TypeRef(_, __))
    }
}

impl MsModule {
    pub fn clear_aliases(&mut self) {
        self.aliased_types.clear();
    }

    pub fn add_alias(&mut self, alias_name: TypeNameWithGenerics, alias_type: MsTypeWithId) {
        assert!(self.aliased_types.insert(alias_name, alias_type).is_none())
    }

    pub fn resolve_from_str(&mut self, type_name: &str) -> Option<MsResolved> {
        self.resolve(&Type::Named(mantis_parser::ast::Ident::new(type_name, mantis_parser::token::Span::new(0, 0))))
    }

    pub fn resolve(&mut self, type_name: &Type) -> Option<MsResolved> {
        if let Some(ty_name) = TypeNameWithGenerics::from_type(type_name) {
            if let Some(resolved) = self.aliased_types.get(&ty_name) {
                return Some(MsResolved::Type(resolved.clone()));
            }
        }

        match type_name {
            Type::Generic(base, generics) => {
                {
                    let generic_key = format!("{:?}", type_name);
                    if let Some(ty) = self.type_registry.get_from_str(&generic_key) {
                        return Some(MsResolved::Type(ty.clone()));
                    }
                    if let Some(func) = self.fn_registry.registry.get(generic_key.as_str()) {
                        return Some(MsResolved::Function(func.clone()));
                    }
                }
                {
                    let key = base.as_name().unwrap_or_default().to_string();
                    if let Some(template) = self.type_templates.registry.get(key.as_str()).cloned()
                    {
                        log::info!("found template {}, generating struct", key);
                        let mut real_types = HashMap::<Box<str>, MsTypeWithId>::new();

                        for (generic_name, ty) in template.generics.iter().zip(generics.iter()) {
                            if let Some(MsResolved::Type(real_ty)) = self.resolve(ty) {
                                real_types.insert(generic_name.as_str().into(), real_ty);
                            }
                        }
                        dbg!(&real_types);
                        let generated_type = template.generate(&real_types, self);
                        return Some(MsResolved::Type(generated_type));
                    }
                    if let Some(template) = self.fn_templates.registry.get(key.as_str()).cloned() {
                        let mut real_types = generics
                            .iter()
                            .map(|x| self.resolve(x))
                            .collect::<Option<Vec<_>>>()?;

                        let generated_func = template.generate(real_types);
                        todo!("compile the generated func")
                    }
                }
                return None;
            }
            Type::Named(ident) => {
                let key = ident.name.as_str();
                if let Some(ty) = self.type_registry.get_from_str(key) {
                    return Some(MsResolved::Type(ty.clone()));
                }
                if let Some(func) = self.fn_registry.registry.get(key) {
                    return Some(MsResolved::Function(func.clone()));
                }
                return None;
            }
            Type::Nested(root, child) => {
                if let Some(MsResolved::Type(ty)) = self.resolve(root) {
                    match &ty.ty {
                        MsType::Enum(enum_ty) => {
                            let variant_name = child.as_name()?;
                            return Some(MsResolved::EnumUnwrap(ty, variant_name.into()));
                        }
                        _ => {}
                    }
                    let func = self
                        .type_fn_registry
                        .map
                        .get(&ty.id)?
                        .registry
                        .get(child.as_name()?)?;
                    return Some(MsResolved::Function(func.clone()));
                }

                let key = root.as_name().unwrap_or_default().to_string();

                let module = self.submodules.get_mut(key.as_str())?;
                return module.resolve(child);
            }

            Type::Ref(ty, is_mutable) => {
                let ty = self.resolve(ty)?.ty().unwrap();
                let ty = MsResolved::TypeRef(ty, *is_mutable);
                return Some(ty);
            }

            _ => unreachable!("unhandled {:?}", type_name),
        };
    }

    pub fn resolve_with_generics(
        &mut self,
        type_name: &Type,
        root_generices: &[String],
    ) -> MsGenericTemplate {
        match type_name {
            Type::Generic(_, _) | Type::Named(_) => {
                let template = MsGenericTemplate {
                    generics: root_generices.to_vec(),
                    inner_type: MsGenericTemplateInner::Type(
                        TypeNameWithGenerics::from_type(type_name).unwrap(),
                    ),
                };

                return template;
            }
            Type::Nested(root, child) => {
                let key = root.as_name().unwrap_or_default().to_string();
                let module = self
                    .submodules
                    .get_mut(key.as_str())
                    .expect("can't find module");
                return module.resolve_with_generics(child, root_generices);
            }
            _ => unreachable!("unhandled {:?}", type_name),
        };
    }
}

pub enum ModuleEntry {
    Module(String),
    Dir(PathBuf),
}

pub fn resolve_module_by_word(include_dirs: &[String], module_name: &str) -> Option<ModuleEntry> {
    for dir_path in include_dirs {
        let dir = match std::fs::read_dir(dir_path) {
            Ok(d) => d,
            Err(err) => {
                log::warn!("unable to read dir {}, error: {:?}", dir_path, err);
                continue;
            }
        };
        for entity in dir {
            let entry = match entity {
                Ok(d) => d,
                Err(err) => {
                    log::warn!("unable to read dir {}, error: {:?}", dir_path, err);
                    continue;
                }
            };
            let entry_name = entry.file_name();
            let file_name = entry_name.to_str().expect("invalid os string");
            if file_name == module_name && entry.file_type().unwrap().is_dir() {
                return Some(ModuleEntry::Dir(entry.path()));
            } else if file_name.len() == module_name.len() + 3
                && file_name.starts_with(module_name)
                && file_name.ends_with(".ms")
            {
                let content = std::fs::read_to_string(entry.path())
                    .expect(&format!("Failed to read {:?}", entry.path()));
                return Some(ModuleEntry::Module(content));
            }
        }
    }
    return None;
}
