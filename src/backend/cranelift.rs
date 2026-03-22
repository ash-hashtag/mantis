use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use crate::lexer::ast::Program;
use anyhow::anyhow;
use codegen::ir::{condcodes, Inst};
use cranelift::{
    codegen::{
        ir::{
            types::{I64},
            UserExternalName, UserFuncName,
        },
        isa::{CallConv, TargetFrontendConfig, TargetIsa},
        Context,
    },
    prelude::*,
};
use cranelift_module::{
    default_libcall_names, DataDescription, FuncId, FuncOrDataId, Linkage, Module,
};
use cranelift_object::{ObjectBuilder, ObjectModule};
use types::I32;

#[test]
fn test_cranelift() -> anyhow::Result<()> {
    let data_description = DataDescription::new();
    let mut flag_builder = settings::builder();
    flag_builder.set("preserve_frame_pointers", "true");

    let isa_builder = cranelift_native::builder().map_err(|x| anyhow!(x))?;
    let isa = isa_builder.finish(settings::Flags::new(flag_builder))?;
    let libcalls = default_libcall_names();
    let mut module = ObjectModule::new(ObjectBuilder::new(isa.clone(), "main", libcalls)?);
    let mut fn_builder_ctx = FunctionBuilderContext::new();

    let mut ctx = module.make_context();

    build_loop_fn(&mut ctx, &mut fn_builder_ctx);
    let func_id =
        module.declare_function("loop_function", Linkage::Preemptible, &ctx.func.signature)?;
    module.define_function(func_id, &mut ctx)?;
    module.clear_context(&mut ctx);

    build_ifelse_main_fn(&mut ctx, &mut fn_builder_ctx);
    let func_id =
        module.declare_function("ifelse_function", Linkage::Preemptible, &ctx.func.signature)?;
    module.define_function(func_id, &mut ctx)?;
    module.clear_context(&mut ctx);

    create_foo_fn(&mut module, &mut ctx, &mut fn_builder_ctx);
    let func_id = module.declare_function("create_foo", Linkage::Export, &ctx.func.signature)?;
    module.define_function(func_id, &mut ctx)?;
    module.clear_context(&mut ctx);

    create_small_struct_fn(&mut ctx, &mut fn_builder_ctx);
    let func_id =
        module.declare_function("create_small_struct", Linkage::Export, &ctx.func.signature)?;
    module.define_function(func_id, &mut ctx)?;
    module.clear_context(&mut ctx);

    sum_the_foo_fn(&mut ctx, &mut fn_builder_ctx);
    let func_id = module.declare_function("sum_foo", Linkage::Export, &ctx.func.signature)?;
    module.define_function(func_id, &mut ctx)?;
    module.clear_context(&mut ctx);

    sum_from_foo_ptr_fn(&mut ctx, &mut fn_builder_ctx);
    let func_id = module.declare_function("sum_foo_ptr", Linkage::Export, &ctx.func.signature)?;
    module.define_function(func_id, &mut ctx)?;
    module.clear_context(&mut ctx);

    anonymous_fn_builder(&mut module, &mut ctx, &mut fn_builder_ctx);
    let func_id = module.declare_function("anonymous_fn", Linkage::Export, &ctx.func.signature)?;
    module.define_function(func_id, &mut ctx)?;
    module.clear_context(&mut ctx);

    // let func_id = module.declare_function("recursive_fn", Linkage::Export, &ctx.func.signature)?;
    recursive_fn_builder(&mut module, &mut ctx, &mut fn_builder_ctx);
    // module.define_function(func_id, &mut ctx)?;
    // module.clear_context(&mut ctx);

    // build_main_fn(&mut module, &mut ctx, &mut fn_builder_ctx);
    // let func_id = module.declare_function("main", Linkage::Preemptible, &ctx.func.signature)?;
    // module.define_function(func_id, &mut ctx)?;
    // module.clear_context(&mut ctx);

    let object_product = module.finish();

    let bytes = object_product.emit()?;

    std::fs::write("/tmp/output.o", bytes)?;

    Ok(())
}

pub struct IfElseChainBuilder {
    else_block: Option<Block>,
    end_block: Block,
}

