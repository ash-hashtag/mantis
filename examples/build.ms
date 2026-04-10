import "compiler.ms" as *;
import "borrow-checker.ms" as borrowChecker;


fn main() {
	let executableName = "example";
	let buildDir = std.env.getVar("TARGET_DIR").unwrap_or("./build");
	let outputObjFile = #format("${buildDir}/${example}.o");
	let outputExeFile = #format("${buildDir}/${example}");

	
	mut compiler = Compiler.default();

	compiler.addPlugin(borrowChecker.BorrowChecker.new());
	
	let obj = compiler.compileExecutable("src/main.ms");
	std.fs.write(outputObjFile, obj);
	
	let result = std.process.Child.spawn(#format("cc -o ${outputExeFile} ${outputObjFile}")).wait().unwrap();

	#assert(result, 0);

	// run it if needed
	std.process.Child.spawn(outputExeFile);
}
