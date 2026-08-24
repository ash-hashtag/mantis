use std.vec;
use std.memory;

fn malloc(size i64) i64 extern ;
fn free(ptr i64) extern ;
fn memcpy(dest i64, src i64, size i64) i64 extern ;

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
        let mut i = 0;
        loop {
            if i >= s.len {
                break;
            }
            let byte_ptr = (s.pointer + i) as @u8;
            self.vec.push(@byte_ptr);
            i = i + 1;
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

    fn free(self @String) {
        self.vec.free();
    }
}
