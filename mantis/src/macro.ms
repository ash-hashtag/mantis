
/*

#macro anything starts with # is a compile time function, most of the time is a macro
$CompType anything starting with $ is a compile time only type, these types can't be used in runtime, they can only be used inside macros

*/



fn #add_two(a $I64, b $Any) $I64 {
	match #type(b) {
		i32 | i64 => {
			return a + b;
		},
		f32 | f64 => {
			return a + b.floor().to_int();
		}
	}


	return a + b;
}


fn #vec[T](args $Args[U]) Vec[T] where
 U: Any + Into[T] {
	let v = Vec.with_capacity(args.len());

	let idx = 0;
	loop {
		v.push(args.at(idx).into());
		idx = idx + 1;
		if idx == args.len() {
			break;
		}
	}

	return v;
}


fn main() {
	let v = #vec[u32](0, 1, 2.0);

	let vs = #vec[String]("hello ", s, " world");
}
