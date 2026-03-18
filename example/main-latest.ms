
import libc;

import std.net.IpAddr;


fn foo1(a i64, b u64) i64 extern;
fn foo2(a i64, b u64) extern;

type Option[T] = enum {
    Some(T),
    None
}

type Result[U, V] = enum {
	Ok(U),
	Err(V)
}

type ptr[T] = i64;

type ArraySlice[T] = struct {
    pointer ptr[T],
    len u64
}

type Vec[T] = struct {
    capacity u64,
    slice ArraySlice[T]
}


fn main() {
	mut a = libc.malloc(1024);
	a = 10; // assigns value to variable 'a'
	a @= 10; // sets value at address 'a';

	let b = @a; // gets value from address 'a';
	let c = @(a + 1); // gets value from address 'a' + 1;

	d = IpAddr.new(127, 0, 0, 1);

}

fn build(s &ArraySlice[MantisSource]) {
	

}

fn buildConfig(c &mut mantis.BuildConfig) {
	c.allowAnySysCalls(true);
	c.allowAnyExternalFunctions(true);

	// or

	c.allowAnySysCalls(false);
	c.allowAnyExternalFunctions(false);


	c.allowSysCall(mantis.SysCallSignature { callId: 0, arguments: [mantis.types.i32, mantis.types.i32, mantis.types.i64]}); // only allow syscall with this matching signature


	c.allowExternalFunction(mantis.FunctionSignature {
		name: "malloc", returns: [mantis.types.i64], arguments: [mantis.types.i64]
	}); // only allow external functions with this matching signature


	// hook up your own function


	c.addFunction("free", fn (size i64) {
		let libc = #import("libc");
		libc.free(size);
	});


	c.compile(fn (compiler mantis.Compiler, source mantis.Source) mantis.CompiledObject {
		// modify anything if needed

		if source.filename == "main.ms" {
			let newConfig = mantis.BuildConfig.copyFrom(c);
			// update new config
			let newCompiler = mantis.Compiler.withConfig(newConfig);
			return newCompiler.compile(source);
			
		} else {
			return compiler.compile(source);
		}

	});
	
}
