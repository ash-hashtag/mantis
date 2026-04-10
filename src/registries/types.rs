use std::{
    collections::{BTreeMap, HashMap},
    fmt::Display,
    hash::Hash,
    rc::Rc,
};

use codegen::ir::{condcodes, Inst};
use cranelift::prelude::*;
use linear_map::LinearMap;
use mantis_parser::ast::{self, BinOp as BinaryOperation, FnDecl as FunctionDecl, TypeExpr};

use crate::{backend::compile_function::random_string, native::instructions::Either};

use super::{
    functions::{MsDeclaredFunction, MsFunctionRegistry},
    modules::MsModule,
    structs::{MsEnumType, MsStructType},
    MsRegistry, MsRegistryExt,
};

#[derive(Clone, Debug, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MsNativeType {
    Char,
    Bool,
    Void,
    Array,
    Function,
    I64,
    I32,
    F32,
    F64,
    U32,
    U64,
    I8,
    U8,
    I16,
    U16,
}

impl MsNativeType {
    pub fn size(&self) -> usize {
        match self {
            Self::Bool | Self::I8 | Self::U8 => 1,
            Self::I16 | Self::U16 => 2,
            Self::Char | Self::I32 | Self::U32 | Self::F32 => 4,
            Self::I64 | Self::U64 | Self::F64 => 8,
            _ => {
                panic!("unhandled type {:?}", self);
                return 0;
            }
        }
    }

    pub fn align(&self) -> usize {
        self.size()
    }

    pub fn from_str(s: &str) -> Option<Self> {
        Some(match s {
            "i8" => Self::I8,
            "i16" => Self::I16,
            "i32" => Self::I32,
            "i64" => Self::I64,
            "u8" => Self::U8,
            "u16" => Self::U16,
            "u32" => Self::U32,
            "u64" => Self::U64,
            "f32" => Self::F32,
            "f64" => Self::F64,
            "array" => Self::Array,
            "bool" => Self::Bool,
            "char" => Self::Char,
            _ => {
                return None;
            }
        })
    }

    pub fn to_abi_param(&self) -> Option<AbiParam> {
        Some(match self {
            MsNativeType::Void => return None,
            MsNativeType::Bool => AbiParam::new(types::I8),
            MsNativeType::F32 => AbiParam::new(types::F32),
            MsNativeType::F64 => AbiParam::new(types::F64),
            MsNativeType::I8 => AbiParam::new(types::I8).sext(),
            MsNativeType::I16 => AbiParam::new(types::I16).sext(),
            MsNativeType::I32 => AbiParam::new(types::I32).sext(),
            MsNativeType::I64 => AbiParam::new(types::I64).sext(),
            MsNativeType::U8 => AbiParam::new(types::I8).uext(),
            MsNativeType::U16 => AbiParam::new(types::I16).uext(),
            MsNativeType::U32 => AbiParam::new(types::I32).uext(),
            MsNativeType::U64 => AbiParam::new(types::I64).uext(),
            _ => todo!(),
        })
    }
    pub fn to_cl_type(&self) -> Option<Type> {
        Some(match self {
            MsNativeType::Void => return None,
            MsNativeType::Bool => types::I8,
            MsNativeType::Char => types::I32,
            MsNativeType::F32 => types::F32,
            MsNativeType::F64 => types::F64,
            MsNativeType::I8 => types::I8,
            MsNativeType::I16 => types::I16,
            MsNativeType::I32 => types::I32,
            MsNativeType::I64 => types::I64,
            MsNativeType::U8 => types::I8,
            MsNativeType::U16 => types::I16,
            MsNativeType::U32 => types::I32,
            MsNativeType::U64 => types::I64,
            _ => todo!(),
        })
    }

    pub(crate) fn add(&self, lhs: Value, rhs: Value, fbx: &mut FunctionBuilder<'_>) -> Value {
        if self.is_int() {
            fbx.ins().iadd(lhs, rhs)
        } else if self.is_float() {
            fbx.ins().fadd(lhs, rhs)
        } else {
            unreachable!()
        }
    }
    pub(crate) fn sub(&self, lhs: Value, rhs: Value, fbx: &mut FunctionBuilder<'_>) -> Value {
        if self.is_int() {
            fbx.ins().isub(lhs, rhs)
        } else if self.is_float() {
            fbx.ins().fsub(lhs, rhs)
        } else {
            unreachable!()
        }
    }
    pub(crate) fn mult(&self, lhs: Value, rhs: Value, fbx: &mut FunctionBuilder<'_>) -> Value {
        if self.is_int() {
            fbx.ins().imul(lhs, rhs)
        } else if self.is_float() {
            fbx.ins().fmul(lhs, rhs)
        } else {
            unreachable!()
        }
    }

