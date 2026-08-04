// Test usage of std library components

use std;

type VecInt = Vec[i64]

fn test_vec() i64 {
    let v = Vec.with_capacity[i64](4);
    v.push(10);
    v.push(20);
    v.push(30);

    let len = v.len();
    let elem = v.get(1);
    v.free();

    if len == 3 {
        return elem;
    }
    return 0;
}

fn test_string() i64 {
    let str = String.from_str("hello mantis");
    let len = str.len();
    str.free();
    return len as i64;
}

fn test_option() i64 {
    let opt = Option.Some(42);
    if opt.is_some() {
        return opt.unwrap();
    }
    return 0;
}

fn main() i32 {
    let elem = test_vec();
    if elem != 20 {
        return 1;
    }

    let str_len = test_string();
    if str_len != 12 {
        return 2;
    }

    let opt_val = test_option();
    if opt_val != 42 {
        return 3;
    }

    return 0;
}
