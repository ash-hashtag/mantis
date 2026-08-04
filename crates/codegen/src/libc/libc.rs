use std::collections::BTreeMap;

use anyhow::anyhow;
use cranelift::{
    codegen::{
        entity::EntityRef,
        ir::{
            self,
            types::{self, I64},
            AbiParam, FuncRef, GlobalValueData, Inst, InstBuilder, MemFlags, Signature,
            UserExternalName, UserFuncName,
        },
        settings::{self, Configurable},
    },
    frontend::{FunctionBuilder, FunctionBuilderContext, Variable},
};
use cranelift_module::{default_libcall_names, DataDescription, FuncId, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};

// pub fn link_libc_functions(
//     module: &mut ObjectModule,
//     fbx: &mut FunctionBuilderContext,
//     ctx: &mut cranelift::prelude::codegen::Context,
// ) -> anyhow::Result<BTreeMap<String, FuncId>> {
//     let mut signature = Signature::new(cranelift::codegen::isa::CallConv::Fast);
//     signature.params.push(AbiParam::new(types::I64));
//     signature.returns.push(AbiParam::new(types::I64));

//     let mut map = BTreeMap::new();
//     let libc_malloc_name = "libc_malloc";
//     let libc_malloc = declare_external_function(
//         "malloc",
//         &libc_malloc_name,
//         signature.clone(),
//         module,
//         fbx,
//         ctx,
//     )?;

//     map.insert(libc_malloc_name.into(), libc_malloc);

//     // signature.returns.clear();
//     // signature.returns.push(AbiParam::new(types::I32));
//     // let libc_puts_name = "libc_puts";
//     // let libc_puts =
//     //     declare_external_function("puts", &libc_puts_name, signature.clone(), module, fbx, ctx)?;

//     // map.insert(libc_puts_name.into(), libc_puts);

//     Ok(map)
// }

pub fn declare_external_function(
    fn_name: &str,
    wrapper_fn_name: &str,
    fn_signature: Signature,
    module: &mut ObjectModule,
    fbx: &mut FunctionBuilderContext,
    ctx: &mut cranelift::prelude::codegen::Context,
) -> anyhow::Result<FuncId> {
    ctx.func.signature = fn_signature.clone();
    let func_id = module
        .declare_function(fn_name, Linkage::Import, &ctx.func.signature)
        .unwrap();
    let mut builder = FunctionBuilder::new(&mut ctx.func, fbx);
    let entry_block = builder.create_block();
    builder.append_block_params_for_function_params(entry_block);
    builder.switch_to_block(entry_block);
    let func_ref = module.declare_func_in_func(func_id, &mut builder.func);
    let params = builder.block_params(entry_block).to_vec();
    let inst = builder.ins().call(func_ref, &params);
    let results = builder.inst_results(inst).to_vec();
    builder.ins().return_(&results);
    builder.seal_all_blocks();
    builder.finalize();

    let id = module.declare_function(wrapper_fn_name, Linkage::Local, &ctx.func.signature)?;

    module.define_function(id, ctx)?;

    module.clear_context(ctx);

    log::info!(
        "External Function Declared {} {} {}",
        fn_name,
        wrapper_fn_name,
        func_id
    );

    Ok(id)
}

pub fn malloc_test() -> anyhow::Result<Vec<u8>> {
    let data_description = DataDescription::new();
    let mut flag_builder = settings::builder();
    flag_builder.set("is_pic", "true");
    flag_builder.set("use_colocated_libcalls", "false");
    let isa_builder = cranelift_native::builder().map_err(|x| anyhow!(x))?;
    let isa = isa_builder.finish(settings::Flags::new(flag_builder))?;
    let libcalls = default_libcall_names();
    let mut module = ObjectModule::new(ObjectBuilder::new(isa.clone(), "main", libcalls)?);
    let mut fn_builder_ctx = FunctionBuilderContext::new();
    let mut ctx = module.make_context();

    // Defining Malloc Signature
    let libc_malloc_fn_id = {
        ctx.func.signature.params.push(AbiParam::new(types::I64));
        ctx.func.signature.returns.push(AbiParam::new(types::I64));
        let func_id = module.declare_function("malloc", Linkage::Import, &ctx.func.signature)?;
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut fn_builder_ctx);
        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        let func_ref = module.declare_func_in_func(func_id, &mut builder.func);
        let malloc_size = builder.block_params(entry_block)[0];
        let inst = builder.ins().call(func_ref, &[malloc_size]);
        let result = builder.inst_results(inst)[0];
        builder.ins().return_(&[result]);

        builder.seal_all_blocks();
        builder.finalize();

        let id = module.declare_function("libc_malloc", Linkage::Local, &ctx.func.signature)?;

        module.define_function(id, &mut ctx)?;

        module.clear_context(&mut ctx);

        id
    };

    let libc_puts_fn_id = {
        ctx.func.signature.params.push(AbiParam::new(types::I64));
        ctx.func.signature.returns.push(AbiParam::new(types::I32));
        let func_id = module.declare_function("puts", Linkage::Import, &ctx.func.signature)?;
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut fn_builder_ctx);
        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        let func_ref = module.declare_func_in_func(func_id, &mut builder.func);
        let char_ptr = builder.block_params(entry_block)[0];
        let inst = builder.ins().call(func_ref, &[char_ptr]);
        let result = builder.inst_results(inst)[0];
        builder.ins().return_(&[result]);

        builder.seal_all_blocks();
        builder.finalize();

        let id = module.declare_function("libc_puts", Linkage::Local, &ctx.func.signature)?;

        module.define_function(id, &mut ctx)?;

        module.clear_context(&mut ctx);

        id
    };

    let main_func_id = {
        let mut signature = module.make_signature();
        signature.params.push(AbiParam::new(types::I32));
        signature.params.push(AbiParam::new(types::I64));
        signature.returns.push(AbiParam::new(types::I32));
        ctx.func.signature = signature.clone();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut fn_builder_ctx);
        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);

        let argc = builder.block_params(entry_block)[0];
        let size_val = builder.ins().iconst(types::I64, 1024);
        let func_ref = module.declare_func_in_func(libc_malloc_fn_id, &mut builder.func);
        let call_inst = builder.ins().call(func_ref, &[size_val]);
        let malloc_ptr = builder.inst_results(call_inst)[0];
        let const_str = "Hello World\n\0";
        let data_id = module.declare_data("const_str", Linkage::Local, false, false)?;
        let mut data_description = DataDescription::new();
        data_description.define(const_str.as_bytes().into());
        module.define_data(data_id, &data_description)?;
        // let string_ptr = malloc_ptr;
        // for (i, c) in const_str.bytes().enumerate() {
        //     let val = builder.ins().iconst(types::I8, c as i64);
        //     builder
        //         .ins()
        //         .store(MemFlags::trusted(), val, malloc_ptr, i as i32);
        // }
        let string_ptr = module.declare_data_in_func(data_id, builder.func);
        let func_ref = module.declare_func_in_func(libc_puts_fn_id, &mut builder.func);
        let string_ptr = builder.ins().global_value(I64, string_ptr);
        let call_inst = builder.ins().call(func_ref, &[string_ptr]);
        let puts_result = builder.inst_results(call_inst)[0];
        builder.ins().return_(&[puts_result]);
        builder.seal_all_blocks();
        builder.finalize();
        let func_id = module.declare_function("main", Linkage::Export, &signature)?;
        module.define_function(func_id, &mut ctx)?;
        module.clear_context(&mut ctx);
        func_id
    };

    let obj = module.finish();
    let bytes = obj.emit()?;

    Ok(bytes)
}
