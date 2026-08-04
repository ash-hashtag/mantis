fn malloc(size i64) i64 extern ;
fn free(ptr i64) extern ;
fn memcpy(dest i64, src i64, size i64) i64 extern ;

type Vec[T] = struct {
    ptr @T,
    size u64,
    cap u64
}

impl[T] Vec[T] {
    fn with_capacity(cap u64) @Vec[T] {
        let v @Vec[T] = malloc(#size_of(Vec[T])) as @Vec[T];
        if cap > 0 {
            v.ptr = malloc(#size_of(T) * cap) as @T;
        } else {
            v.ptr = 0 as @T;
        }
        v.size = 0;
        v.cap = cap;
        return v;
    }

    fn new() @Vec[T] {
        return Vec.with_capacity[T](0);
    }

    fn len(self @Vec[T]) u64 {
        return self.size;
    }

    fn capacity(self @Vec[T]) u64 {
        return self.cap;
    }

    fn is_empty(self @Vec[T]) bool {
        return self.size == 0;
    }

    fn get(self @Vec[T], index u64) T {
        return @(self.ptr + index * #size_of(T));
    }

    fn set(self @Vec[T], index u64, el T) {
        (self.ptr + index * #size_of(T)) @= el;
    }

    fn push(self @Vec[T], el T) {
        if self.size >= self.cap {
            let mut new_cap = 4 as u64;
            if self.cap > 0 {
                new_cap = self.cap * 2;
            }
            let new_ptr = malloc(#size_of(T) * new_cap) as @T;
            if self.size > 0 {
                memcpy(new_ptr as i64, self.ptr as i64, self.size * #size_of(T));
                free(self.ptr as i64);
            }
            self.ptr = new_ptr;
            self.cap = new_cap;
        }
        (self.ptr + self.size * #size_of(T)) @= el;
        self.size = self.size + 1;
    }

    fn pop(self @Vec[T]) T {
        if self.size > 0 {
            self.size = self.size - 1;
            return @(self.ptr + self.size * #size_of(T));
        }
        return @(self.ptr);
    }

    fn clear(self @Vec[T]) {
        self.size = 0;
    }

    fn free(self @Vec[T]) {
        if self.cap > 0 {
            free(self.ptr as i64);
        }
        free(self as i64);
    }
}
