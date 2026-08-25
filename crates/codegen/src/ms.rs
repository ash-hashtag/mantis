use crate::registries::types::MsTypeId;
use cranelift::{
    codegen::ir::Inst,
    prelude::{EntityRef, FunctionBuilder, InstBuilder, Variable},
};
use cranelift_module::DataId;
use cranelift_object::ObjectModule;
use std::collections::HashMap;

#[derive(Clone, Copy)]
pub struct MsGlobal {
    pub data_id: DataId,
    pub ty_id: MsTypeId,
    pub is_const: bool,
}

use crate::{
    registries::{
        functions::{MsFunctionRegistry, MsFunctionTemplates, MsInstantiation, MsTraitTemplates},
        modules::{MsModule, MsModuleRegistry},
        traits::MsTraitRegistry,
        types::{MsTypeRegistry, MsTypeTemplates},
    },
    scope::{drop_scopes_until_index, MsLoopScope, MsLoopScopes, MsScopes, MsVarScopes},
};

pub struct MsContext {
    variable_index: usize,
    pub var_scopes: MsVarScopes,
    pub loop_scopes: MsLoopScopes,
    pub current_module: MsModule,
    pub disable_auto_drop: bool,
    pub instantiation_queue: Vec<MsInstantiation>,
    pub globals: HashMap<Box<str>, MsGlobal>,
}

impl MsContext {
    pub fn new(offset: usize) -> Self {
        Self {
            current_module: Default::default(),
            variable_index: offset,
            var_scopes: Default::default(),
            loop_scopes: Default::default(),
            disable_auto_drop: false,
            instantiation_queue: vec![],
            globals: HashMap::new(),
        }
    }

    pub fn new_variable(&mut self) -> Variable {
        self.variable_index += 1;
        Variable::new(self.variable_index)
    }

    pub fn new_loop_scope(
        &mut self,
        name: Option<Box<str>>,
        fbx: &mut FunctionBuilder,
    ) -> &MsLoopScope {
        let scope_index = self.var_scopes.scopes.len();
        self.loop_scopes.new_loop(name, fbx, scope_index)
    }

    pub fn break_out_of_loop(
        &mut self,
        name: Option<&str>,
        fbx: &mut FunctionBuilder,
        module: &mut ObjectModule,
    ) -> Inst {
        let var_scope_index = {
            let scope = self
                .loop_scopes
                .find_last_loop(name)
                .expect("unidentified loop");
            scope.var_scope_index
        };

        drop_scopes_until_index(var_scope_index, self, fbx, module);

        fbx.ins().jump(self.loop_scopes.find_last_loop(name).expect("unidentified loop").exit_block, &[])
    }
    pub fn continue_loop(
        &mut self,
        name: Option<&str>,
        fbx: &mut FunctionBuilder,
        module: &mut ObjectModule,
    ) -> Inst {
        let (var_scope_index, entry_block) = {
            let scope = self
                .loop_scopes
                .find_last_loop(name)
                .expect("unidentified loop");
            (scope.var_scope_index, scope.entry_block)
        };

        drop_scopes_until_index(var_scope_index, self, fbx, module);

        fbx.ins().jump(entry_block, &[])
    }
}
