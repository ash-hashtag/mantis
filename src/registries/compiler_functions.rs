use std::collections::HashMap;

use cranelift::prelude::{types, FunctionBuilder, InstBuilder, StackSlotData};
use cranelift_object::ObjectModule;
use mantis_parser::ast::Expr;

use crate::{frontend::tokens::MsNode, ms::MsContext, native::instructions::NodeResult};

use super::{
    types::MsType,
    variable::{MsVal, MsVar},
    MsRegistry, MsRegistryExt,
};

pub type MsCompilerFunction = Box<
    dyn for<'a> Fn(
        &'a [&'a MsNode],
        &'a mut MsContext,
        &'a mut FunctionBuilder,
        &'a mut ObjectModule,
    ) -> NodeResult,
>;

pub struct MsCompilerFunctionsRegistry {
    registry: HashMap<Box<str>, MsCompilerFunction>,
}

impl Default for MsCompilerFunctionsRegistry {
    fn default() -> Self {
        let mut registry = HashMap::<Box<str>, MsCompilerFunction>::new();

        // registry.insert("init".into(), Box::new(ms_init_fn));
        registry.insert("size_of".into(), Box::new(ms_size_of_fn));

        Self { registry }
    }
}

// fn ms_init_fn(
//     nodes: &[&MsNode],
//     ms_ctx: &mut MsContext,
//     fbx: &mut FunctionBuilder,
//     _module: &mut ObjectModule,
// ) -> NodeResult {
//     let Some(MsNode::Var(MantisLexerTokens::Word(ty_name))) = nodes.first() else {
//         panic!("Invalid type name or missing {:?}", nodes);
//     };

//     let ty = ms_ctx.type_registry.get(&ty_name).expect("undefined type");

//     ms_init_struct(ty.clone(), fbx)
// }
fn ms_size_of_fn(
    nodes: &[&MsNode],
    ms_ctx: &mut MsContext,
    fbx: &mut FunctionBuilder,
    _module: &mut ObjectModule,
) -> NodeResult {
    let Some(crate::frontend::tokens::MsNode::Ident(ident)) = nodes.first() else {
        panic!("Invalid type name or missing {:?}", nodes);
    };
    let ty_name = ident.name.clone();

    let ty = ms_ctx
        .current_module
        .type_registry
        .get_from_str(&ty_name)
        .expect("undefined type");

    let ty_i64 = ms_ctx
        .current_module
        .type_registry
        .get_from_str("i64")
        .unwrap();

    let value = fbx
        .ins()
        .iconst(ty_i64.ty.to_cl_type().unwrap(), ty.ty.size() as i64);

    NodeResult::Val(MsVal::new(ty_i64.id, value))

    // ms_size_of(ty.clone(), fbx)
}

// fn ms_init_struct(ty: MsType, fbx: &mut FunctionBuilder) -> NodeResult {
//     let stack_slot = fbx.create_sized_stack_slot(StackSlotData::new(
//         cranelift::prelude::StackSlotKind::ExplicitSlot,
//         ty.size() as u32,
//     ));

//     let ptr = fbx.ins().stack_addr(types::I64, stack_slot, 0);
//     NodeResult::Val(MsVal::new(ptr, ty, "i64"))
// }

// fn ms_size_of(ty: MsType, fbx: &mut FunctionBuilder) -> NodeResult {
//     let size = ty.size();
//     let value = fbx.ins().iconst(types::I64, size as i64);
//     NodeResult::Val(MsVal::new(
//         value,
//         MsType::Native(super::types::MsNativeType::I64),
//         "i64",
//     ))
// }