    pub(crate) fn div(&self, lhs: Value, rhs: Value, fbx: &mut FunctionBuilder<'_>) -> Value {
        if self.is_int() {
            if self.is_uint() {
                fbx.ins().udiv(lhs, rhs)
            } else {
                fbx.ins().sdiv(lhs, rhs)
            }
        } else if self.is_float() {
            fbx.ins().fdiv(lhs, rhs)
        } else {
            unreachable!()
        }
    }

    pub(crate) fn compare(
        &self,
        op: BinaryOperation,
        lhs: Value,
        rhs: Value,
        fbx: &mut FunctionBuilder<'_>,
    ) -> Value {
        if self.is_int() {
            fbx.ins().icmp(
                binary_cmp_op_to_condcode_intcc(op, !self.is_uint()),
                lhs,
                rhs,
            )
        } else if self.is_float() {
            fbx.ins().fcmp(binary_cmp_op_to_condcode_fcc(op), lhs, rhs)
        } else {
            unreachable!()
        }
    }

    pub fn is_int(&self) -> bool {
        match self {
            MsNativeType::I8
            | MsNativeType::U8
            | MsNativeType::I16
            | MsNativeType::U16
            | MsNativeType::U32
            | MsNativeType::U64
            | MsNativeType::I64
            | MsNativeType::I32
            | MsNativeType::Char => true,
            _ => false,
        }
    }

    pub fn is_uint(&self) -> bool {
        match self {
            MsNativeType::U32 | MsNativeType::U8 | MsNativeType::U16 | MsNativeType::U64 => true,
            _ => false,
        }
    }

    pub fn is_sint(&self) -> bool {
        match self {
            MsNativeType::I32
            | MsNativeType::I8
            | MsNativeType::I16
            | MsNativeType::I64
            | MsNativeType::Char => true,
            _ => false,
        }
    }

    pub fn is_float(&self) -> bool {
        match self {
            MsNativeType::F32 | MsNativeType::F64 => true,
            _ => false,
        }
    }

    pub fn cast_to(&self, lhs: Value, r: &MsType, fbx: &mut FunctionBuilder) -> Value {
        let MsType::Native(rnty) = r else {
            panic!("Non native casting not supported yet");
        };
        let diff = r.size() as isize - self.size() as isize;
        let cl_type = rnty.to_cl_type().unwrap();
        if self.is_int() {
            if rnty.is_int() {
                if diff < 0 {
                    return fbx.ins().ireduce(cl_type, lhs);
                } else {
                    if diff == 0 {
                        return lhs;
                    }
                    if rnty.is_uint() {
                        return fbx.ins().uextend(cl_type, lhs);
                    } else {
                        return fbx.ins().sextend(cl_type, lhs);
                    }
                }
            } else if rnty.is_float() {
                if self.is_uint() {
                    return fbx.ins().fcvt_from_uint(cl_type, lhs);
                } else {
                    return fbx.ins().fcvt_from_sint(cl_type, lhs);
                }
            }
        }
        if self.is_float() {
            if rnty.is_uint() {
                return fbx.ins().fcvt_to_uint(cl_type, lhs);
            } else if rnty.is_sint() {
                return fbx.ins().fcvt_to_sint(cl_type, lhs);
            } else if rnty.is_float() {
                if diff > 0 {
                    return fbx.ins().fpromote(cl_type, lhs);
                } else if diff < 0 {
                    return fbx.ins().fdemote(cl_type, lhs);
                }
            }
        }

        lhs
    }

    pub fn to_string(&self) -> &'static str {
        match self {
            MsNativeType::Bool => "bool",
            MsNativeType::Void => "void",
            MsNativeType::Array => "array",
            MsNativeType::Function => "function",
            MsNativeType::I64 => "i64",
            MsNativeType::I32 => "i32",
            MsNativeType::F32 => "f32",
            MsNativeType::F64 => "f64",
            MsNativeType::U32 => "u32",
            MsNativeType::U64 => "u64",
            MsNativeType::I8 => "I8",
            MsNativeType::U8 => "U8",
            MsNativeType::I16 => "I16",
            MsNativeType::U16 => "U16",
            MsNativeType::Char => "char",
        }
    }
}

