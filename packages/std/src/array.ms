use std.memory;

pub type Array[T] = struct {
    ptr @T,
    len u64
}

impl[T] Array[T] {
    pub fn new(ptr @T, len u64) Array[T] {
        return Array[T] {
            ptr: ptr,
            len: len
        };
    }

    pub fn with_capacity(cap u64) Array[T] {
        let mut ptr @T = 0 as @T;
        if cap > 0 {
            ptr = GlobalAllocator.alloc(cap, #size_of(T)) as @T;
        }
        return Array[T] {
            ptr: ptr,
            len: cap
        };
    }

    pub fn len(self @Array[T]) u64 {
        return self.len;
    }

    pub fn is_empty(self @Array[T]) bool {
        return self.len == 0;
    }

    pub fn as_ptr(self @Array[T]) @T {
        return self.ptr;
    }

    pub fn get(self @Array[T], index u64) T {
        return @(self.ptr + index * #size_of(T));
    }

    pub fn set(self @Array[T], index u64, val T) {
        (self.ptr + index * #size_of(T)) @= val;
    }

    pub fn at(self @Array[T], index u64) @T {
        return self.ptr + index * #size_of(T);
    }

    pub fn slice(self @Array[T], start u64, end u64) Array[T] {
        let mut actual_end = end;
        if actual_end > self.len {
            actual_end = self.len;
        }
        let mut actual_start = start;
        if actual_start > actual_end {
            actual_start = actual_end;
        }
        let new_ptr = self.ptr + actual_start * #size_of(T);
        return Array[T] {
            ptr: new_ptr,
            len: actual_end - actual_start
        };
    }
}

impl[T] AsRef[T] for Array[T] {
    pub fn as_ref(self @Array[T]) @T {
        return self.ptr;
    }
}

impl[T] Deref[T] for Array[T] {
    pub fn deref(self @Array[T]) @T {
        return self.ptr;
    }
}
