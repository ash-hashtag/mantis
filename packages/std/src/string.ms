use libc;
use std.vec;

pub type Str = StrSlice;

pub type String = struct {
    buf Vec[u8]
};

impl String {
    pub fn new() String {
        let v = Vec {
            ptr: 0 as @u8,
            size: 0,
            cap: 0
        };
        return String { buf: v };
    }

    pub fn with_capacity(cap u64) String {
        if cap == 0 {
            return String.new();
        }
        let p = libc.malloc(cap as i64);
        let v = Vec {
            ptr: p as @u8,
            size: 0,
            cap: cap
        };
        return String { buf: v };
    }

    pub fn from_str(s Str) String {
        if s.len <= 0 {
            return String.new();
        }
        let p = libc.malloc(s.len);
        libc.memcpy(p as @i64, s.pointer as @i64, s.len);
        let v = Vec {
            ptr: p as @u8,
            size: s.len as u64,
            cap: s.len as u64
        };
        return String { buf: v };
    }

    pub fn len(self) u64 {
        return self.buf.size;
    }

    pub fn is_empty(self) bool {
        return self.buf.size == 0;
    }

    pub fn as_ptr(self) @u8 {
        return self.buf.ptr;
    }

    pub fn as_str(self) Str {
        return StrSlice {
            pointer: self.buf.ptr as i64,
            len: self.buf.size as i64
        };
    }

    pub fn push_byte(mut self, b u8) {
        if self.buf.size >= self.buf.cap {
            let mut new_cap = self.buf.cap * 2;
            if new_cap < 8 {
                new_cap = 8;
            }
            let mut new_ptr = 0 as @u8;
            if (self.buf.ptr as i64) == 0 {
                new_ptr = libc.malloc(new_cap as i64) as @u8;
            } else {
                new_ptr = libc.realloc(self.buf.ptr as i64, new_cap as i64) as @u8;
            }
            self.buf.ptr = new_ptr;
            self.buf.cap = new_cap;
        }
        libc.memset((self.buf.ptr as i64 + self.buf.size as i64) as @i64, b as i64, 1);
        self.buf.size = self.buf.size + 1;
    }

    pub fn push_str(mut self, s Str) {
        if s.len <= 0 {
            return;
        }
        let needed = self.buf.size + (s.len as u64);
        if needed > self.buf.cap {
            let mut new_cap = self.buf.cap * 2;
            if new_cap < needed {
                new_cap = needed;
            }
            if new_cap < 8 {
                new_cap = 8;
            }
            let mut new_ptr = 0 as @u8;
            if (self.buf.ptr as i64) == 0 {
                new_ptr = libc.malloc(new_cap as i64) as @u8;
            } else {
                new_ptr = libc.realloc(self.buf.ptr as i64, new_cap as i64) as @u8;
            }
            self.buf.ptr = new_ptr;
            self.buf.cap = new_cap;
        }
        libc.memcpy((self.buf.ptr as i64 + self.buf.size as i64) as @i64, s.pointer as @i64, s.len);
        self.buf.size = needed;
    }

    pub fn clear(mut self) {
        self.buf.size = 0;
    }

    pub fn free(mut self) {
        if (self.buf.ptr as i64) != 0 {
            libc.free(self.buf.ptr as i64);
            self.buf.ptr = 0 as @u8;
            self.buf.size = 0;
            self.buf.cap = 0;
        }
    }
}