fn binary_cmp_op_to_condcode_fcc(op: BinaryOperation) -> condcodes::FloatCC {
    use condcodes::FloatCC;
    match op {
        BinaryOperation::Gt => FloatCC::GreaterThan,
        BinaryOperation::GtEq => FloatCC::GreaterThanOrEqual,
        BinaryOperation::Eq => FloatCC::Equal,
        BinaryOperation::NotEq => FloatCC::NotEqual,
        BinaryOperation::Lt => FloatCC::LessThan,
        BinaryOperation::LtEq => FloatCC::GreaterThanOrEqual,
        _ => unreachable!(),
    }
}

pub fn binary_cmp_op_to_condcode_intcc(op: BinaryOperation, signed: bool) -> condcodes::IntCC {
    use condcodes::IntCC;
    if signed {
        match op {
            BinaryOperation::Gt => IntCC::SignedGreaterThan,
            BinaryOperation::GtEq => IntCC::SignedGreaterThanOrEqual,
            BinaryOperation::Eq => IntCC::Equal,
            BinaryOperation::NotEq => IntCC::NotEqual,
            BinaryOperation::Lt => IntCC::SignedLessThan,
            BinaryOperation::LtEq => IntCC::SignedGreaterThanOrEqual,
            _ => unreachable!(),
        }
    } else {
        match op {
            BinaryOperation::Gt => IntCC::UnsignedGreaterThan,
            BinaryOperation::GtEq => IntCC::UnsignedGreaterThanOrEqual,
            BinaryOperation::Eq => IntCC::Equal,
            BinaryOperation::NotEq => IntCC::NotEqual,
            BinaryOperation::Lt => IntCC::UnsignedLessThan,
            BinaryOperation::LtEq => IntCC::UnsignedGreaterThanOrEqual,
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TypeNameWithGenerics {
    pub name: Box<str>,
    pub generics: Vec<TypeNameWithGenerics>,
}

impl TypeNameWithGenerics {
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        let mut name = String::new();
        let mut generics = Vec::new();
        for (i, c) in s.chars().enumerate() {
            if c == '[' {
                let end_of_generic_args = s[i..].find(']')?;
                generics = s[i..end_of_generic_args]
                    .split(',')
                    .map(|x| Self::parse(x))
                    .collect::<Option<Vec<Self>>>()?;

                break;
            } else {
                name.push(c);
            }
        }

        Some(Self {
            name: name.into(),
            generics,
        })
    }

    pub fn new(name: Box<str>, inner_types: Vec<TypeNameWithGenerics>) -> Self {
        Self {
            name,
            generics: inner_types,
        }
    }

    pub fn generate(
        &self,
        real_types: &HashMap<Box<str>, MsTypeWithId>,
        ms_module: &mut MsModule,
    ) -> MsTypeWithId {
        if let Some(ty) = real_types.get(&self.name) {
            assert!(self.generics.is_empty());
            return ty.clone();
        }

        if let Some(ty) = ms_module.type_registry.get_from_str(&self.name) {
            assert!(self.generics.is_empty());
            return ty.clone();
        }

        let template = ms_module
            .type_templates
            .registry
            .get(&self.name)
            .expect(&format!("undeclared template {}", self.name))
            .clone();
        return template.generate(real_types, ms_module);
    }

    pub fn from_type(ty: &TypeExpr) -> Option<Self> {
        let ty = match ty {
            TypeExpr::Generic(base, generics) => Self {
                name: Box::<str>::from(base.as_name().unwrap_or("")),
                generics: generics
                    .iter()
                    .map(Self::from_type)
                    .collect::<Option<Vec<_>>>()?,
            },
            TypeExpr::Named(ident) => Self {
                name: ident.name.clone().into_boxed_str(),
                generics: vec![],
            },
            _ => {
                return None;
            }
        };

        Some(ty)
    }
}

#[derive(Clone, Debug, Default)]
pub struct EnumWithGenerics {
    pub map: LinearMap<Box<str>, Option<TypeNameWithGenerics>>,
}

impl EnumWithGenerics {
    fn generate(
        &self,
        real_types: &HashMap<Box<str>, MsTypeWithId>,
        ms_module: &mut MsModule,
    ) -> MsTypeWithId {
        let mut enum_ty = MsEnumType::default();
        for (field_name, field_ty) in &self.map {
            if let Some(field_ty) = field_ty {
                let ty = field_ty.generate(&real_types, ms_module);
                enum_ty.add_variant(field_name.clone(), Some(ty));
            } else {
                enum_ty.add_variant(field_name.clone(), None);
            }
        }

        let ty = MsType::Enum(Rc::new(enum_ty));

        let ty_name = random_string(20);
        let id = ms_module.type_registry.add_type(ty_name, ty.clone());

        return MsTypeWithId { ty, id };
    }
}

#[derive(Clone, Debug, Default)]
pub struct StructWithGenerics {
    pub map: LinearMap<Box<str>, TypeNameWithGenerics>,
}
impl StructWithGenerics {
    fn generate(
        &self,
        real_types: &HashMap<Box<str>, MsTypeWithId>,
        ms_module: &mut MsModule,
    ) -> MsTypeWithId {
        let mut struct_ty = MsStructType::default();

        for (field_name, field_ty) in &self.map {
            let ty = field_ty.generate(&real_types, ms_module);
            struct_ty.add_field(field_name.clone(), ty);
        }

        let ty = MsType::Struct(Rc::new(struct_ty));

        let ty_name = random_string(20);
        let id = ms_module.type_registry.add_type(ty_name, ty.clone());

        return MsTypeWithId { ty, id };
    }
}

#[derive(Clone, Debug)]
pub enum MsGenericTemplateInner {
    Type(TypeNameWithGenerics),
    Struct(StructWithGenerics),
    Enum(EnumWithGenerics),
    Function(FunctionWithGenerics),
}

#[derive(Clone, Debug)]
pub struct FunctionWithGenerics {
    fn_decl: Rc<FunctionDecl>,
}
impl FunctionWithGenerics {
    fn generte(
        &self,
        real_types: &&HashMap<Box<str>, MsTypeWithId>,
        ms_module: &mut MsModule,
    ) -> MsTypeWithId {
        todo!()
    }
}

#[derive(Clone, Debug)]
pub struct MsGenericTemplate {
    pub generics: Vec<String>,
    pub inner_type: MsGenericTemplateInner,
}

impl MsGenericTemplate {
    pub fn add_generic(&mut self, s: impl Into<String>) {
        let s: String = s.into();
        assert!(!self.generics.contains(&s));
        self.generics.push(s);
    }

    pub fn generate(
        &self,
        real_types: &HashMap<Box<str>, MsTypeWithId>,
        ms_module: &mut MsModule,
    ) -> MsTypeWithId {
        match &self.inner_type {
            MsGenericTemplateInner::Type(ty) => ty.generate(&real_types, ms_module),
            MsGenericTemplateInner::Struct(ty) => ty.generate(&real_types, ms_module),
            MsGenericTemplateInner::Enum(ty) => ty.generate(&real_types, ms_module),
            MsGenericTemplateInner::Function(func) => func.generte(&real_types, ms_module),
        }
    }
}

#[derive(Clone, Debug)]
pub enum MsType {
    Native(MsNativeType),
    Struct(Rc<MsStructType>),
    Enum(Rc<MsEnumType>),
    Ref(Box<MsType>, bool),
    Function(Rc<MsDeclaredFunction>),
}

impl PartialEq for MsType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (MsType::Native(ty1), MsType::Native(ty2)) => ty1 == ty2,
            (MsType::Struct(ty1), MsType::Struct(ty2)) => Rc::ptr_eq(ty1, ty2),
            (MsType::Ref(ty1, mut1), MsType::Ref(ty2, mut2)) => ty1 == ty2 && mut1 == mut2,
            _ => false,
        }
    }
}