impl IfElseChainBuilder {
    pub fn new_block(
        fbx: &mut FunctionBuilder,
        cond: impl FnOnce(&mut FunctionBuilder) -> Value,
        block_instruction_builder: impl FnOnce(&mut FunctionBuilder) -> Option<Inst>,
    ) -> Self {
        let if_block = fbx.create_block();
        let else_block = fbx.create_block();
        let end_block = fbx.create_block();

        let value = cond(fbx);
        fbx.ins().brif(value, if_block, &[], else_block, &[]);
        fbx.switch_to_block(if_block);
        if block_instruction_builder(fbx).is_none() {
            fbx.ins().jump(end_block, &[]);
        }
        fbx.seal_block(if_block);

        Self {
            else_block: Some(else_block),
            end_block,
        }
    }

    pub fn elseif_block(
        &mut self,
        fbx: &mut FunctionBuilder,
        cond: impl FnOnce(&mut FunctionBuilder) -> Value,
        block_instruction_builder: impl FnOnce(&mut FunctionBuilder) -> Option<Inst>,
    ) {
        let if_block = fbx.create_block();
        let else_block = fbx.create_block();
        let previous_else_block = self.else_block.replace(else_block).unwrap();
        let end_block = self.end_block;
        fbx.switch_to_block(previous_else_block);
        let value = cond(fbx);
        fbx.ins().brif(value, if_block, &[], else_block, &[]);
        fbx.seal_block(previous_else_block);

        fbx.switch_to_block(if_block);
        if block_instruction_builder(fbx).is_none() {
            fbx.ins().jump(end_block, &[]);
        }
        fbx.seal_block(if_block);
    }

    pub fn else_block(
        &mut self,
        fbx: &mut FunctionBuilder,
        block_instruction_builder: impl FnOnce(&mut FunctionBuilder) -> Option<Inst>,
    ) {
        let else_block = self.else_block.take().unwrap();
        fbx.switch_to_block(else_block);

        if block_instruction_builder(fbx).is_none() {
            fbx.ins().jump(self.end_block, &[]);
        }
        fbx.seal_block(else_block);
    }

    pub fn end(&mut self, fbx: &mut FunctionBuilder) {
        if self.else_block.is_some() {
            self.else_block(fbx, |_f| None);
        }
        fbx.switch_to_block(self.end_block);
        fbx.seal_block(self.end_block);
    }
}

fn build_ifelse_main_fn(ctx: &mut Context, fbx: &mut FunctionBuilderContext) {
    ctx.func.signature.params = vec![AbiParam::new(I32), AbiParam::new(I64)];
    ctx.func.signature.returns = vec![AbiParam::new(I32)];
    let mut f = FunctionBuilder::new(&mut ctx.func, fbx);
    let entry_block = f.create_block();

    f.append_block_params_for_function_params(entry_block);
    f.switch_to_block(entry_block);
    f.seal_block(entry_block);

    let i = f.declare_var(types::I32);
    let a = f.block_params(entry_block)[0];
    f.def_var(i, a);

    {
        let mut chain = IfElseChainBuilder::new_block(
            &mut f,
            |f| f.ins().icmp_imm(condcodes::IntCC::Equal, a, 1),
            |f| {
                let val = f.ins().iconst(types::I32, 11);
                f.def_var(i, val);

                None
            },
        );

        chain.elseif_block(
            &mut f,
            |f| f.ins().icmp_imm(condcodes::IntCC::Equal, a, 2),
            |f| {
                let val = f.ins().iconst(types::I32, 22);
                f.def_var(i, val);
                None
            },
        );
        chain.elseif_block(
            &mut f,
            |f| f.ins().icmp_imm(condcodes::IntCC::Equal, a, 3),
            |f| {
                let val = f.ins().iconst(types::I32, 33);
                f.def_var(i, val);
                None
            },
        );

        chain.else_block(&mut f, |f| {
            let val = f.ins().iconst(types::I32, 69);
            f.def_var(i, val);
            None
        });

        chain.end(&mut f);
    }

    let ret = f.use_var(i);
    f.ins().return_(&[ret]);
    f.finalize();
}

pub struct Loop {
    end_block: Block,
    loop_block: Block,
}

impl Loop {
    pub fn new(fbx: &mut FunctionBuilder) -> Self {
        let loop_block = fbx.create_block();
        let end_block = fbx.create_block();
        fbx.ins().jump(loop_block, &[]);

        fbx.switch_to_block(loop_block);

        Self {
            end_block,
            loop_block,
        }
    }

    pub fn break_inst(&self, fbx: &mut FunctionBuilder) -> Inst {
        fbx.ins().jump(self.end_block, &[])
    }

    pub fn continue_inst(&self, fbx: &mut FunctionBuilder) -> Inst {
        fbx.ins().jump(self.loop_block, &[])
    }

