use std;

fn print_int_array(arr Array[i64]) {
    print_str("[");
    let mut i = 0 as u64;
    let n = arr.len();
    loop {
        if i >= n {
            break;
        }
        print_int(arr.get(i));
        if i + 1 < n {
            print_str(", ");
        }
        i = i + 1;
    }
    print_str("]\n");
}

fn greet(name Str) {
    print_str("Hello, ");
    print_str(name);
    print_str("! (len = ");
    print_int(name.len);
    print_str(")\n");
}

pub fn main() i32 {
    print_str("=== 1. Testing Vec[T] -> Array[T] Slice View ===\n");
    let mut vec = Vec.with_capacity[i64](8);
    vec.push(10);
    vec.push(20);
    vec.push(30);
    vec.push(40);
    vec.push(50);

    let arr = vec.as_array();
    print_str("Full Vec as Array (len = ");
    print_int(arr.len() as i64);
    print_str("): ");
    print_int_array(arr);

    let sub_arr = arr.slice(1, 4);
    print_str("Sub-slice [1..4] (len = ");
    print_int(sub_arr.len() as i64);
    print_str("): ");
    print_int_array(sub_arr);

    print_str("=== 2. Testing String -> Str & String -> Array[u8] ===\n");
    let mut s = String.from_str("Mantis Language");
    greet(s.as_str());

    let u8_arr = s.as_array();
    print_str("String as Array[u8] len = ");
    print_int(u8_arr.len() as i64);
    print_str("\n");

    print_str("=== 3. Testing Direct String Literal passing as Str ===\n");
    greet("World of Systems");

    print_str("=== All Array[T], Str & Vec[T] conversions successful! ===\n");
    return 0;
}
