use std.vec;
use std.memory;
use std.array;

pub type Str = StrSlice;

pub type String = struct {
    vec Vec[u8]
}

impl String {
    pub fn with_capacity(cap u64) String {
        return String { vec: Vec.with_capacity[u8](cap) };
    }

    pub fn new() String {
        return String.with_capacity(0);
    }

    pub fn push_byte(self @String, b u8) {
        self.vec.push(b);
    }

    pub fn push_str(self @String, s Str) {
        if s.len > 0 {
            let s_len = s.len as u64;
            self.vec.reserve(s_len);
            let ptr = self.vec.as_ptr();
            GlobalAllocator.copy_bytes(
                ptr as i64 + self.vec.len(),
                s.pointer,
                s_len
            );
            self.vec.size = self.vec.size + s_len;
        }
    }

    pub fn from_str(s Str) String {
        let str = String.with_capacity(s.len as u64);
        str.push_str(s);
        return str;
    }

    pub fn as_str(self @String) Str {
        let arr = self.vec.as_array();
        let slice = StrSlice {
            pointer: arr.ptr as i64,
            len: arr.len as i64
        };
        return slice;
    }

    pub fn as_array(self @String) Array[u8] {
        return self.vec.as_array();
    }

    pub fn as_ptr(self @String) @u8 {
        return self.vec.as_ptr();
    }

    pub fn len(self @String) u64 {
        return self.vec.len();
    }

    pub fn is_empty(self @String) bool {
        return self.vec.len() == 0;
    }

    pub fn clear(self @String) {
        self.vec.clear();
    }

    /// Release early. Optional — RAII frees automatically at scope exit.
    pub fn free(self @String) {
        self.vec.free();
    }
}

impl AsRef[Str] for String {
    pub fn as_ref(self @String) Str {
        return self.as_str();
    }
}

impl Deref[Str] for String {
    pub fn deref(self @String) Str {
        return self.as_str();
    }
}

impl AsRef[Array[u8]] for String {
    pub fn as_ref(self @String) Array[u8] {
        return self.as_array();
    }
}

impl Deref[Array[u8]] for String {
    pub fn deref(self @String) Array[u8] {
        return self.as_array();
    }
}

impl AsRef[u8] for String {
    pub fn as_ref(self @String) @u8 {
        return self.vec.as_ptr();
    }
}
