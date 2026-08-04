use std::{collections::BTreeMap, rc::Rc};

use anyhow::anyhow;
use cranelift::{
    codegen::{
        ir::{
            types::I64, AbiParam, ExternalName, FuncRef, InstBuilder, Signature, UserExternalName,
            UserExternalNameRef, UserFuncName,
        },
        settings, Context,
    },
    frontend::{FunctionBuilder, FunctionBuilderContext},
    prelude::settings::{Configurable, Flags},
};

use cranelift_module::{
    default_libcall_names, DataDescription, FuncId, Linkage, Module, ModuleError,
};
use cranelift_object::{ObjectBuilder, ObjectModule};

use crate::utils::{rc_str::RcStr, rc_vec::RcVec};

use super::{
    function::Function,
    variable::{MsBackendVariable, MsVariable},
};

#[derive(Default)]
pub struct LocalVariables {
    pub map: BTreeMap<RcStr, MsBackendVariable>,
    pub parent: Option<Rc<LocalVariables>>,
}

impl LocalVariables {
    pub fn new() -> Self {
        Self::default()
    }
}

pub struct MsBackendFunction {
    pub func_id: FuncId,
    pub func_signature: Signature,
}

#[derive(Default)]
pub struct LocalFunctions {
    pub map: BTreeMap<RcStr, MsBackendFunction>,
    pub parent: Option<Rc<LocalFunctions>>,
}

#[derive(Default)]
pub struct MsContext {
    pub local_variables: LocalVariables,
    pub local_functions: LocalFunctions,
}

pub fn compile(functions: RcVec<Function>) -> anyhow::Result<Vec<u8>> {
    let data_description = DataDescription::new();
    let mut flag_builder = settings::builder();
    flag_builder.set("is_pic", "true");
    flag_builder.set("use_colocated_libcalls", "false");
    let flags = Flags::new(flag_builder);
    let isa_builder = cranelift_native::builder().map_err(|x| anyhow!(x))?;
    let isa = isa_builder.finish(flags)?;
    let libcalls = default_libcall_names();
    let mut module = ObjectModule::new(ObjectBuilder::new(isa.clone(), "main", libcalls)?);
    let mut fbx = FunctionBuilderContext::new();
    let mut ctx = module.make_context();

    // let print_i64_fn = declare_print_i64_fn(&mut module, &mut ctx)?;
    let mut ms_ctx = MsContext::default();
    // ms_ctx
    //     .local_functions
    //     .map
    //     .insert("print_i64".into(), print_i64_fn);

    for function in functions.iter() {
        if function.is_external {
            let mut sig = module.make_signature();
            sig.params = function
                .signature
                .params
                .iter()
                .map(|x| AbiParam::new(x.backend_type))
                .collect();
            if let Some(var) = &function.signature.returns {
                sig.returns.push(AbiParam::new(var.backend_type));
            }
            let extern_func_id =
                module.declare_function(&function.signature.name, Linkage::Import, &sig)?;
            let local_func_name = format!("{}_extern", function.signature.name);
            let local_func_id = module.declare_function(&local_func_name, Linkage::Local, &sig)?;

            ctx.func.signature = sig.clone();
            ctx.func.name = UserFuncName::user(0, local_func_id.as_u32());
            {
                let mut bcx = FunctionBuilder::new(&mut ctx.func, &mut fbx);
                let local_func = module.declare_func_in_func(extern_func_id, &mut bcx.func);
                let entry_block = bcx.create_block();
                bcx.append_block_params_for_function_params(entry_block);
                bcx.switch_to_block(entry_block);
                let values = bcx.block_params(entry_block).to_vec();
                bcx.ins().call(local_func, &values);
                bcx.ins().return_(&[]);

                bcx.seal_all_blocks();
                bcx.finalize();
            }

            module.define_function(local_func_id, &mut ctx)?;

            let func_id = local_func_id;
            ms_ctx.local_functions.map.insert(
                RcStr::from(local_func_name),
                MsBackendFunction {
                    func_id,
                    func_signature: sig.clone(),
                },
            );
        } else {
            function.declare(&mut ctx, &mut fbx, &mut ms_ctx)?;
            let linkage = Linkage::Export;

            let func_id =
                module.declare_function(&function.signature.name, linkage, &ctx.func.signature)?;

            print!(
                "Linkage: {:?} IsExternal: {}: Name: {} FuncId: {}\n",
                linkage, function.is_external, function.signature.name, func_id,
            );

            module.define_function(func_id, &mut ctx)?;

            ms_ctx.local_functions.map.insert(
                function.signature.name.clone(),
                MsBackendFunction {
                    func_id,
                    func_signature: ctx.func.signature.clone(),
                },
            );
        }

        module.clear_context(&mut ctx);
    }

    let obj = module.finish();
    let bytes = obj.emit()?;

    Ok(bytes)
}

// pub fn declare_print_i64_fn(
//     module: &mut ObjectModule,
//     context: &mut Context,
// ) -> Result<MsBackendFunction, ModuleError> {
//     let mut signature = Signature::new(cranelift::codegen::isa::CallConv::SystemV);
//     signature.params.push(AbiParam::new(I64));
//     let func_id = module.declare_function("print_i64", Linkage::Import, &signature)?;
//     module.define_function(func_id, context);

//     Ok(MsBackendFunction {
//         func_id,
//         func_signature: signature,
//     })
// }
