

fn main() {

	let src = "hello world";
	mut iter = src.split(' '); 

	loop {
		let next = iter.next();
		if next.is_some() {
			let next = next.unwrap();
			match next {
				"hello" : {
					print("Hello");
				},
				"world": {
					print("World");
				},
				_ : {}
			}

			
		} else {
			break;
		}

	}

	

}
