// Mantis Standard Library
use libc;

fn malloc(size i64) i64 extern ;
fn free(ptr i64) extern ;
fn memcpy(dest i64, src i64, size i64) i64 extern ;
fn memcmp(s1 i64, s2 i64, n i64) i32 extern ;
fn write(fd i64, data_ptr i64, count i64) i64 extern ;

// ── Memory ───────────────────────────────────────────────────────────────────

fn alloc[T](count u64) @T {
    return malloc(#size_of(T) * count) as @T;
}

fn dealloc[T](ptr @T) {
    free(ptr as i64);
}

fn copy_bytes[T](dest @T, src @T, count u64) {
    memcpy(dest as i64, src as i64, count * #size_of(T));
}

// ── Option ───────────────────────────────────────────────────────────────────

type Option[T] = enum {
    Some(T),
    None
}

impl[T] Option[T] {
    fn is_some(self) bool {
        match self {
            Option.Some(_) : { return true; },
            Option.None : { return false; }
        }
    }

    fn is_none(self) bool {
        return !self.is_some();
    }

    fn unwrap(self) T {
        match self {
            Option.Some(val) : { return val; },
            Option.None : { panic("called Option.unwrap() on a None value"); }
        }
    }

    fn unwrap_or(self, default_val T) T {
        match self {
            Option.Some(val) : { return val; },
            Option.None : { return default_val; }
        }
    }
}

// ── Result ───────────────────────────────────────────────────────────────────

type Result[T, E] = enum {
    Ok(T),
    Err(E)
}

impl[T, E] Result[T, E] {
    fn is_ok(self) bool {
        match self {
            Result.Ok(_) : { return true; },
            Result.Err(_) : { return false; }
        }
    }

    fn is_err(self) bool {
        return !self.is_ok();
    }

    fn unwrap(self) T {
        match self {
            Result.Ok(val) : { return val; },
            Result.Err(_) : { panic("called Result.unwrap() on an Err value"); }
        }
    }

    fn unwrap_err(self) E {
        match self {
            Result.Ok(_) : { panic("called Result.unwrap_err() on an Ok value"); },
            Result.Err(err) : { return err; }
        }
    }
}

// ── Vec ──────────────────────────────────────────────────────────────────────

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

// ── String ───────────────────────────────────────────────────────────────────

type String = struct {
    vec @Vec[u8]
}

impl String {
    fn with_capacity(cap u64) @String {
        let s @String = malloc(#size_of(String)) as @String;
        s.vec = Vec.with_capacity[u8](cap);
        return s;
    }

    fn new() @String {
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

    fn from_str(s StrSlice) @String {
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

    fn clear(self @String) {
        self.vec.clear();
    }

    fn free(self @String) {
        self.vec.free();
        free(self as i64);
    }
}

// ── IO ───────────────────────────────────────────────────────────────────────

fn exit(code i32) extern ;

fn panic(msg StrSlice) {
    let prefix = "panic: ";
    write(2, prefix.pointer, prefix.len);
    write(2, msg.pointer, msg.len);
    let nl = "\n";
    write(2, nl.pointer, 1);
    exit(1);
}

fn print_str(s StrSlice) {
    write(1, s.pointer, s.len);
}

fn println_str(s StrSlice) {
    write(1, s.pointer, s.len);
    let nl = "\n";
    write(1, nl.pointer, 1);
}

fn print_int(n i64) {
    if n == 0 {
        let zero = "0";
        write(1, zero.pointer, 1);
        return;
    }
    let mut temp = n;
    if temp < 0 {
        let minus = "-";
        write(1, minus.pointer, 1);
        temp = -temp;
    }
    let mut buf = [0 as u8, 0 as u8, 0 as u8, 0 as u8, 0 as u8, 0 as u8, 0 as u8, 0 as u8, 0 as u8, 0 as u8, 0 as u8, 0 as u8, 0 as u8, 0 as u8, 0 as u8, 0 as u8, 0 as u8, 0 as u8, 0 as u8, 0 as u8];
    let mut count = 0;
    loop {
        if temp == 0 {
            break;
        }
        let rem = temp % 10;
        let digit = (48 + rem) as u8;
        let ptr = (&buf as @u8) + count;
        ptr @= digit;
        count = count + 1;
        temp = temp / 10;
    }
    loop {
        if count == 0 {
            break;
        }
        count = count - 1;
        let ptr = (&buf as @u8) + count;
        write(1, ptr as i64, 1);
    }
}
