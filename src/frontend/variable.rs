use std::collections::HashMap;

use cranelift::{
    codegen::{ir::Type, Context},
    frontend::{FunctionBuilder, FunctionBuilderContext},
    prelude::*,
};

use crate::utils::rc_str::RcStr;

use super::function::LocalVariables;

#[derive(Clone, Debug)]
pub struct MsVariable {
    pub name: RcStr,
    pub type_name: RcStr,
    pub backend_type: Type,
}

#[derive(Clone)]
pub struct MsLiteral {
    pub name: RcStr,
    pub literal: RcStr,
    pub backend_type: Type,
    pub type_name: RcStr,
}

impl MsVariable {
    // size in bytes
    pub fn new(
        variable_name: impl Into<RcStr>,
        type_name: impl Into<RcStr>,
        backend_type: Type,
    ) -> Self {
        Self {
            name: variable_name.into(),
            type_name: type_name.into(),
            backend_type,
        }
    }

    pub fn declare(
        &self,
        value: Value,
        local_variables: &mut LocalVariables,
        fbx: &mut FunctionBuilder<'_>,
    ) -> anyhow::Result<()> {
        let var = fbx.declare_var(self.backend_type);
        fbx.try_def_var(var, value)
            .map_err(|err| anyhow::anyhow!(err))?;
        local_variables.insert(
            self.name.clone(),
            MsBackendVariable {
                ms_var: self.clone(),
                c_var: var,
                c_val: value,
            },
        );
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct MsBackendVariable {
    pub ms_var: MsVariable,
    pub c_var: Variable,
    pub c_val: Value,
}

impl MsBackendVariable {
    pub fn take_value(&mut self, val: Value, fbx: &mut FunctionBuilder<'_>) {
        self.c_val = val;
        fbx.def_var(self.c_var, val);
    }
}
