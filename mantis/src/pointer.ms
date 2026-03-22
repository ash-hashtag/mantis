fn malloc(size i64) i64 extern;
fn free(ptr i64) extern;
fn write(fd i64, data_ptr i64, count i64) i64 extern;
fn memcpy(dest i64, src i64, size i64) i64 extern;

type Array[T] = struct {
	ptr pointer[T],
	size u64
}

fn array_new[T](size u64) pointer[Array[T]] {
	let ptr pointer[T] = malloc(size * #size_of(T)) as pointer[T];
	let arr pointer[Array[T]] = malloc(#size_of(Array[T])) as pointer[Array[T]];
	arr.ptr = ptr;
	arr.size = size;
	return arr;
}

fn array_free[T](arr pointer[Array[T]]) {
	free(arr.ptr as i64);
	free(arr as i64);
}

fn array_get_at[T](arr pointer[Array[T]], index u64) {
	return #get(arr.ptr + index * #size_of(T));
}

fn array_set_at[T](arr pointer[Array[T]], index u64, value T) {
	#set(arr.ptr + index * #size_of(T), value);
}


type Vec[T] = struct {
	ptr pointer[T],
	cap u64,
	size u64
}

fn Vec_free[T](v pointer[Vec[T]]) {
	free(v.ptr as i64);
	free(v as i64);
}

fn Vec_with_capacity[T](cap u64)  pointer[Vec[T]] {
	let ptr pointer[T] =  malloc(#size_of(T) * cap);
	let arr pointer[Vec[T]] = malloc(#size_of(Vec[T])) as pointer[Vec[T]];
	arr.ptr = 0;
	arr.size = 0;
	arr.cap = cap;
	return arr;
}

fn Vec_new[T]() pointer[Vec[T]]  {
	return Vec_with_capacity[T](0);
}


fn Vec_get_at[T](v pointer[Vec[T]], index u64) {
	return #get(v.ptr + index * #size_of(T));
}

fn Vec_set_at[T](v pointer[Vec[T]], index u64, el T) {
	#set(v.ptr + index * #size_of(T), el);
}

fn Vec_push[T](v pointer[Vec[T]], el T) {
	if v.size + 1 >= v.cap {
		let new_size = (v.size + 1) * 2;
		let new_ptr = malloc(#size_of(T) * new_size);
		memcpy(new_ptr, v.ptr, v.size);
		free(v.ptr);
		v.ptr = new_ptr;
		v.cap = new_size;
	}
	Vec_set_at(v, v.size + 1, el);
}


fn Vec_pop[T](v pointer[Vec[T]]) T {
	if v.size > 0 {
		v.size = v.size - 1;
	}
	return Vec_get_at(v.size + 1);
}


fn Vec_truncate(v pointer[Vec[T]], size u64) {
	v.size = size;
}




fn main(argv i32, argc i64) i32 {
	let arr pointer[Array[u8]] = array_new[u8](11);

	array_set_at[u8](arr, 0, 104 as u8);
	array_set_at[u8](arr, 1, 101 as u8);
	array_set_at[u8](arr, 2, 108 as u8);
	array_set_at[u8](arr, 3, 108 as u8);
	array_set_at[u8](arr, 4, 111 as u8);

	write(1, arr.ptr, 5);

	array_free[u8](arr);

	return 0;
}