impl Hash for MsType {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            MsType::Native(ty) => state.write(ty.to_string().as_bytes()),
            _ => todo!(),
        }
    }
}

impl MsType {
    pub fn enum_ty(&self) -> Option<Rc<MsEnumType>> {
        match self {
            MsType::Enum(enum_ty) => Some(enum_ty.clone()),
            _ => None,
        }
    }

    pub fn native(&self) -> Option<MsNativeType> {
        match self {
            MsType::Native(ms_native_type) => Some(*ms_native_type),
            _ => None,
        }
    }
    pub fn struct_ty(&self) -> Option<Rc<MsStructType>> {
        match self {
            MsType::Struct(ms_struct) => Some(ms_struct.clone()),
            _ => None,
        }
    }

    pub fn equal(&self, ty: &MsType) -> bool {
        match (self, ty) {
            (MsType::Native(t0), MsType::Native(t1)) => t0 == t1,
            (MsType::Struct(t0), MsType::Struct(t1)) => Rc::ptr_eq(t0, t1),
            (MsType::Ref(t0, m0), MsType::Ref(t1, m1)) => m0 == m1 && t0 == t1,
            _ => false,
        }
    }

    pub fn size(&self) -> usize {
        match self {
            MsType::Native(ty) => ty.size(),
            MsType::Struct(ty) => ty.size(),
            MsType::Ref(_ty, _) => {
                log::warn!("using hardcoded ref(pointer) size of 8 bytes");
                8
            }
            _ => todo!(),
        }
    }

