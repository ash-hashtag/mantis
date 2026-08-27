use std::borrow::Cow;
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
        MsDeclaredFunction, MsFunctionRegistry, MsFunctionTemplates, MsGenericFunction,
        MsTraitGenericTemplates, MsTraitTemplates,
    },
    traits::MsTraitRegistry,
    types::{
        EnumWithGenerics, MsGenericTemplate, MsGenericTemplateInner, MsType, MsTypeId,
        MsTypeMethodRegistry, MsTypeNameRegistry, MsTypeRegistry, MsTypeTemplates, MsTypeWithId,
        TypeNameWithGenerics,
    },
};

#[derive(Default, Debug)]
pub struct MsModuleRegistry {
    pub registry: HashMap<Cow<'static, str>, MsModule>, // path of module -> Registries
}

#[derive(Default, Debug)]
pub struct MsModule {
    pub fn_registry: MsFunctionRegistry,
    pub fn_templates: MsFunctionTemplates,
    pub trait_registry: MsTraitRegistry,
    pub trait_templates: MsTraitTemplates,
    pub type_registry: MsTypeNameRegistry,
    pub type_templates: MsTypeTemplates,
    pub type_fn_registry: MsTypeMethodRegistry,
    pub trait_generic_templates: MsTraitGenericTemplates,
    pub submodules: HashMap<Cow<'static, str>, MsModule>,
    pub aliased_types: HashMap<TypeNameWithGenerics, MsTypeWithId>,
}

// MsTypeFunctionRegistry was removed and replaced by MsTypeMethodRegistry in types.rs