    pub fn end(&self, fbx: &mut FunctionBuilder) {
        fbx.ins().jump(self.loop_block, &[]);
        fbx.seal_block(self.loop_block);

        fbx.switch_to_block(self.end_block);
        fbx.seal_block(self.end_block);
    }
}

fn build_loop_fn(ctx: &mut Context, fbx: &mut FunctionBuilderContext) {
    ctx.func.signature.params = vec![AbiParam::new(I32), AbiParam::new(I64)];
    ctx.func.signature.returns = vec![AbiParam::new(I32)];
    let mut f = FunctionBuilder::new(&mut ctx.func, fbx);
    let entry_block = f.create_block();

    f.append_block_params_for_function_params(entry_block);
    f.switch_to_block(entry_block);
    f.seal_block(entry_block);

    let i = f.declare_var(types::I32);
    let a = f.block_params(entry_block)[0];
    let zero = f.ins().iconst(types::I32, 0);
    f.def_var(i, zero);

    {
        let loopp = Loop::new(&mut f);

        let mut chain = IfElseChainBuilder::new_block(
            &mut f,
            |f| {
                let ivalue = f.use_var(i);
                f.ins().icmp(condcodes::IntCC::SignedLessThan, a, ivalue)
            },
            |f| {
                let ivalue = f.use_var(i);
                let val = f.ins().iadd_imm(ivalue, 1);
                f.def_var(i, val);
                None
            },
        );

        chain.elseif_block(
            &mut f,
            |f| {
                let ivalue = f.use_var(i);
                f.ins().icmp_imm(condcodes::IntCC::NotEqual, ivalue, 6)
            },
            |f| {
                let val = f.ins().iconst(I32, 66);
                Some(f.ins().return_(&[val]))
            },
        );

        chain.else_block(&mut f, |f| Some(loopp.break_inst(f)));

        chain.end(&mut f);

        let ival = f.use_var(i);
        let val = f.ins().iadd_imm(ival, 2);
        f.def_var(i, val);

        loopp.end(&mut f);
    }

    let ret = f.use_var(i);
    f.ins().return_(&[ret]);
    f.finalize();
}

// fn create_foo_fn(ctx: &mut Context, fbx: &mut FunctionBuilderContext) {
//     ctx.func.signature.params = vec![AbiParam::new(I32)];
//     ctx.func.signature.returns = vec![AbiParam::new(I64)];
//     let mut f = FunctionBuilder::new(&mut ctx.func, fbx);
//     let entry_block = f.create_block();

//     f.append_block_params_for_function_params(entry_block);
//     f.switch_to_block(entry_block);
//     f.seal_block(entry_block);

//     let b = f.block_params(entry_block)[0];
//     let sixty = f.ins().iconst(types::I32, 60);
//     let zero = f.ins().iconst(types::I64, 0);

//     let stack_slot = f.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 32));

//     f.ins().stack_store(sixty, stack_slot, 0);
//     f.ins().stack_store(b, stack_slot, 4);
//     f.ins().stack_store(sixty, stack_slot, 8);
//     f.ins().stack_store(zero, stack_slot, 16);
//     f.ins().stack_store(zero, stack_slot, 24);

//     let ret = f.ins().stack_addr(I64, stack_slot, 0);

