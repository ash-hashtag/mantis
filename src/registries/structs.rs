use std::{
    collections::{BTreeMap, HashMap},
    rc::Rc,
};

use cranelift::{
    codegen::ir::StackSlot,
    prelude::{
        isa::TargetFrontendConfig, types, AbiParam, FunctionBuilder, InstBuilder, MemFlags,
        StackSlotData,
    },
};
use cranelift_module::{DataDescription, FuncOrDataId, Linkage, Module};
use cranelift_object::ObjectModule;
use linear_map::LinearMap;

use crate::{
    backend::compile_function::compile_assignment_on_pointers,
    ms::MsContext,
    native::instructions::{Either, NodeResult},
};

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

    pub fn create_stack_slot(&mut self, fbx: &mut FunctionBuilder) -> StackSlot {
        fbx.create_sized_stack_slot(StackSlotData::new(
            cranelift::prelude::StackSlotKind::ExplicitSlot,
            self.size() as u32,
            0,
        ))
    }

    pub fn add_field(&mut self, field_name: impl Into<Box<str>>, field_type: MsTypeWithId) {
        let size = field_type.ty.size();
        let align = field_type.ty.align();

        let field_name: Box<str> = field_name.into();
        log::info!(
            "adding field {} of size {} and alignment {}",
            field_name,
            size,
            align
        );

        // let mut padding = 0;
        // if self.size % align != 0 {
        //     let padding = align - (self.size % align);
        //     log::info!("adding padding {} to current size {}", padding, self.size);
        //     self.size += padding;
        // }

        self.fields.insert(
            field_name,
            MsStructFieldValue {
                offset: self.size,
                ty: field_type.id,
            },
        );
        self.size += align;
    }

    pub fn get_field(&self, field_name: &str) -> Option<&MsStructFieldValue> {
        self.fields.get(field_name)
    }

    pub fn to_abi_param(&self) -> AbiParam {
        AbiParam::new(types::I64)
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

        let value = match field_ty {
            MsType::Native(nty) => fbx.ins().load(
                field_ty.to_cl_type().unwrap(),
                MemFlags::new(),
                ptr.value(),
                field.offset as i32,
            ),
            MsType::Struct(_sty) => fbx.ins().iadd_imm(ptr.value(), field.offset as i64),
            _ => todo!(),
        };

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

        match ms_ctx
            .current_module
            .type_registry
            .get_from_type_id(field.ty)
            .unwrap()
        {
            MsType::Native(nty) => {
                fbx.ins().store(
                    MemFlags::new(),
                    value.value(),
                    ptr.value(),
                    field.offset as i32,
                );
            }
            MsType::Struct(struct_ty) => {
                let dest = fbx.ins().iadd_imm(ptr.value(), field.offset as i64);
                struct_ty.copy(dest, value.value(), fbx, module, ms_ctx);
            }
            _ => todo!(),
        }
    }

    pub fn copy(
        &self,
        dest: cranelift::prelude::Value,
        src: cranelift::prelude::Value,
        fbx: &mut FunctionBuilder,
        module: &mut ObjectModule,
        ms_ctx: &MsContext,
    ) {
        let func_id = ms_ctx
            .current_module
            .fn_registry
            .registry
            .get("memcpy")
            .unwrap()
            .func_id;

        let func_ref = module.declare_func_in_func(func_id, fbx.func);
        let size = fbx.ins().iconst(types::I64, self.size() as i64);
        fbx.ins().call(func_ref, &[dest, src, size]);
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

pub fn call_memcpy(
    module: &mut ObjectModule,
    fbx: &mut FunctionBuilder,
    dest: cranelift::prelude::Value,
    src: cranelift::prelude::Value,
    size: cranelift::prelude::Value,
) {
    let Some(FuncOrDataId::Func(func)) = module.get_name("memcpy") else {
        unreachable!();
    };
    let func_ref = module.declare_func_in_func(func, fbx.func);
    fbx.ins().call(func_ref, &[dest, src, size]);
}

#[derive(Clone, Debug, Default)]
pub struct MsEnumType {
    variants: LinearMap<Box<str>, Option<MsTypeWithId>>,
    max_variant_size: usize,
}

impl MsEnumType {
    pub fn create_stack_slot(&mut self, fbx: &mut FunctionBuilder) -> StackSlot {
        fbx.create_sized_stack_slot(StackSlotData::new(
            cranelift::prelude::StackSlotKind::ExplicitSlot,
            self.size() as u32,
            0,
        ))
    }

    pub fn add_variant(
        &mut self,
        variant_name: impl Into<Box<str>>,
        variant_arg: Option<MsTypeWithId>,
    ) {
        if let Some(variant_arg) = variant_arg {
            self.max_variant_size = self.max_variant_size.max(variant_arg.ty.size());
            self.variants.insert(variant_name.into(), Some(variant_arg));
        } else {
            self.variants.insert(variant_name.into(), None);
        }
    }

    pub fn size(&self) -> usize {
        self.max_variant_size + 8 // extra i64 to store the tag of enum value
    }

    pub fn set_variant(
        &self,
        self_ptr: cranelift::prelude::Value,
        variant_name: &str,
        variant_arg: Option<NodeResult>,
        fbx: &mut FunctionBuilder,
        ms_ctx: &mut MsContext,
        module: &mut ObjectModule,
    ) {
        let mut found_variant = false;
        for (idx, (key, value)) in self.variants.iter().enumerate() {
            let key: &str = &key;
            if key == variant_name {
                found_variant = true;
                {
                    let value = fbx.ins().iconst(types::I64, idx as i64);
                    fbx.ins().store(MemFlags::new(), value, self_ptr, 0); // storing the tag
                }

                if let Some(value) = value {
                    let arg = variant_arg.unwrap();
                    assert!(arg.ty() == value.id);
                    let variant_ptr = fbx.ins().iadd_imm(self_ptr, 8);
                    let ty = ms_ctx
                        .current_module
                        .type_registry
                        .get_from_str("i64")
                        .unwrap();
                    let lhs = NodeResult::Val(MsVal::new(ty.id, variant_ptr)); // ptr
                    compile_assignment_on_pointers(lhs, arg, module, fbx, ms_ctx);
                }
                break;
            }
        }

        if !found_variant {
            panic!("undefiend variant {} on type {:?}", variant_name, self);
        }
    }

    pub fn to_abi_param(&self) -> AbiParam {
        AbiParam::new(types::I64)
    }

    pub fn to_cl_type(&self) -> types::Type {
        types::I64
    }

    pub fn get_tag_index(&self, variant_name: &str) -> Option<usize> {
        for (i, (var_name, _)) in self.variants.iter().enumerate() {
            let vname: &str = var_name;
            if vname == variant_name {
                return Some(i);
            }
        }

        return None;
    }

    pub fn get_tag(
        &self,
        ptr: cranelift::prelude::Value,
        fbx: &mut FunctionBuilder,
    ) -> cranelift::prelude::Value {
        fbx.ins().load(types::I64, MemFlags::new(), ptr, 0)
    }

    pub fn get_inner_ptr(
        &self,
        ptr: cranelift::prelude::Value,
        fbx: &mut FunctionBuilder,
    ) -> cranelift::prelude::Value {
        fbx.ins().iadd_imm(ptr, 8)
    }

    pub fn get_inner_ty(&self, variant_name: &str) -> Option<MsTypeWithId> {
        self.variants.get(variant_name).cloned()?
    }

    pub fn copy(
        &self,
        dest: cranelift::prelude::Value,
        src: cranelift::prelude::Value,
        fbx: &mut FunctionBuilder,
        module: &mut ObjectModule,
        ms_ctx: &mut MsContext,
    ) {
        let size = fbx.ins().iconst(types::I64, self.size() as i64);
        call_memcpy(module, fbx, dest, src, size);
    }
}
