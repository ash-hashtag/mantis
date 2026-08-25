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

    /// Grow the buffer so at least `additional` more elements fit.
    fn reserve(self @Vec[T], additional u64) {
        let needed = self.size + additional;
        if needed > self.cap {
            let mut new_cap = 4 as u64;
            loop {
                if new_cap >= needed {
                    break;
                }
                new_cap = new_cap * 2;
            }
            let new_ptr = GlobalAllocator.alloc(new_cap, #size_of(T)) as @T;
            if self.size > 0 {
                GlobalAllocator.copy_bytes(new_ptr as i64, self.ptr as i64, self.size * #size_of(T));
                GlobalAllocator.dealloc(self.ptr as i64);
            }
            self.ptr = new_ptr;
            self.cap = new_cap;
        }
    }

    fn get(self @Vec[T], index u64) T {
        return @(self.ptr + index * #size_of(T));
    }

    fn set(self @Vec[T], index u64, el T) {
        (self.ptr + index * #size_of(T)) @= el;
    }

    fn push(self @Vec[T], el T) {
        self.reserve(1);
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

    /// Release early. Optional — RAII frees automatically at scope exit.
    fn free(self @Vec[T]) {
        if self.cap > 0 {
            GlobalAllocator.dealloc(self.ptr as i64);
        }
    }
}

/// RAII: release the buffer when the Vec goes out of scope.
impl[T] Drop for Vec[T] {
    fn drop(self @mut Vec[T]) {
        if self.cap > 0 {
            GlobalAllocator.dealloc(self.ptr as i64);
        }
    }
}
