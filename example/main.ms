
fn fwrite(buffer_ptr i64, size i64, count i64, stream_ptr i64) i64 extern;
fn write(fd i64, buffer_ptr i64, len i64) i64 extern;
fn malloc(size i64) i64 extern;
fn free(size i64) extern;
fn memcpy(dest i64, src i64, count i64) i64 extern;
fn strlen(ptr i64) i64 extern;


type Option[T] = enum {
    Some(T),
    None
}

type ptr[T] = i64;

type ArraySlice[T] = struct {
    pointer ptr[T],
    len u64
}

type Vec[T] = struct {
    capacity u64,
    slice ArraySlice[T]
}


impl[T] Drop for Vec[T] {
    fn drop(self @mut Self) {
        free(self.slice.pointer);
    }
}

impl[T] Self for Vec[T] {
    fn with_capacity(cap u64) Self {
        if cap > 0 {
            return Self {
                slice: ArraySlice[T] {
                    pointer: malloc(cap),
                    len: 0
                },
                capacity: cap
            };
        } else {
            return Self {
                slice: [],
                capacity: cap
            };
        }
    }

    fn new() Self {
        return Self.with_capacity(0);
    }

    fn grow(self @mut Self, new_cap u64) {
        let new_vec = Self.with_capacity(new_cap);
        memcpy(new_vec.slice.pointer, self.slice.pointer, self.slice.len);
        new_vec.slice.len = self.slice.len;
        self = new_vec;
    }

    fn push(self @mut Self, element T) {
        *(self.slice.pointer + self.slice.len) = element;
    }

    fn truncate(self @mut Self, new_len u64) {
      self.slice.len = new_len;  
    }

    fn clear(self @mut Self) {
        self.truncate(0);
    }
    
    fn get_at(self @Self, idx u64) Option[@T] {
        if idx < self.slice.len {
            return Option.Some(self.slice.pointer + self.slice.len);
        } else {
            return Option.None;
        }
    }

    fn pop(self @mut Self) Option[T] {
        if self.slice.len != 0 {
            let result = Option.Some(self.slice.pointer + self.slice.len);
            self.slice.len = self.slice.len - 1;
            return result;
        } else {
            return Option.None;
        }
    }
    
}



type StrSlice = ArraySlice[u8];

fn print(s StrSlice) {
    write(1, s.pointer, s.len);
}

fn println(s StrSlice) {
    print(s);
    print("\n");
}


type String = struct {
    capacity u64,
    slice StrSlice
}


trait Drop {
    fn drop(self @mut Self);
}


impl Drop for String {
    fn drop(self @mut Self) {
        if self.slice.pointer != 0 {
            free(self.slice.pointer);
        }
    }
}


impl Self for String {
    fn with_capacity(cap u64) String {
        if cap > 0 {
            return String {
                slice: StrSlice {
                    pointer: malloc(cap),
                    len: 0
                },
                capacity: cap
            };
        } else {
            return String {
                slice: "",
                capacity: cap
            };
        }
    }


    fn new() String {
        return String.with_capacity(0);
    }

    fn grow(self @mut String, new_cap u64) {
        let new_string = String.with_capacity(new_cap);
        memcpy(new_string.slice.pointer, self.slice.pointer, self.slice.len);
        new_string.slice.len = self.slice.len;
        self = new_string;
    }

    fn push_str(self @mut String, s StrSlice) {
        if self.capacity < (self.slice.len + s.len) {
            self.grow((self.slice.len + s.len) * 2);
        } 
        memcpy(self.slice.pointer + self.slice.len, s.pointer, s.len);
        self.slice.len = (self.slice.len + s.len);
    }

    fn push_byte(self @mut String, c i8) {
        if self.capacity < (self.slice.len + 1) {
            self.grow((self.slice.len + 1) * 2);
        } 
        (self.slice.len + self.slice.pointer) = c;
        self.slice.len = (self.slice.len + 1);
        
    }

    fn truncate(self @mut String, new_len u64) {
        let s = self.slice;
        s.len = new_len;
    }

    fn clear(self @mut String) {
        self.truncate(0);
    }

    fn fromStr(slice StrSlice) String {
        mut s = String.with_capacity(slice.len);
        s.push_str(slice);
        return s;
    }
}

type StrVec = Vec[u8];


fn main(argc i32, argv i64) i32 { 
    let s = "hello world\n";


    let a = 32;
    
    mut string_test = String.new();
    string_test.push_str(s);
    string_test.push_str(s);
    string_test.push_str(s);
    string_test.push_byte('h' as i8);
    string_test.push_byte('e' as i8);
    string_test.push_byte('l' as i8);
    string_test.push_byte('l' as i8);
    string_test.push_byte('0' as i8);

    println(string_test.slice);

    let b = (1 + a) * a / 2 + string_test.len() >> 3;
    mut i = 0;
    loop {
        if i >= argc as i64 {
            break;
        }

        let arg_char_ptr = *(argv + i * 8);
        let arg_len = strlen(arg_char_ptr);

        println(StrSlice{ pointer: arg_char_ptr, len: arg_len});

        i = i + 1;
    }

    return string_test.capacity as i32; 
}


