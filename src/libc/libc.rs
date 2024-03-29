use anyhow::anyhow;
use cranelift::{
    codegen::{
        entity::EntityRef,
        ir::{
            self, types, AbiParam, FuncRef, Inst, InstBuilder, MemFlags, UserExternalName,
            UserFuncName,
        },
        settings::{self, Configurable},
    },
    frontend::{FunctionBuilder, FunctionBuilderContext, Variable},
};
use cranelift_module::{default_libcall_names, DataDescription, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};

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
        for (i, c) in const_str.bytes().enumerate() {
            let val = builder.ins().iconst(types::I8, c as i64);
            builder
                .ins()
                .store(MemFlags::trusted(), val, malloc_ptr, i as i32);
        }
        let func_ref = module.declare_func_in_func(libc_puts_fn_id, &mut builder.func);
        let call_inst = builder.ins().call(func_ref, &[malloc_ptr]);
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
