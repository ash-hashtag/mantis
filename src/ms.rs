use cranelift::{
    codegen::ir::Inst,
    prelude::{EntityRef, FunctionBuilder, InstBuilder, Variable},
};
use cranelift_object::ObjectModule;

use crate::{
    registries::{
        functions::{MsFunctionRegistry, MsFunctionTemplates, MsTraitTemplates},
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
}

impl MsContext {
    pub fn new(offset: usize) -> Self {
        Self {
            current_module: Default::default(),
            variable_index: offset,
            var_scopes: Default::default(),
            loop_scopes: Default::default(),
            disable_auto_drop: false,
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
        let scope = self
            .loop_scopes
            .find_last_loop(name)
            .expect("unidentified loop");

        drop_scopes_until_index(scope.var_scope_index, self, fbx, module);

        fbx.ins().jump(scope.exit_block, &[])
    }
    pub fn continue_loop(
        &mut self,
        name: Option<&str>,
        fbx: &mut FunctionBuilder,
        module: &mut ObjectModule,
    ) -> Inst {
        let scope = self
            .loop_scopes
            .find_last_loop(name)
            .expect("unidentified loop");

        drop_scopes_until_index(scope.var_scope_index, self, fbx, module);

        fbx.ins().jump(scope.entry_block, &[])
    }
}