    pub fn align(&self) -> usize {
        match self {
            MsType::Native(ty) => ty.align(),
            MsType::Struct(ty) => ty.align(),
            _ => todo!(),
        }
    }

    pub fn to_abi_param(&self) -> Option<AbiParam> {
        match self {
            MsType::Native(ty) => ty.to_abi_param(),
            MsType::Struct(ty) => Some(ty.to_abi_param()),
            _ => todo!(),
        }
    }

    pub fn to_cl_type(&self) -> Option<Type> {
        match self {
            MsType::Native(ty) => ty.to_cl_type(),
            MsType::Ref(ty, _) => Some(types::I64),
            MsType::Struct(_) => Some(types::I64),
            _ => todo!(),
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            MsType::Native(ty) => ty.to_string().into(),
            // MsType::Ref(ty, is_mutable) => ty.to_string(),
            // MsType::Struct(ty) => {
            //     todo!("{:?}", ty)
            // }
            _ => todo!(),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MsTypeId(pub u32);

impl Display for MsTypeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{:?}", self))
    }
}

#[derive(Debug)]
pub struct MsTypeNameRegistry {
    map: HashMap<TypeNameWithGenerics, MsTypeId>, // map -> type_id
    inner_map: HashMap<MsTypeId, MsType>,
    // inner: Vec<MsType>,
}

#[derive(Debug, Clone)]
pub struct MsTypeWithId {
    pub id: MsTypeId,
    pub ty: MsType,
}

impl MsTypeNameRegistry {
    pub fn get_from_str(&self, s: &str) -> Option<MsTypeWithId> {
        let id = *self.map.get(&TypeNameWithGenerics::new(s.into(), vec![]))?;
        let ty = self.get_from_type_id(id)?;

        Some(MsTypeWithId { id, ty })
    }

    pub fn get_from_type_id(&self, id: MsTypeId) -> Option<MsType> {
        self.inner_map.get(&id).cloned()
        // self.inner.get(id.0).cloned()
    }

    pub fn get_type_id(&self, s: &str) -> Option<MsTypeId> {
        self.map
            .get(&TypeNameWithGenerics::new(s.into(), vec![]))
            .cloned()
    }

    pub fn add_type(&mut self, ty_name: impl Into<Box<str>>, ty: MsType) -> MsTypeId {
        let idx = MsTypeId(rand::random());

        // let idx = MsTypeId(self.inner.len());
        let ty_name: Box<str> = ty_name.into();

        let ty_name = TypeNameWithGenerics::new(ty_name, vec![]);

        log::info!("Added Type {:?} -> {:?} with type_id {}", ty_name, ty, idx);
        if self.map.insert(ty_name, idx).is_some() {
            panic!("A Type with that name already exists");
        }

        // self.inner.push(ty);

        self.inner_map.insert(idx.clone(), ty.clone());
        idx
    }

    pub fn add_alias(&mut self, ty_name: impl Into<Box<str>>, ty_id: MsTypeId) {
        let ty_name: Box<str> = ty_name.into();
        let ty_name = TypeNameWithGenerics::new(ty_name, vec![]);
        log::info!("Added Alias {:?} -> with type_id {}", ty_name, ty_id);
        if self.map.insert(ty_name, ty_id).is_some() {
            log::warn!("Already a type_name exists");
        }
    }

