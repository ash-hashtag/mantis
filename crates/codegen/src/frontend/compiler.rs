use cranelift::{
    codegen::settings::{self, Configurable, Flags},
    frontend::FunctionBuilderContext,
};
use cranelift_module::{default_libcall_names, DataDescription, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};

use crate::{
    // libc::libc::link_libc_functions,
    registries::{functions::MsFunctionRegistry, types::MsTypeRegistry},
};

use super::tokens::{FunctionDeclaration, MsFunctionDeclaration, StructRegistry};
use anyhow::anyhow;

// pub fn compile(
//     functions: Vec<FunctionDeclaration>,
//     struct_registry: StructRegistry,
// ) -> anyhow::Result<Vec<u8>> {
//     let data_description = DataDescription::new();
//     let mut flag_builder = settings::builder();
//     flag_builder.set("is_pic", "true");
//     flag_builder.set("use_colocated_libcalls", "false");
//     let flags = Flags::new(flag_builder);
//     let isa_builder = cranelift_native::builder().map_err(|x| anyhow!(x))?;
//     let isa = isa_builder.finish(flags)?;
//     let libcalls = default_libcall_names();
//     let mut module = ObjectModule::new(ObjectBuilder::new(isa.clone(), "main", libcalls)?);
//     let mut fbx = FunctionBuilderContext::new();
//     let mut ctx = module.make_context();

//     // let print_i64_fn = declare_print_i64_fn(&mut module, &mut ctx)?;

//     // let libc_fns = link_libc_functions(&mut module, &mut fbx, &mut ctx).unwrap();
//     // println!("LIBC FNS: {:?}", libc_fns);

//     let mut ms_ctx = MsContext::new(0);
//     ms_ctx.set_struct_registry(struct_registry);
//     for function in functions {
//         function.declare(&mut ctx, &mut fbx, &mut module, &mut ms_ctx);
//         ms_ctx.clear_scopes();
//         ms_ctx.clear_variables();
//     }

//     let obj = module.finish();

//     let bytes = obj.emit()?;

//     Ok(bytes)
// }

pub fn ms_compile(
    functions: Vec<MsFunctionDeclaration>,
    type_registry: MsTypeRegistry,
    fn_registry: MsFunctionRegistry,
) -> anyhow::Result<Vec<u8>> {
    // let data_description = DataDescription::new();
    // let mut flag_builder = settings::builder();
    // flag_builder.set("is_pic", "true");
    // flag_builder.set("use_colocated_libcalls", "false");
    // let flags = Flags::new(flag_builder);
    // let isa_builder = cranelift_native::builder().map_err(|x| anyhow!(x))?;
    // let isa = isa_builder.finish(flags)?;
    // let libcalls = default_libcall_names();
    // let mut module = ObjectModule::new(ObjectBuilder::new(isa.clone(), "main", libcalls)?);
    // let mut fbx = FunctionBuilderContext::new();
    // let mut ctx = module.make_context();

    // let mut ms_ctx = MsContext::new(0);
    // ms_ctx.type_registry = type_registry;
    // ms_ctx.fn_registry = fn_registry;
    // for function in functions {
    //     let fn_id = function.declare(&mut ctx, &mut fbx, &mut module, &mut ms_ctx);
    //     log::info!("Function Declared {} func id: {}", function.name, fn_id);
    // }

    // let obj = module.finish();

    // let bytes = obj.emit()?;

    // Ok(bytes)
    todo!()
}
