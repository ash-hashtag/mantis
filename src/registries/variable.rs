use std::collections::HashMap;

use cranelift::codegen::ir::StackSlot;
use cranelift::prelude::{FunctionBuilder, InstBuilder, Value, Variable};

use super::{
    types::{MsType, MsTypeId},
    MsRegistry, MsRegistryExt,
};

#[derive(Clone, Debug)]
pub struct MsVar {
    pub c_var: Variable,
    pub stack_slot: Option<StackSlot>,
    pub ty_id: MsTypeId,
    pub is_mutable: bool,
    pub is_reference: bool,
}

impl MsVar {
    pub fn new(
        ty_id: MsTypeId,
        c_var: Variable,
        stack_slot: Option<StackSlot>,
        is_mutable: bool,
        is_reference: bool,
    ) -> Self {
        Self {
            ty_id,
            c_var,
            stack_slot,
            is_mutable,
            is_reference,
        }
    }

    pub fn value(&self, fbx: &mut FunctionBuilder, ms_ctx: &crate::ms::MsContext) -> Value {
        let ty = ms_ctx
            .current_module
            .type_registry
            .get_from_type_id(self.ty_id);
        if let Some(crate::registries::types::MsType::Ref(_, _)) = ty {
            if let Some(ss) = self.stack_slot {
                let ptr = fbx.ins().stack_addr(cranelift::prelude::types::I64, ss, 0);
                return fbx.ins().load(
                    cranelift::prelude::types::I64,
                    cranelift::prelude::MemFlags::new(),
                    ptr,
                    0,
                );
            }
        }
        fbx.use_var(self.c_var)
    }
}

#[derive(Clone, Debug)]
pub struct MsVal {
    pub value: Value,
    pub ty_id: MsTypeId,
}

impl MsVal {
    pub fn new(ty_id: MsTypeId, value: Value) -> Self {
        Self { ty_id, value }
    }

    pub fn value(&self) -> Value {
        self.value
    }
}

#[derive(Default, Clone, Debug)]
pub struct MsVarRegistry {
    pub registry: HashMap<Box<str>, MsVar>, // variable name -> variable type name
    pub stack: Vec<Box<str>>,
}
