use std;

fn compute_magic(x i32) i32 extern ;

fn main() i32 {
    print_str("=== Testing Mantis 'mat' Package Manager ===\n");
    let result = compute_magic(10);
    print_str("Computed result from local dependency 'my_lib': ");
    print_int(result as i64);
    print_str("\n");
    return 0;
}
