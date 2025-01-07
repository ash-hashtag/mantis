use std::{
    collections::{BTreeMap, HashMap},
    rc::Rc,
};

use cranelift::prelude::{types, AbiParam, FunctionBuilder, InstBuilder, MemFlags};
use cranelift_module::{DataDescription, Linkage, Module};
use cranelift_object::ObjectModule;
use linear_map::LinearMap;

use crate::{frontend::tokens::MsContext, native::instructions::Either};

use super::{
    functions::MsDeclaredFunction,
    types::{MsGenericTemplate, MsType, MsTypeId, MsTypeWithId},
    variable::MsVal,
    MsRegistry,
};

#[derive(Debug, Clone)]
pub struct MsStructFieldValue {
    pub offset: usize,
    pub ty: MsTypeId,
}

#[derive(Clone, Debug, Default)]
pub struct MsStructType {
    fields: HashMap<Box<str>, MsStructFieldValue>,
    size: usize,
}

impl MsStructType {
    pub fn size(&self) -> usize {
        self.size
    }

    pub fn align(&self) -> usize {
        if self.size % 8 == 0 {
            self.size
        } else {
            self.size + (8 - self.size % 8)
        }
    }

    pub fn add_field(&mut self, field_name: impl Into<Box<str>>, field_type: MsTypeWithId) {
        let size = field_type.ty.size();
        let align = field_type.ty.align();

        if self.size % align != 0 {
            self.size += align - (self.size % align);
        }

        self.fields.insert(
            field_name.into(),
            MsStructFieldValue {
                offset: self.size,
                ty: field_type.id,
            },
        );
        self.size += size;
    }

    pub fn get_field(&self, field_name: &str) -> Option<&MsStructFieldValue> {
        self.fields.get(field_name)
    }

    pub fn to_abi_param(&self) -> AbiParam {
        todo!()
    }

    pub fn set_data(
        &self,
        ptr: MsVal,
        values: HashMap<Box<str>, MsVal>,
        ms_ctx: &mut MsContext,
        fbx: &mut FunctionBuilder<'_>,
        module: &mut ObjectModule,
    ) {
        for (k, v) in self.fields.iter() {
            let val = values.get(k).expect("Missing field on struct");

            if v.ty != val.ty_id {
                panic!("Types don match {:?} != {:?}", v.ty, val.ty_id);
            }
            self.set_field(&ptr, &k, val, ms_ctx, fbx, module);
        }
    }

    pub fn get_data(
        &self,
        ptr: MsVal,
        field: &str,
        ms_ctx: &mut MsContext,
        fbx: &mut FunctionBuilder<'_>,
        module: &mut ObjectModule,
    ) -> MsVal {
        let field = self
            .fields
            .get(field)
            .expect(&format!("undefined field name on {:?}", self));

        let field_ty = ms_ctx
            .current_module
            .type_registry
            .get_from_type_id(field.ty)
            .unwrap();

        let value = fbx.ins().load(
            field_ty.to_cl_type().unwrap(),
            MemFlags::new(),
            ptr.value(),
            field.offset as i32,
        );

        return MsVal::new(field.ty, value);
    }

    pub fn get_method(
        ptr: MsVal,
        field: &str,
        ms_ctx: &mut MsContext,
        fbx: &mut FunctionBuilder<'_>,
        module: &mut ObjectModule,
    ) -> Rc<MsDeclaredFunction> {
        todo!()
    }

    pub fn set_field(
        &self,
        ptr: &MsVal,
        field_name: &str,
        value: &MsVal,
        ms_ctx: &mut MsContext,
        fbx: &mut FunctionBuilder<'_>,
        module: &mut ObjectModule,
    ) {
        let field = self.get_field(field_name).unwrap();
        fbx.ins().store(
            MemFlags::new(),
            value.value(),
            ptr.value(),
            field.offset as i32,
        );
    }
}

// pub fn array_struct() -> MsStructType {
//     let mut st = MsStructType::default();
//     st.add_field("size", MsType::Native(super::types::MsNativeType::U64));
//     st.add_field("ptr", MsType::Native(super::types::MsNativeType::U64));
//     return st;
// }

// pub fn pointer_template() -> MsGenericTemplate {
//     let mut st = MsGenericTemplate::default();
//     st.add_generic("T");
//     st.add_field(
//         "ptr",
//         Either::Left(MsType::Native(super::types::MsNativeType::U64)),
//     );
//     st
// }

// pub fn array_template() -> MsGenericTemplate {
//     let mut st = MsGenericTemplate::default();
//     st.add_generic("T");
//     st.add_field(
//         "size",
//         Either::Left(MsType::Native(super::types::MsNativeType::U64)),
//     );
//     st.add_field(
//         "ptr",
//         Either::Left(MsType::Native(super::types::MsNativeType::U64)),
//     );
//     return st;
// }
