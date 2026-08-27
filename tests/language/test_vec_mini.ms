use std;

fn print_int_array(arr Array[i64]) {
    print_str("inside fn get(0): ");
    print_int(arr.get(0));
    print_str("
");
    print_str("inside fn get(1): ");
    print_int(arr.get(1));
    print_str("
");
    print_str("inside fn get(2): ");
    print_int(arr.get(2));
    print_str("
");
}

pub fn main() i32 {
    let mut vec = Vec.with_capacity[i64](8);
    vec.push(10);
    vec.push(20);
    vec.push(30);

    let arr = vec.as_array();
    print_str("in main get(0): ");
    print_int(arr.get(0));
    print_str("
");
    print_str("in main get(1): ");
    print_int(arr.get(1));
    print_str("
");
    print_str("in main get(2): ");
    print_int(arr.get(2));
    print_str("
");

    print_int_array(arr);
    return 0;
}