    pub fn with_default_types() -> Self {
        let mut registry = Self {
            map: Default::default(),
            inner_map: Default::default(),
        };

        registry.add_type("i16", MsType::Native(MsNativeType::I16));
        registry.add_type("i32", MsType::Native(MsNativeType::I32));
        registry.add_type("i64", MsType::Native(MsNativeType::I64));
        registry.add_type("u8", MsType::Native(MsNativeType::U8));
        registry.add_type("u16", MsType::Native(MsNativeType::U16));
        registry.add_type("u64", MsType::Native(MsNativeType::U64));
        registry.add_type("f32", MsType::Native(MsNativeType::F32));
        registry.add_type("f64", MsType::Native(MsNativeType::F64));

        let u32_ty = registry.add_type("u32", MsType::Native(MsNativeType::U32));
        let i8_ty = registry.add_type("i8", MsType::Native(MsNativeType::I8));
        registry.add_alias("bool", i8_ty);
        registry.add_alias("char", u32_ty);

        registry
    }
}

impl Default for MsTypeNameRegistry {
    fn default() -> Self {
        Self::with_default_types()
    }
}

#[derive(Debug)]
pub struct MsTypeRegistry {
    pub registry: HashMap<String, MsType>,
}

impl MsRegistry<MsType> for MsTypeRegistry {
    fn get_registry(&self) -> &HashMap<String, MsType> {
        &self.registry
    }

    fn get_registry_mut(&mut self) -> &mut HashMap<String, MsType> {
        &mut self.registry
    }
}

impl MsRegistryExt<MsType> for MsTypeRegistry {}

impl Default for MsTypeRegistry {
    fn default() -> Self {
        let mut registry = HashMap::<String, MsType>::with_capacity(512);
        registry.insert("i8".into(), MsType::Native(MsNativeType::I8));
        registry.insert("i16".into(), MsType::Native(MsNativeType::I16));
        registry.insert("i32".into(), MsType::Native(MsNativeType::I32));
        registry.insert("i64".into(), MsType::Native(MsNativeType::I64));
        registry.insert("u8".into(), MsType::Native(MsNativeType::U8));
        registry.insert("u32".into(), MsType::Native(MsNativeType::U32));
        registry.insert("u64".into(), MsType::Native(MsNativeType::U64));
        registry.insert("f32".into(), MsType::Native(MsNativeType::F32));
        registry.insert("f64".into(), MsType::Native(MsNativeType::F64));

        Self { registry }
    }
}
#[derive(Debug)]
pub struct MsTemplateRegistry {
    registry: HashMap<String, MsGenericTemplate>,
}

impl MsRegistry<MsGenericTemplate> for MsTemplateRegistry {
    fn get_registry(&self) -> &HashMap<String, MsGenericTemplate> {
        &self.registry
    }

    fn get_registry_mut(&mut self) -> &mut HashMap<String, MsGenericTemplate> {
        &mut self.registry
    }
}
impl MsRegistryExt<MsGenericTemplate> for MsTemplateRegistry {}

impl Default for MsTemplateRegistry {
    fn default() -> Self {
        let registry = HashMap::<String, MsGenericTemplate>::with_capacity(512);

        Self { registry }
    }
}

#[derive(Default, Debug)]
pub struct MsTypeTemplates {
    pub registry: HashMap<Box<str>, Rc<MsGenericTemplate>>,
}

pub trait TypeRegistryResolver {
    fn resolve_type(&self, name: &str) -> Option<MsTypeWithId>;
}

impl TypeRegistryResolver for MsTypeNameRegistry {
    fn resolve_type(&self, name: &str) -> Option<MsTypeWithId> {
        self.get_from_str(name)
    }
}

#[derive(Debug, Default)]
pub struct MsTypeMethodRegistry {
    pub map: HashMap<MsTypeId, MsFunctionRegistry>,
}

impl MsTypeMethodRegistry {
    pub fn add_method(&mut self, type_id: MsTypeId, name: impl Into<Box<str>>, func: Rc<MsDeclaredFunction>) {
        self.map.entry(type_id).or_default().add_function(name, func);
    }

    pub fn add_function(&mut self, type_id: MsTypeId, name: impl Into<Box<str>>, func: Rc<MsDeclaredFunction>) {
        self.add_method(type_id, name, func);
    }

    pub fn get_method(&self, type_id: MsTypeId, name: &str) -> Option<Rc<MsDeclaredFunction>> {
        self.map.get(&type_id)?.registry.get(name).cloned()
    }
}
