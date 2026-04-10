use cranelift::prelude::*;

fn main() {
    let mut ctx = cranelift::codegen::Context::new();
    let mut builder_context = FunctionBuilderContext::new();
    
    let mut builder = FunctionBuilder::new(&mut ctx.func, &mut builder_context);
    let entry_block = builder.create_block();
    builder.append_block_params_for_function_params(entry_block);
    builder.switch_to_block(entry_block);
    builder.seal_block(entry_block);
    
    let zero = builder.ins().iconst(types::I32, 0);
    builder.ins().return_(&[zero]);
    
    builder.finalize();
    
    println!("{}", ctx.func.display());
}