#[derive(Debug, Clone)]
pub enum MsResolved {
    Function(Rc<MsDeclaredFunction>),
    Type(MsTypeWithId),
    TypeRef(MsTypeWithId, bool),
    Generic(Rc<MsGenericTemplate>),
    EnumUnwrap(MsTypeWithId, Cow<'static, str>), // enum_ty and variant name
    GenericFunctionInstantiation(MsGenericFunction, Vec<MsResolved>),
    GenericFunction(MsGenericFunction),
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
    pub fn find_type(&self, name: &str) -> Option<MsTypeWithId> {
        if let Some(ty) = self.type_registry.get_from_str(name) {
            return Some(ty.clone());
        }
        for sub in self.submodules.values() {
            if let Some(ty) = sub.find_type(name) {
                return Some(ty);
            }
        }
        None
    }

    pub fn find_any_type_fn(&self, fn_name: &str) -> Option<Rc<MsDeclaredFunction>> {
        for reg in self.type_fn_registry.map.values() {
            if let Some(func) = reg.registry.get(fn_name) {
                return Some(func.clone());
            }
        }
        for sub in self.submodules.values() {
            if let Some(func) = sub.find_any_type_fn(fn_name) {
                return Some(func);
            }
        }
        None
    }

    pub fn find_type_fn_by_name(&self, type_name_prefix: &str, fn_name: &str) -> Option<Rc<MsDeclaredFunction>> {
        for (ty_id, reg) in &self.type_fn_registry.map {
            if let Some(ty_name) = self.type_registry.name_of(*ty_id) {
                if ty_name == type_name_prefix || ty_name.starts_with(&format!("{}[", type_name_prefix)) {
                    if let Some(func) = reg.registry.get(fn_name) {
                        return Some(func.clone());
                    }
                }
            }
        }
        for sub in self.submodules.values() {
            if let Some(func) = sub.find_type_fn_by_name(type_name_prefix, fn_name) {
                return Some(func);
            }
        }
        None
    }

    pub fn find_type_fn(&self, ty_id: MsTypeId, fn_name: &str) -> Option<Rc<MsDeclaredFunction>> {
        if let Some(reg) = self.type_fn_registry.map.get(&ty_id) {
            if let Some(func) = reg.registry.get(fn_name) {
                return Some(func.clone());
            }
        }
        for sub in self.submodules.values() {
            if let Some(func) = sub.find_type_fn(ty_id, fn_name) {
                return Some(func);
            }
        }
        None
    }

    pub fn find_fn_template(&self, name: &str) -> Option<MsGenericFunction> {
        if let Some(t) = self.fn_templates.registry.get(name) {
            return Some(t.clone());
        }
        for sub in self.submodules.values() {
            if let Some(t) = sub.find_fn_template(name) {
                return Some(t);
            }
        }
        None
    }

    pub fn find_function(&self, name: &str) -> Option<Rc<MsDeclaredFunction>> {
        if let Some(f) = self.fn_registry.registry.get(name) {
            return Some(f.clone());
        }
        for sub in self.submodules.values() {
            if let Some(f) = sub.find_function(name) {
                return Some(f);
            }
        }
        None
    }

    pub fn find_type_template(&self, name: &str) -> Option<Rc<MsGenericTemplate>> {
        if let Some(t) = self.type_templates.registry.get(name) {
            return Some(t.clone());
        }
        for sub in self.submodules.values() {
            if let Some(t) = sub.find_type_template(name) {
                return Some(t);
            }
        }
        None
    }

    pub fn clear_aliases(&mut self) {
        self.aliased_types.clear();
    }

    pub fn add_alias(&mut self, alias_name: TypeNameWithGenerics, alias_type: MsTypeWithId) {
        if let Some(existing) = self.aliased_types.get(&alias_name) {
            if existing.id == alias_type.id {
                return;
            }
        }
        if let Some(old) = self
            .aliased_types
            .insert(alias_name.clone(), alias_type.clone())
        {
            if old.id != alias_type.id {
                log::warn!(
                    "Overwriting type alias {:?} (Old ID: {:?}, New ID: {:?})",
                    alias_name,
                    old.id,
                    alias_type.id
                );
            }
        }
    }

    pub fn resolve_from_str(&mut self, type_name: &str) -> Option<MsResolved> {
        self.resolve(&Type::Named(mantis_parser::ast::Ident::new(
            type_name,
            mantis_parser::token::Span::new(0, 0),
        )))
    }

    pub fn resolve(&mut self, type_name: &Type) -> Option<MsResolved> {
        if let Some(ty_name) = TypeNameWithGenerics::from_type(type_name) {
            if let Some(resolved) = self.aliased_types.get(&ty_name) {
                return Some(MsResolved::Type(resolved.clone()));
            }
        }

        match type_name {
            Type::Generic(base, generics) => {
                if let Some(base_name) = base.as_name() {
                    if base_name.starts_with('$') {
                        let real_name = match &base_name[1..] {
                            "Args" => "i64",
                            "Any" => "i64",
                            other => "i64",
                        };
                        if let Some(ty) = self.type_registry.get_from_str(real_name) {
                            return Some(MsResolved::Type(ty.clone()));
                        }
                    }
                }
                if let Type::Nested(root, method) = &**base {
                    if let (Some(root_name), Some(method_name)) = (root.as_name(), method.as_name())
                    {
                        let matching_template = self
                            .trait_generic_templates
                            .registry
                            .get(root_name)
                            .and_then(|templates| {
                                templates
                                    .iter()
                                    .find(|t| {
                                        t.decl.name.as_ref().and_then(|n| n.as_name())
                                            == Some(method_name)
                                    })
                                    .cloned()
                            });

                        if let Some(template) = matching_template {
                            let real_types = generics
                                .iter()
                                .map(|x| self.resolve(x))
                                .collect::<Option<Vec<_>>>()?;
                            return Some(MsResolved::GenericFunctionInstantiation(
                                template, real_types,
                            ));
                        }
                    }
                }
                {
                    let generic_key = format!("{:?}", type_name);
                    if let Some(ty) = self.type_registry.get_from_str(&generic_key) {
                        return Some(MsResolved::Type(ty.clone()));
                    }
                    if let Some(func) = self.fn_registry.registry.get(generic_key.as_str()) {
                        return Some(MsResolved::Function(func.clone()));
                    }
                }
                if let Type::Nested(root, child) = &**base {
                    let gen_expr = Type::Generic(root.clone(), generics.clone());
                    if let Some(MsResolved::Type(ty)) = self.resolve(&gen_expr) {
                        if let Some(variant_name) = child.as_name() {
                            return Some(MsResolved::EnumUnwrap(ty, Cow::Owned(variant_name.to_string())));
                        }
                    }
                }
                {
                    let key = type_name.as_name().unwrap_or_default().to_string();

                    let template_opt = self.find_type_template(key.as_str());
                    if let Some(template) = template_opt
                    {
                        log::info!("found template {}, generating struct", key);
                        let mut real_types = HashMap::<Cow<'static, str>, MsTypeWithId>::new();

                        for (generic_name, ty) in template.generics.iter().zip(generics.iter()) {
                            if let Some(real_ty) = self.resolve(ty).and_then(|r| r.ty()) {
                                real_types.insert(generic_name.clone(), real_ty);
                            }
                        }
                        let generated_type = template.generate(&real_types, self);
                        return Some(MsResolved::Type(generated_type));
                    }
                    if let Some(template) = self.fn_templates.registry.get(key.as_str()).cloned() {
                        let real_types = generics
                            .iter()
                            .map(|x| self.resolve(x))
                            .collect::<Option<Vec<_>>>()?;

                        return Some(MsResolved::GenericFunctionInstantiation(
                            template, real_types,
                        ));
                    }
                }

                if let Type::Nested(root, child) = type_name {
                    let root_name = root.as_name().unwrap_or_default();
                    let method_name = child.as_name().unwrap_or_default();
                    let template_opt = self
                        .trait_generic_templates
                        .registry
                        .get(root_name)
                        .and_then(|templates| {
                            templates.iter().find(|t| {
                                t.decl.name.as_ref().and_then(|n| n.as_name()) == Some(method_name)
                            }).cloned()
                        });
                    if let Some(template) = template_opt {
                        let real_types = generics
                            .iter()
                            .map(|x| self.resolve(x))
                            .collect::<Option<Vec<_>>>()?;
                        return Some(MsResolved::GenericFunctionInstantiation(
                            template,
                            real_types,
                        ));
                    }
                }
                return None;
            }
            Type::Named(ident) => {
                let key = ident.name.as_str();
                if let Some(ty) = self.find_type(key) {
                    return Some(MsResolved::Type(ty));
                }
                if key.starts_with('$') {
                    let real_name = match &key[1..] {
                        "I64" => "i64",
                        "I32" => "i32",
                        "I16" => "i16",
                        "I8" => "i8",
                        "U64" => "u64",
                        "U32" => "u32",
                        "U16" => "u16",
                        "U8" => "u8",
                        "F64" => "f64",
                        "F32" => "f32",
                        "Bool" => "bool",
                        "Char" => "char",
                        "Str" => "StrSlice",
                        "Any" => "i64",
                        "Args" => "i64",
                        other => other,
                    };
                    if let Some(ty) = self.type_registry.get_from_str(real_name) {
                        return Some(MsResolved::Type(ty.clone()));
                    }
                }
                if let Some(func) = self.fn_registry.registry.get(key) {
                    return Some(MsResolved::Function(func.clone()));
                }
                if let Some(func) = self
                    .fn_registry
                    .registry
                    .iter()
                    .find(|(k, _)| k.ends_with(&format!(".{}", key)))
                    .map(|(_, v)| v)
                {
                    return Some(MsResolved::Function(func.clone()));
                }
                if let Some(template) = self.fn_templates.registry.get(key) {
                    return Some(MsResolved::GenericFunction(template.clone()));
                }
                for sub in self.submodules.values_mut() {
                    if let Some(res) = sub.resolve(type_name) {
                        return Some(res);
                    }
                }
                return None;
            }
            Type::Nested(root, child) => {
                let ty_opt = self.resolve(root).and_then(|r| r.ty()).or_else(|| {
                    let key = root.as_name()?;
                    let template = self.find_type_template(key)?;
                    let mut real_types = HashMap::new();
                    for gen_name in &template.generics {
                        if let Some(dummy_ty) = self.type_registry.get_from_str("i64") {
                            real_types.insert(gen_name.clone(), dummy_ty);
                        }
                    }
                    Some(template.generate(&real_types, self))
                });

                if let Some(ty) = ty_opt {
                    match &ty.ty {
                        MsType::Enum(_) => {
                            if let Some(variant_name) = child.as_name() {
                                return Some(MsResolved::EnumUnwrap(ty.clone(), Cow::Owned(variant_name.to_string())));
                            }
                        }
                        _ => {}
                    }
                    if let Some(child_name) = child.as_name() {
                        if let Some(func) = self.find_type_fn(ty.id, child_name) {
                            return Some(MsResolved::Function(func));
                        }
                        if let Some(root_name) = root.as_name() {
                            if let Some(func) = self.find_type_fn_by_name(root_name, child_name) {
                                return Some(MsResolved::Function(func));
                            }
                        }
                        if let Some(func) = self.find_any_type_fn(child_name) {
                            return Some(MsResolved::Function(func));
                        }
                        if let Some(func) = self.find_function(child_name) {
                            return Some(MsResolved::Function(func));
                        }
                        if let Some(template) = self.find_fn_template(child_name) {
                            return Some(MsResolved::GenericFunction(template));
                        }
                    }
                }

                fn flatten_type_path(t: &Type) -> String {
                    match t {
                        Type::Named(id) => id.name.to_string(),
                        Type::Nested(r, c) => format!("{}.{}", flatten_type_path(r), flatten_type_path(c)),
                        Type::Generic(base, _) => flatten_type_path(base),
                        _ => String::new(),
                    }
                }
                let full_fn_name = format!("{}.{}", flatten_type_path(root), flatten_type_path(child));
                if let Some(template) = self.find_fn_template(full_fn_name.as_str()) {
                    return Some(MsResolved::GenericFunction(template));
                }
                if let Some(func) = self.find_function(full_fn_name.as_str()) {
                    return Some(MsResolved::Function(func));
                }

                let parts: Vec<&str> = full_fn_name.split('.').collect();
                if parts.len() > 1 {
                    if let Some(sub) = self.submodules.get_mut(parts[0]) {
                        let remaining = parts[1..].join(".");
                        let rem_child = mantis_parser::ast::Ident::new(
                            &remaining,
                            mantis_parser::token::Span::new(0, 0),
                        );
                        if let Some(res) = sub.resolve(&Type::Named(rem_child)) {
                            return Some(res);
                        }
                    }
                }

                if let Some(child_name) = child.as_name() {
                    if let Some(func) = self.find_function(child_name) {
                        return Some(MsResolved::Function(func));
                    }
                    if let Some(template) = self.find_fn_template(child_name) {
                        return Some(MsResolved::GenericFunction(template));
                    }
                }

                let key = root.as_name().unwrap_or_default().to_string();

                if let Some(template) = self.type_templates.registry.get(key.as_str()).cloned() {
                    if let crate::registries::types::MsGenericTemplateInner::Enum(ref enum_gen) =
                        template.inner_type
                    {
                        let variant_name = child.as_name()?;
                        if enum_gen.map.contains_key(variant_name) {
                            let mut real_types = HashMap::new();
                            for gen_name in &template.generics {
                                let gen_key = TypeNameWithGenerics::new(Cow::Owned(gen_name.to_string()), vec![]);
                                if let Some(aliased) = self.aliased_types.get(&gen_key) {
                                    real_types.insert(Cow::Owned(gen_name.to_string()), aliased.clone());
                                } else if let Some(dummy_ty) =
                                    self.type_registry.get_from_str("i64")
                                {
                                    real_types.insert(Cow::Owned(gen_name.to_string()), dummy_ty);
                                }
                            }
                            let enum_ty = template.generate(&real_types, self);
                            return Some(MsResolved::EnumUnwrap(enum_ty, Cow::Owned(variant_name.to_string())));
                        }
                    }
                }

                if let Some(module) = self.submodules.get_mut(key.as_str()) {
                    return module.resolve(child);
                }
                None
            }

            Type::Ref(ty, is_mutable) => {
                let inner = self.resolve(ty)?.ty().unwrap();
                let deterministic_name = format!(
                    "ref_{}{}",
                    if *is_mutable { "mut_" } else { "" },
                    inner.id.0
                );
                let ty_val = MsType::Ref(Box::new(inner.ty), *is_mutable);
                let id = self
                    .type_registry
                    .add_type(deterministic_name, ty_val.clone());
                let ref_ty = MsTypeWithId { id, ty: ty_val };
                return Some(MsResolved::TypeRef(ref_ty, *is_mutable));
            }
            Type::Function(params, ret) => {
                let mut param_ids = Vec::new();
                for p in params {
                    param_ids.push(self.resolve(p)?.ty()?.id);
                }
                let ret_id = self.resolve(ret)?.ty()?.id;

                // Create a dummy function signature for the type
                let mut arguments = LinearMap::new();
                for (i, id) in param_ids.into_iter().enumerate() {
                    arguments.insert(format!("p{}", i).into(), id);
                }

                let signature = MsDeclaredFunction {
                    arguments,
                    rets: Some(ret_id),
                    fn_type: crate::registries::functions::FunctionType::Public,
                    func_id: cranelift_module::FuncId::from_u32(0), // Placeholder
                };

                let id = self
                    .type_registry
                    .get_or_add_type(MsType::Function(Rc::new(signature)));
                let ty = self.type_registry.get_from_type_id(id).unwrap();
                return Some(MsResolved::Type(MsTypeWithId { id, ty }));
            }

            Type::Unknown => return None,
            _ => unreachable!("unhandled {:?}", type_name),
        }
    }

    pub fn resolve_with_generics(
        &mut self,
        type_name: &Type,
        root_generics: &[Cow<'static, str>],
    ) -> MsGenericTemplate {
        match type_name {
            Type::Generic(_, _) | Type::Named(_) => {
                let name = type_name.as_name().unwrap_or("alias");
                let template = MsGenericTemplate {
                    name: Cow::Owned(name.to_string()),
                    generics: root_generics.to_vec(),
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
                return module.resolve_with_generics(child, root_generics);
            }
            _ => unreachable!("unhandled {:?}", type_name),
        }
    }
}

pub enum ModuleEntry {
    Module(String),
    Dir(PathBuf),
}

pub fn resolve_module_by_path(
    include_dirs: &[String],
    path: &[mantis_parser::ast::Ident],
) -> Option<ModuleEntry> {
    if path.is_empty() {
        return None;
    }
    let subpath = path
        .iter()
        .map(|i| i.name.as_str())
        .collect::<Vec<_>>()
        .join("/");
    for dir_path in include_dirs {
        let base = std::path::Path::new(dir_path).join(&subpath);
        let file_cand = base.with_extension("ms");
        if file_cand.is_file() {
            if let Ok(content) = std::fs::read_to_string(&file_cand) {
                return Some(ModuleEntry::Module(content));
            }
        }
        if path.len() > 1 {
            let pkg = path[0].name.as_str();
            let rest = path[1..].iter().map(|i| i.name.as_str()).collect::<Vec<_>>().join("/");
            let src_cand = std::path::Path::new(dir_path).join(pkg).join("src").join(&rest).with_extension("ms");
            if src_cand.is_file() {
                if let Ok(content) = std::fs::read_to_string(&src_cand) {
                    return Some(ModuleEntry::Module(content));
                }
            }
        }
        if base.is_dir() {
            for sub in &["mod.ms", "lib.ms", "main.ms", "src/lib.ms", "src/main.ms", "src/mod.ms"] {
                let p = base.join(sub);
                if p.is_file() {
                    if let Ok(content) = std::fs::read_to_string(&p) {
                        return Some(ModuleEntry::Module(content));
                    }
                }
            }
            return Some(ModuleEntry::Dir(base));
        }
    }
    if let Some(first) = path.first() {
        return resolve_module_by_word(include_dirs, &first.name);
    }
    None
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
