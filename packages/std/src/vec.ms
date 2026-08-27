use libc;

pub type Vec[T] = struct {
    ptr @T,
    size u64,
    cap u64
};

impl[T] Vec[T] {
    pub fn new() Vec[T] {
        return Vec {
            ptr: 0 as @T,
            size: 0,
            cap: 0
        };
    }

    pub fn with_capacity(cap u64) Vec[T] {
        if cap == 0 {
            return Vec {
                ptr: 0 as @T,
                size: 0,
                cap: 0
            };
        }
        let elem_size = #size_of(T);
        let ptr = libc.malloc((cap as i64) * (elem_size as i64));
        return Vec {
            ptr: ptr as @T,
            size: 0,
            cap: cap
        };
    }

    pub fn len(self) u64 {
        return self.size;
    }

    pub fn is_empty(self) bool {
        return self.size == 0;
    }

    pub fn as_ptr(self) @T {
        return self.ptr;
    }

    pub fn reserve(mut self, additional u64) {
        let needed = self.size + additional;
        if needed <= self.cap {
            return;
        }
        let mut new_cap = self.cap * 2;
        if new_cap < needed {
            new_cap = needed;
        }
        if new_cap < 8 {
            new_cap = 8;
        }
        let elem_size = #size_of(T);
        let new_size = (new_cap as i64) * (elem_size as i64);
        let mut new_ptr = 0 as @T;
        if (self.ptr as i64) == 0 {
            new_ptr = libc.malloc(new_size) as @T;
        } else {
            new_ptr = libc.realloc(self.ptr as i64, new_size) as @T;
        }
        self.ptr = new_ptr;
        self.cap = new_cap;
    }

    pub fn get(self, index u64) T {
        let elem_size = #size_of(T);
        let elem_ptr = (self.ptr as i64) + ((index as i64) * (elem_size as i64));
        let mut res = 0 as T;
        libc.memcpy((&res) as @i64, elem_ptr as @i64, elem_size as i64);
        return res;
    }

    pub fn set(mut self, index u64, val T) bool {
        if index >= self.size {
            return false;
        }
        let elem_size = #size_of(T);
        let elem_ptr = (self.ptr as i64) + ((index as i64) * (elem_size as i64));
        libc.memcpy(elem_ptr as @i64, (&val) as @i64, elem_size as i64);
        return true;
    }

    pub fn push(mut self, val T) {
        self.reserve(1);
        let elem_size = #size_of(T);
        let elem_ptr = (self.ptr as i64) + ((self.size as i64) * (elem_size as i64));
        libc.memcpy(elem_ptr as @i64, (&val) as @i64, elem_size as i64);
        self.size = self.size + 1;
    }

    pub fn clear(mut self) {
        self.size = 0;
    }

    pub fn free(mut self) {
        if (self.ptr as i64) != 0 {
            libc.free(self.ptr as i64);
            self.ptr = 0 as @T;
            self.size = 0;
            self.cap = 0;
        }
    }
}
