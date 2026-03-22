use cranelift::prelude::*;
use cranelift_module::{DataDescription, FuncOrDataId, Linkage, Module};
use cranelift_object::ObjectModule;

use crate::registries::{
    types::{MsType, MsTypeId},
    variable::{MsVal, MsVar},
};

#[derive(Debug, Clone)]
pub enum Either<L, R> {
    Left(L),
    Right(R),
}

#[derive(Debug, Clone)]
pub enum NodeResult {
    Val(MsVal),
    Var(MsVar),
    StructAccessVar { ptr: MsVal, offset: u32 },
}

impl NodeResult {
    pub fn value(&self, fbx: &mut FunctionBuilder) -> Value {
        match self {
            NodeResult::Val(val) => val.value,
            NodeResult::Var(var) => fbx.use_var(var.c_var),
            // NodeResult::StructAccessVar { ptr, offset } => {
            //     let value = ptr.value();
            //     fbx.ins().load(
            //         ptr.ty.to_cl_type().unwrap(),
            //         MemFlags::new(),
            //         value,
            //         *offset as i32,
            //     )
            // }
            _ => todo!(),
        }
    }

    pub fn ty(&self) -> MsTypeId {
        match self {
            NodeResult::Val(val) => val.ty_id,
            NodeResult::Var(var) => var.ty_id,
            NodeResult::StructAccessVar { ptr, offset } => ptr.ty_id,
        }
    }

    // pub fn type_name(&self) -> &str {
    //     match self {
    //         NodeResult::Val(ms_val) => ms_val.type_name(),
    //         NodeResult::Var(ms_var) => ms_var.type_name(),
    //         NodeResult::StructAccessVar { ptr, offset } => todo!(),
    //     }
    // }

    pub fn to_ms_val(&self, fbx: &mut FunctionBuilder) -> MsVal {
        let ty = self.ty().clone();
        let val = self.value(fbx);
        MsVal::new(ty, val)
    }
}

// pub fn translate_node(
//     node: &MsNode,
//     ms_ctx: &mut MsContext,
//     fbx: &mut FunctionBuilder<'_>,
//     module: &mut ObjectModule,
// ) -> NodeResult {
//     use mantis_expression::node::Node;

//     match node {
//         Node::Binary(op, lhs, rhs) => {
//             return translate_binary_op(
//                 op.clone(),
//                 lhs.as_ref(),
//                 rhs.as_ref(),
//                 ms_ctx,
//                 fbx,
//                 module,
//             );
//         }
//         Node::Var(var_token) => {
//             match var_token {
//                 MantisLexerTokens::Word(var_name) => {
//                     let var = ms_ctx
//                         .scopes
//                         .get_variable(var_name)
//                         .expect("Undeclared variable");

//                     return NodeResult::Var(MsVar::new(var.ty.clone(), var.c_var));
//                 }
//                 MantisLexerTokens::Integer(int) => {
//                     let val = fbx.ins().iconst(types::I64, *int);
//                     let ty = MsType::Native(MsNativeType::I64);
//                     return NodeResult::Val(MsVal::new(val, ty));
//                 }

//                 MantisLexerTokens::Float(float) => {
//                     let val = fbx.ins().f64const(*float);
//                     let ty = MsType::Native(MsNativeType::F64);
//                     return NodeResult::Val(MsVal::new(val, ty));
//                 }

//                 MantisLexerTokens::String(s) => {
//                     let data_id = if let Some(FuncOrDataId::Data(data_id)) = module.get_name(s) {
//                         data_id
//                     } else {
//                         let data_id = module
//                             .declare_data(s, Linkage::Local, false, false)
//                             .unwrap();
//                         let mut data_description = DataDescription::new();
//                         data_description.define(s.as_bytes().into());
//                         module.define_data(data_id, &data_description);

//                         // let gl_value = module.declare_data_in_func(data_id, fbx.func);
//                         // let val = fbx.ins().global_value(types::I64, gl_value);

//                         // let st = ms_ctx.type_registry.get("array").unwrap();
//                         data_id
//                     };

//                     let gl_value = module.declare_data_in_func(data_id, fbx.func);
//                     ms_ctx.type_registry.get("array").unwrap().clone();
//                 }
//                 _ => panic!("Unsupported variable token {:?}", var_token),
//             };
//         }
//         Node::Expr(inner_node) => return translate_node(inner_node.as_ref(), ms_ctx, fbx, module),
//         _ => {}
//     };

//     let null = fbx.ins().null(types::I32);
//     log::info!("Somewhere we got null {:?}", node);
//     NodeResult::Val(MsVal::new(null, MsType::Native(MsNativeType::Void)))
// }
