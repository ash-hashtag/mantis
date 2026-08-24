use std.memory;

type Vec[T] = struct {
    ptr @T,
    size u64,
    cap u64
}

impl[T] Vec[T] {
    fn with_capacity(cap u64) Vec[T] {
        let mut ptr @T = 0 as @T;
        if cap > 0 {
            ptr = GlobalAllocator.alloc(cap, #size_of(T)) as @T;
        }
        return Vec[T] { ptr: ptr, size: 0, cap: cap };
    }

    fn new() Vec[T] {
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
            let new_ptr = GlobalAllocator.alloc(new_cap, #size_of(T)) as @T;
            if self.size > 0 {
                GlobalAllocator.copy_bytes(new_ptr as i64, self.ptr as i64, self.size * #size_of(T));
                GlobalAllocator.dealloc(self.ptr as i64);
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
            GlobalAllocator.dealloc(self.ptr as i64);
        }
    }
}
