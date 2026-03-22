type Foo = struct {
	a i32,
	b i64,
	c u32
}


fn sumFoo(foo Foo) i32 {
	return foo.a + foo.b as i32 + foo.c as i32;
}


fn main() {
	let a = (1 + 2) * 3 / 2;
	let b = a & 5;
	let c = (b | a == 5);

	let foo = Foo;
	foo.a = a as i32;
	foo.b = b as i64;
	foo.c = c as u32;

	return sumFoo(foo);
}
