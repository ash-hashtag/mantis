
fn main(argv i32, argc i64) i32 {
	mut a = 10;

	let c = argv as i64;

	if c == 1 {
		a = 1;
	} else {
		if c == 2 {
			a = 2;
		} else {
			a = 4;
		}
	}

	return a as i32;
}
