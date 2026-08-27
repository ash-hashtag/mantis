use std.memory;
use std.array;

pub type Vec[T] = struct {
    buf Box[Array[T]],
    size u64,
    cap u64
}

impl[T] Vec[T] {
    pub fn with_capacity(cap u64) Vec[T] {
        let arr = Array.with_capacity[T](cap);
        let b = Box.new(arr);
        return Vec[T] {
            buf: b,
            size: 0,
            cap: cap
        };
    }

    pub fn new() Vec[T] {
        return Vec.with_capacity[T](0);
    }

    pub fn len(self @Vec[T]) u64 {
        return self.size;
    }

    pub fn capacity(self @Vec[T]) u64 {
        return self.cap;
    }

    pub fn is_empty(self @Vec[T]) bool {
        return self.size == 0;
    }

    pub fn as_ptr(self @Vec[T]) @T {
        return self.buf.get().ptr;
    }

    pub fn as_array(self @Vec[T]) Array[T] {
        return self.buf.get().slice(0, self.size);
    }

    pub fn as_slice(self @Vec[T]) Array[T] {
        return self.buf.get().slice(0, self.size);
    }

    pub fn reserve(self @Vec[T], additional u64) {
        let needed = self.size + additional;
        if needed > self.cap {
            let mut new_cap = 4 as u64;
            loop {
                if new_cap >= needed {
                    break;
                }
                new_cap = new_cap * 2;
            }
            let new_arr = Array.with_capacity[T](new_cap);
            if self.size > 0 {
                let old_ptr = self.buf.get().ptr;
                GlobalAllocator.copy_bytes(new_arr.ptr as i64, old_ptr as i64, self.size * #size_of(T));
                if self.cap > 0 {
                    GlobalAllocator.dealloc(old_ptr as i64);
                }
            }
            let new_box = Box.new(new_arr);
            self.buf = new_box;
            self.cap = new_cap;
        }
    }

    pub fn get(self @Vec[T], index u64) T {
        return self.buf.get().get(index);
    }

    pub fn set(self @Vec[T], index u64, el T) {
        self.buf.get().set(index, el);
    }

    pub fn push(self @Vec[T], el T) {
        self.reserve(1);
        self.buf.get().set(self.size, el);
        self.size = self.size + 1;
    }

    pub fn pop(self @Vec[T]) T {
        if self.size > 0 {
            self.size = self.size - 1;
            return self.buf.get().get(self.size);
        }
        return self.buf.get().get(0);
    }

    pub fn clear(self @Vec[T]) {
        self.size = 0;
    }

    pub fn free(self @Vec[T]) {
        if self.cap > 0 {
            let p = self.buf.get().ptr;
            GlobalAllocator.dealloc(p as i64);
            self.size = 0;
            self.cap = 0;
        }
    }
}

impl[T] AsRef[Array[T]] for Vec[T] {
    pub fn as_ref(self @Vec[T]) Array[T] {
        return self.buf.get().slice(0, self.size);
    }
}

impl[T] Deref[Array[T]] for Vec[T] {
    pub fn deref(self @Vec[T]) Array[T] {
        return self.buf.get().slice(0, self.size);
    }
}

impl[T] AsRef[T] for Vec[T] {
    pub fn as_ref(self @Vec[T]) @T {
        return self.buf.get().ptr;
    }
}

impl[T] Deref[T] for Vec[T] {
    pub fn deref(self @Vec[T]) @T {
        return self.buf.get().ptr;
    }
}
