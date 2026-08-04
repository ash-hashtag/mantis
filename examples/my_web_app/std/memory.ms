fn malloc(size i64) i64 extern ;
fn free(ptr i64) extern ;
fn memcpy(dest i64, src i64, size i64) i64 extern ;
fn memcmp(s1 i64, s2 i64, n i64) i32 extern ;

fn alloc[T](count u64) @T {
    return malloc(#size_of(T) * count) as @T;
}

fn dealloc[T](ptr @T) {
    free(ptr as i64);
}

fn copy_bytes[T](dest @T, src @T, count u64) {
    memcpy(dest as i64, src as i64, count * #size_of(T));
}
