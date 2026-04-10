type Foo = struct {
	a i32,
	b i64,
	c u32
}


fn sumFoo(foo Foo) i32 {
	return foo.a + foo.b as i32 + foo.c as i32;
}


fn main() i32 {
	let a = (1 + 2) * 3 / 2;
	let b = a & 5;
	let c = (b | (a == 5) as i64);

	let foo = Foo { a: 0, b: 0, c: 0 };
	foo.a = a as i32;
	foo.b = b as i64;
	foo.c = c as u32;

	return sumFoo(foo);
}
