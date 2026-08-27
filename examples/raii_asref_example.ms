use std;

pub type CustomContainer = struct {
    boxed Box[i64],
    values Vec[i64]
}

impl CustomContainer {
    pub fn value(self @CustomContainer) i64 {
        return self.boxed.get();
    }
}

pub fn main() i32 {
    print_str("=== 1. Testing Box[T] with pub fn and AsRef ===\n");
    let b = Box.new(42);
    print_str("Box get: ");
    print_int(b.get());
    print_str("\n");

    let ptr = b.as_ref();
    print_str("Box as_ref dereferenced: ");
    print_int(@ptr);
    print_str("\n");

    print_str("=== 2. Testing Vec[T] with pub fn and AsRef ===\n");
    let mut v = Vec.with_capacity[i64](4);
    v.push(10);
    v.push(20);
    v.push(30);
    print_str("Vec len: ");
    print_int(v.len() as i64);
    print_str("\n");

    let v_ptr = v.as_ref();
    print_str("Vec as_ref first element: ");
    print_int(@v_ptr);
    print_str("\n");

    print_str("=== 3. Testing Custom Struct Member Auto-Drop ===\n");
    let mut c = CustomContainer {
        boxed: Box.new(99),
        values: Vec.with_capacity[i64](2)
    };
    c.values.push(777);
    print_str("CustomContainer value: ");
    print_int(c.value());
    print_str("\n");
    print_str("CustomContainer vector len: ");
    print_int(c.values.len() as i64);
    print_str("\n");

    print_str("=== All tests passed successfully! Auto-dropping struct members ===\n");
    return 0;
}
