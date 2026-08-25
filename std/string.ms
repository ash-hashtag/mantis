use std.vec;
use std.memory;

// libc externs (malloc/free/memcpy/memset) are provided by std.memory.

type String = struct {
    vec Vec[u8]
}

impl String {
    fn with_capacity(cap u64) String {
        return String { vec: Vec.with_capacity[u8](cap) };
    }

    fn new() String {
        return String.with_capacity(0);
    }

    fn push_byte(self @String, b u8) {
        self.vec.push(b);
    }

    fn push_str(self @String, s StrSlice) {
        if s.len > 0 {
            self.vec.reserve(s.len as u64);
            GlobalAllocator.copy_bytes(
                self.vec.ptr as i64 + self.vec.len(),
                s.pointer,
                s.len
            );
            self.vec.size = self.vec.size + s.len;
        }
    }

    fn from_str(s StrSlice) String {
        let str = String.with_capacity(s.len as u64);
        str.push_str(s);
        return str;
    }

    fn as_str(self @String) StrSlice {
        let slice = StrSlice {
            pointer: self.vec.ptr as i64,
            len: self.vec.len() as i64
        };
        return slice;
    }

    fn len(self @String) u64 {
        return self.vec.len();
    }

    fn clear(self @String) {
        self.vec.clear();
    }

    /// Release early. Optional — RAII frees automatically at scope exit.
    fn free(self @String) {
        self.vec.free();
    }
}

/// RAII: release the backing buffer when the String goes out of scope.
impl Drop for String {
    fn drop(self @mut String) {
        if self.vec.cap > 0 {
            GlobalAllocator.dealloc(self.vec.ptr as i64);
        }
    }
}
