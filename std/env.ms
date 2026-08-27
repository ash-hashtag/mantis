use libc;
use std.string;
use std.vec;

pub fn args() Vec[String] {
    let argc = libc.mantis_get_argc();
    let argv = libc.mantis_get_argv();
    let mut result = Vec {
        ptr: 0 as @String,
        size: 0,
        cap: 0
    };
    if (argc as i64) > 0 {
        let p = libc.malloc((argc as i64) * 24);
        result.ptr = p as @String;
        result.cap = argc as u64;
    }
    if argv == 0 {
        return result;
    }
    let mut i = 0 as i64;
    loop {
        if i >= (argc as i64) {
            break;
        }
        let c_str = @((argv + i * 8) as @i64);
        if c_str == 0 {
            let s = String.new();
            libc.memcpy((result.ptr as i64 + (result.size as i64) * 24) as @i64, (&s) as @i64, 24);
            result.size = result.size + 1;
        } else {
            let s_len = libc.strlen(c_str);
            let arg_slice = StrSlice {
                pointer: c_str,
                len: s_len as i64
            };
            let s = String.from_str(arg_slice);
            libc.memcpy((result.ptr as i64 + (result.size as i64) * 24) as @i64, (&s) as @i64, 24);
            result.size = result.size + 1;
        }
        i = i + 1;
    }
    return result;
}

pub fn var(key Str) Option[String] {
    if key.len <= 0 {
        return Option.None[String]();
    }
    let mut k_buf = libc.malloc(key.len + 1);
    libc.memcpy(k_buf as @i64, key.pointer as @i64, key.len);
    libc.memset((k_buf + key.len) as @i64, 0, 1);
    let val_ptr = libc.getenv(k_buf);
    libc.free(k_buf);
    if val_ptr == 0 {
        return Option.None[String]();
    }
    let val_len = libc.strlen(val_ptr);
    let val_slice = StrSlice {
        pointer: val_ptr,
        len: val_len as i64
    };
    return Option.Some[String](String.from_str(val_slice));
}

pub fn set_var(key Str, value Str) bool {
    let mut k_buf = libc.malloc(key.len + 1);
    libc.memcpy(k_buf as @i64, key.pointer as @i64, key.len);
    libc.memset((k_buf + key.len) as @i64, 0, 1);
    let mut v_buf = libc.malloc(value.len + 1);
    libc.memcpy(v_buf as @i64, value.pointer as @i64, value.len);
    libc.memset((v_buf + value.len) as @i64, 0, 1);
    let res = libc.setenv(k_buf, v_buf, 1);
    libc.free(k_buf);
    libc.free(v_buf);
    return res == 0;
}

pub fn remove_var(key Str) bool {
    let mut k_buf = libc.malloc(key.len + 1);
    libc.memcpy(k_buf as @i64, key.pointer as @i64, key.len);
    libc.memset((k_buf + key.len) as @i64, 0, 1);
    let res = libc.unsetenv(k_buf);
    libc.free(k_buf);
    return res == 0;
}

pub fn current_dir() Option[String] {
    let mut buf = libc.malloc(4096);
    let res = libc.getcwd(buf, 4096);
    if res == 0 {
        libc.free(buf);
        return Option.None[String]();
    }
    let len = libc.strlen(res);
    let dir_slice = StrSlice {
        pointer: res,
        len: len as i64
    };
    let s = String.from_str(dir_slice);
    libc.free(buf);
    return Option.Some[String](s);
}
