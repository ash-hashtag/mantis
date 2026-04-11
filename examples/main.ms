
// fn fwrite(ptr i64, size i64, nmemb i64, stream i64) i64 extern;
fn malloc(size i64) i64 extern;
fn free(ptr i64) extern;
// fn fopen(file_path_ptr i64, mode_ptr i64) i64 extern;
// fn fread(buffer_ptr i64, size i64, count i64, file_ptr i64) i64 extern;
// fn puts(ptr i64) i32 extern;
// fn fclose(file_ptr i64) i32 extern;
// fn write(fd i64, data_ptr i64, count i64) i64 extern;
// fn rand() i32 extern;

type Foo = struct {
	a i32,
	b i64,
	c f32
}



fn twenty() i64 {
	return 20;
}

fn add(a i64, b i64) i64 {
	return a + b + twenty();
} 


fn main(argc i32, argv i64) i32 {
	mut b: i64 = 7;
	mut i: i64 = argc as i64;

	let foo: @Foo = #init(Foo); // ptr
	
	foo.a = 10 as i32; // (foo + @offset(Foo.a)) = 10
	foo.b = 20; // (foo + @offset(Foo.b)) = 20
	foo.c = 2.0 as f32; // (foo + @offset(Foo.c)) = 2.0

	loop {
		b = i;
		if i > 10 {
			break;
		} else {
			i = i + 1;
			continue;
		}
	}

	b = add(i, b);
	
	#free(foo);

	return b as i32; 
}
