// std.fs — filesystem operations.

use libc;
use std.string;
use std.vec;
use std.option;
use std.result;

// ── Open Flags & Options ────────────────────────────────────────────────────

pub type OpenOptions = struct {
    read_flag bool,
    write_flag bool,
    create_flag bool,
    truncate_flag bool,
    append_flag bool
}

impl OpenOptions {
    pub fn new() OpenOptions {
        return OpenOptions {
            read_flag: false,
            write_flag: false,
            create_flag: false,
            truncate_flag: false,
            append_flag: false
        };
    }

    pub fn read(self @OpenOptions, r bool) OpenOptions {
        return OpenOptions {
            read_flag: r,
            write_flag: self.write_flag,
            create_flag: self.create_flag,
            truncate_flag: self.truncate_flag,
            append_flag: self.append_flag
        };
    }

    pub fn write(self @OpenOptions, w bool) OpenOptions {
        return OpenOptions {
            read_flag: self.read_flag,
            write_flag: w,
            create_flag: self.create_flag,
            truncate_flag: self.truncate_flag,
            append_flag: self.append_flag
        };
    }

    pub fn create(self @OpenOptions, c bool) OpenOptions {
        return OpenOptions {
            read_flag: self.read_flag,
            write_flag: self.write_flag,
            create_flag: c,
            truncate_flag: self.truncate_flag,
            append_flag: self.append_flag
        };
    }

    pub fn truncate(self @OpenOptions, t bool) OpenOptions {
        return OpenOptions {
            read_flag: self.read_flag,
            write_flag: self.write_flag,
            create_flag: self.create_flag,
            truncate_flag: t,
            append_flag: self.append_flag
        };
    }

    pub fn append(self @OpenOptions, a bool) OpenOptions {
        return OpenOptions {
            read_flag: self.read_flag,
            write_flag: self.write_flag,
            create_flag: self.create_flag,
            truncate_flag: self.truncate_flag,
            append_flag: a
        };
    }

    pub fn open(self @OpenOptions, path Str) Result[File, i32] {
        let mut flags = 0;
        if self.read_flag && self.write_flag {
            flags = 2; // O_RDWR
        } elif self.write_flag {
            flags = 1; // O_WRONLY
        } else {
            flags = 0; // O_RDONLY
        }

        if self.create_flag {
            flags = flags + 64; // O_CREAT
        }
        if self.truncate_flag {
            flags = flags + 512; // O_TRUNC
        }
        if self.append_flag {
            flags = flags + 1024; // O_APPEND
        }

        let mut path_buf = String.with_capacity((path.len + 1) as u64);
        path_buf.push_str(path);
        path_buf.push_byte(0);
        let path_ptr = path_buf.as_ptr() as i64;

        let fd = libc.open(path_ptr, flags, 438); // mode 0666 = 438
        if fd < 0 {
            print_str("libc.open failed with fd = ");
            print_int(fd as i64);
            print_str("
");
            return Result.Err[File, i32](fd);
        }
        return Result.Ok[File, i32](File { fd: fd });
    }
}

// ── File Handle ─────────────────────────────────────────────────────────────

pub type File = struct {
    fd i32
}

impl File {
    pub fn open(path Str) Result[File, i32] {
        let opt = OpenOptions.new();
        let read_opt = opt.read(true);
        return read_opt.open(path);
    }

    pub fn create(path Str) Result[File, i32] {
        let opt = OpenOptions.new();
        let w_opt = opt.write(true);
        let c_opt = w_opt.create(true);
        let tr_opt = c_opt.truncate(true);
        return tr_opt.open(path);
    }

    pub fn read(self @File, buf @u8, count u64) i64 {
        return libc.read(self.fd as i64, buf as i64, count as i64);
    }

    pub fn read_to_string(self @File) Result[String, i32] {
        let mut res = String.new();
        let buf_ptr = GlobalAllocator.alloc(1024, 1);
        loop {
            let n = libc.read(self.fd as i64, buf_ptr, 1024);
            if n <= 0 {
                break;
            }
            let slice = StrSlice { pointer: buf_ptr, len: n };
            res.push_str(slice);
        }
        GlobalAllocator.dealloc(buf_ptr);
        return Result.Ok[String, i32](res);
    }

    pub fn write_str(self @File, s Str) i64 {
        return libc.write(self.fd as i64, s.pointer, s.len);
    }

    pub fn write_all(self @File, s Str) Result[u64, i32] {
        let mut total = 0;
        loop {
            if total >= s.len {
                break;
            }
            let remaining = s.len - total;
            let ptr = s.pointer + total;
            let n = libc.write(self.fd as i64, ptr, remaining);
            if n <= 0 {
                return Result.Err[u64, i32](n as i32);
            }
            total = total + n;
        }
        return Result.Ok[u64, i32](total as u64);
    }

    pub fn close(self @mut File) void {
        if self.fd >= 0 {
            libc.close(self.fd);
            self.fd = -1;
        }
    }

    pub fn drop(self @mut File) void {
        if self.fd >= 0 {
            libc.close(self.fd);
            self.fd = -1;
        }
    }
}

// ── Top-level Helper Functions ──────────────────────────────────────────────

pub fn read_to_string(path Str) Result[String, i32] {
    let f_res = File.open(path);
    if f_res.is_err() {
        return Result.Err[String, i32](f_res.unwrap_err());
    }
    let f = f_res.unwrap();
    let content_res = f.read_to_string();
    return content_res;
}

pub fn write(path Str, contents Str) Result[u64, i32] {
    let f_res = File.create(path);
    if f_res.is_err() {
        return Result.Err[u64, i32](f_res.unwrap_err());
    }
    let f = f_res.unwrap();
    return f.write_all(contents);
}

pub fn exists(path Str) bool {
    let mut path_buf = String.with_capacity((path.len + 1) as u64);
    path_buf.push_str(path);
    path_buf.push_byte(0);
    let path_ptr = path_buf.as_ptr() as i64;
    // F_OK = 0
    let res = libc.access(path_ptr, 0);
    return res == 0;
}

pub fn remove_file(path Str) Result[bool, i32] {
    let mut path_buf = String.with_capacity((path.len + 1) as u64);
    path_buf.push_str(path);
    path_buf.push_byte(0);
    let path_ptr = path_buf.as_ptr() as i64;
    let res = libc.unlink(path_ptr);
    if res == 0 {
        return Result.Ok[bool, i32](true);
    }
    return Result.Err[bool, i32](res);
}

pub fn create_dir(path Str) Result[bool, i32] {
    let mut path_buf = String.with_capacity((path.len + 1) as u64);
    path_buf.push_str(path);
    path_buf.push_byte(0);
    let path_ptr = path_buf.as_ptr() as i64;
    let res = libc.mkdir(path_ptr, 493); // 0755 = 493
    if res == 0 {
        return Result.Ok[bool, i32](true);
    }
    return Result.Err[bool, i32](res);
}

pub fn create_dir_all(path Str) Result[bool, i32] {
    let res = create_dir(path);
    if res.is_ok() {
        return Result.Ok[bool, i32](true);
    }
    if exists(path) {
        return Result.Ok[bool, i32](true);
    }

    let mut current = String.new();
    let mut i = 0;
    loop {
        if i >= path.len {
            break;
        }
        let ptr = (path.pointer + i) as @u8;
        let b = @(ptr);
        if b == 47 { // '/'
            if current.len() > 0 {
                let _ = create_dir(current.as_str());
            }
        }
        current.push_byte(b);
        i = i + 1;
    }
    if current.len() > 0 {
        let _ = create_dir(current.as_str());
    }
    return Result.Ok[bool, i32](true);
}
