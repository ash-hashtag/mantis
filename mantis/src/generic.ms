fn malloc(size i64) i64 extern;

type Array[T] = struct {
	ptr ptr[T],
	size u64
}


trait Into[T] {
	fn into(self Ref[Self]) T;
}

impl Into[f64] for u64 {
	fn into(self MutRef[Self]) f64  {
		self as f64	
	}
}


fn main(argc i32, argv i64) i32 {
	let size u64 = 20 as u64;
	let ptr i64 =  malloc(4 * size);

	let array = #init(Array[u64]);
	array.ptr = ptr;
	array.size = size;
	

	return 0;
}
