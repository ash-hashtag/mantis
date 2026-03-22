
type Foo = struct {
	a i64,
	b i64,
	c f64
}


fn main(argv i32, argc i64) i32 {

	let f = #init(Foo);
	f.a = 20;
	f.b = 40;
	f.c = 4.2;

	// let f = Foo {
	// 	a 20,
	// 	b 30,
	// 	c 4.2,
	// };

	let o = f.a;

	return o as i32;
}
