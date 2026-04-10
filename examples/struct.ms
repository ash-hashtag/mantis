
type Foo = struct {
	a i64,
	b i64,
	c f64
}

impl Foo {
	fn new(a i64, b i64, c f64) Foo {
		let f = #init(Foo);
		f.a = a;
		f.b = b;
		f.c = c;
		return f;
	}

	fn sum(self Foo) i64 {
		return self.a + self.b;
	}

	fn set_a(self Foo, val i64) {
		self.a = val;
	}

	fn get_c(self Foo) f64 {
		return self.c;
	}
}

fn main(argv i32, argc i64) i32 {

	let f = Foo.new(20, 40, 4.2);
	f.set_a(100);
	let o = f.sum(); // should be 100 + 40 = 140

	if o == 140 {
		return 0;
	}

	return 1;
}
