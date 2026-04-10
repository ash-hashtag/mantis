fn puts(ptr i64) i32 extern;

fn main(argv i32, argc i64) i32 {
	mut a = 10;

	let c = argv as i64;

	if c == 1 {
		puts("c is 1");
		a = a + 1;
	} elif c == 2 {
		puts("c is 2");
		a = a + 2;
	} elif c == 3 {
		puts("c is 3");
	} else {
		puts("c is not 1 or 2");
		a = a + 4;
	}

	return a as i32;
}
