
fn puts(ptr i64) i32 extern;

fn main(argc i32, argv i64) i32 {
	let i = 0;
	loop {
		puts("Hellow! Epic Loops");
		i = i + 1;
		if i > 10 {
			break;
		} else {
			continue;
		}
	}

	return i as i32;
}