//     f.ins().return_(&[ret]);
//     f.finalize();
// }
fn create_foo_fn(module: &mut ObjectModule, ctx: &mut Context, fbx: &mut FunctionBuilderContext) {
    ctx.func.signature.params = vec![AbiParam::new(I64), AbiParam::new(I32)];
    ctx.func.signature.returns = vec![];

    let mut f = FunctionBuilder::new(&mut ctx.func, fbx);
    let entry_block = f.create_block();

    f.append_block_params_for_function_params(entry_block);
    f.switch_to_block(entry_block);
    f.seal_block(entry_block);

    let stack_pointer = f.block_params(entry_block)[0];

    let b = f.block_params(entry_block)[1];
    let sixty = f.ins().iconst(types::I32, 60);
    let zero = f.ins().iconst(types::I64, 0);

    let size_of_foo = 32;

    f.ins().store(MemFlags::new(), sixty, stack_pointer, 0);
    f.ins().store(MemFlags::new(), b, stack_pointer, 4);
    f.ins().store(MemFlags::new(), sixty, stack_pointer, 8);
    f.ins().store(MemFlags::new(), zero, stack_pointer, 16);
    f.ins().store(MemFlags::new(), zero, stack_pointer, 24);

    let mut signature = Signature::new(CallConv::SystemV);
    signature.params.push(AbiParam::new(I64));
    let func_id = module
        .declare_function("print_foo", Linkage::Import, &signature)
        .unwrap();
    let func_ref = module.declare_func_in_func(func_id, f.func);
    f.ins().call(func_ref, &[stack_pointer]);

    f.ins().return_(&[]);
    f.finalize();
}
fn create_small_struct_fn(ctx: &mut Context, fbx: &mut FunctionBuilderContext) {
    ctx.func.signature.params = vec![AbiParam::new(I64)];
    ctx.func.signature.returns = vec![AbiParam::new(I64), AbiParam::new(I64)];

    let mut f = FunctionBuilder::new(&mut ctx.func, fbx);
    let entry_block = f.create_block();

    f.append_block_params_for_function_params(entry_block);
    f.switch_to_block(entry_block);
    f.seal_block(entry_block);

    let stack_pointer = f.block_params(entry_block)[0];

    // let b = f.block_params(entry_block)[1];
    let value = f.ins().iconst(types::I64, 69);
    let a = f.ins().iconst(types::I64, 68);
    let a_shifted = f.ins().rotl_imm(a, 8);
    let value_a_combined = f.ins().iadd(value, a_shifted);

    let b = f.ins().iconst(types::I64, 70);
    // let zero = f.ins().iconst(types::I64, 0);
    // let size_of_foo = 4;

    // f.ins().store(MemFlags::new(), sixty, stack_pointer, 0);

    f.ins().return_(&[value_a_combined, b]);
    f.finalize();
}

fn sum_from_foo_ptr_fn(ctx: &mut Context, fbx: &mut FunctionBuilderContext) {
    ctx.func.signature.params = vec![AbiParam::new(I64)];
    ctx.func.signature.returns = vec![AbiParam::new(I64)];
    let mut f = FunctionBuilder::new(&mut ctx.func, fbx);
    let entry_block = f.create_block();

    f.append_block_params_for_function_params(entry_block);
    f.switch_to_block(entry_block);
    f.seal_block(entry_block);
    let foo_ptr = f.block_params(entry_block)[0];
    let foo_a = f.ins().load(I32, MemFlags::new(), foo_ptr, 0);
    let foo_b = f.ins().load(I32, MemFlags::new(), foo_ptr, 4);
    let foo_c = f.ins().load(I32, MemFlags::new(), foo_ptr, 8);
    let foo_d = f.ins().load(I64, MemFlags::new(), foo_ptr, 16);
    let foo_e = f.ins().load(I64, MemFlags::new(), foo_ptr, 24);

    let sum = f.ins().iadd(foo_a, foo_b);
    let sum = f.ins().iadd(foo_c, sum);
    let sum = f.ins().sextend(I64, sum);
    let sum = f.ins().iadd(foo_d, sum);
    let sum = f.ins().iadd(foo_e, sum);

    let ret = sum;
    f.ins().return_(&[ret]);
    f.finalize();
}

fn sum_the_foo_fn(ctx: &mut Context, fbx: &mut FunctionBuilderContext) {
    ctx.func.signature.params = vec![];
    ctx.func.signature.returns = vec![AbiParam::new(I64)];
    let mut f = FunctionBuilder::new(&mut ctx.func, fbx);
    let entry_block = f.create_block();

    f.append_block_params_for_function_params(entry_block);
    f.switch_to_block(entry_block);
    f.seal_block(entry_block);

    let rbp = f.ins().get_frame_pointer(I64);
    let foo_ptr = f.ins().iadd_imm(rbp, 0x10);

    // let arg0 = f.block_params(entry_block)[0];
    // let arg1 = f.block_params(entry_block)[1];
    let ptr_deref = f.ins().load(I64, MemFlags::new(), foo_ptr, 0);
    // let ret = f.ins().load(I64, MemFlags::new(), ptr_deref, 0);
    let ret = ptr_deref;
    f.ins().return_(&[ret]);
    f.finalize();
}

fn anonymous_fn_builder(
    module: &mut ObjectModule,
    ctx: &mut Context,
    fbx: &mut FunctionBuilderContext,
) {
    let func_id = {
        ctx.func.signature.params = vec![AbiParam::new(I64)];
        ctx.func.signature.returns = vec![AbiParam::new(I64)];
        let mut f = FunctionBuilder::new(&mut ctx.func, fbx);
        let entry_block = f.create_block();

        f.append_block_params_for_function_params(entry_block);
        f.switch_to_block(entry_block);
        f.seal_block(entry_block);

        let val = f.block_params(entry_block)[0];
        let val = f.ins().imul_imm(val, 8);

        f.ins().return_(&[val]);
        let fn_id = module
            .declare_anonymous_function(&f.func.signature)
            .unwrap();
        f.finalize();
        // let signature = Signature::new(CallConv::SystemV);

        module.define_function(fn_id, ctx);
        module.clear_context(ctx);
        fn_id
    };
    {
        ctx.func.signature.params = vec![AbiParam::new(I64)];
        ctx.func.signature.returns = vec![AbiParam::new(I64)];
        let mut f = FunctionBuilder::new(&mut ctx.func, fbx);
        let entry_block = f.create_block();

        f.append_block_params_for_function_params(entry_block);
        f.switch_to_block(entry_block);
        f.seal_block(entry_block);

        let func_ref = module.declare_func_in_func(func_id, f.func);

        let val = f.block_params(entry_block)[0];
        let inst = f.ins().call(func_ref, &[val]);
        let val = f.inst_results(inst)[0];
        f.ins().return_(&[val]);
        f.finalize();
    }
}
fn recursive_fn_builder(
    module: &mut ObjectModule,
    ctx: &mut Context,
    fbx: &mut FunctionBuilderContext,
) {
    ctx.func.signature.params = vec![AbiParam::new(I64)];
    ctx.func.signature.returns = vec![AbiParam::new(I64)];

    let func_id = module
        .declare_function("fibonacci", Linkage::Export, &ctx.func.signature)
        .unwrap();

    let mut f = FunctionBuilder::new(&mut ctx.func, fbx);
    let entry_block = f.create_block();

    f.append_block_params_for_function_params(entry_block);
    f.switch_to_block(entry_block);

    let n = f.block_params(entry_block)[0];
    let is_n_zero = f
        .ins()
        .icmp_imm(condcodes::IntCC::SignedLessThanOrEqual, n, 1);
    let then_block = f.create_block();
    let else_block = f.create_block();
    f.ins().brif(is_n_zero, then_block, &[], else_block, &[]);
    f.switch_to_block(then_block);
    f.ins().return_(&[n]);
    f.seal_block(then_block);

    f.switch_to_block(else_block);
    let n_minus_one = f.ins().iadd_imm(n, -1);
    let n_minus_two = f.ins().iadd_imm(n, -2);
    let func_ref = module.declare_func_in_func(func_id, f.func);
    let fibn_minus_one = f.ins().call(func_ref, &[n_minus_one]);
    let fibn_minus_one = f.inst_results(fibn_minus_one)[0];
    let fibn_minus_two = f.ins().call(func_ref, &[n_minus_two]);
    let fibn_minus_two = f.inst_results(fibn_minus_two)[0];
    let val = f.ins().iadd(fibn_minus_two, fibn_minus_one);
    f.ins().return_(&[val]);
    f.seal_block(else_block);
    f.seal_block(entry_block);
    f.finalize();

    module.define_function(func_id, ctx);
    module.clear_context(ctx);
}

fn build_main_fn(module: &mut ObjectModule, ctx: &mut Context, fbx: &mut FunctionBuilderContext) {
    ctx.func.signature.params = vec![AbiParam::new(I32), AbiParam::new(I64)];
    ctx.func.signature.returns = vec![AbiParam::new(I32)];
    let mut f = FunctionBuilder::new(&mut ctx.func, fbx);
    let entry_block = f.create_block();

    f.append_block_params_for_function_params(entry_block);
    f.switch_to_block(entry_block);
    f.seal_block(entry_block);

    let argv = f.block_params(entry_block)[0];

    let FuncOrDataId::Func(func_id) = module.get_name("create_foo").unwrap() else {
        unreachable!();
    };

    let func_ref = module.declare_func_in_func(func_id, f.func);

    let inst = f.ins().call(func_ref, &[argv]);

    let foo_ptr = f.inst_results(inst)[0];

    let FuncOrDataId::Func(func_id) = module.get_name("sum_foo").unwrap() else {
        unreachable!();
    };

    let func_ref = module.declare_func_in_func(func_id, f.func);

    let inst = f.ins().call(func_ref, &[foo_ptr]);
    let sum = f.inst_results(inst)[0];

    // let b = f.ins().load(I32, MemFlags::new(), foo_ptr, 4); // foo.b
    let b = f.ins().ireduce(I32, sum);
    f.ins().return_(&[b]);
    f.finalize();
}
