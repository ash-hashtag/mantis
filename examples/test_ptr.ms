fn malloc(size i64) i64 extern;
fn free(ptr i64) extern;

type Point = struct {
    x i64,
    y i64
}

fn main() i32 {
    // 1. Pointer manipulation & Pointer to struct access
    let p_ptr @Point = malloc(#size_of(Point)) as @Point;
    
    // Direct access through pointer (uses my fix in compile_nested_struct_access)
    p_ptr.x = 100;
    p_ptr.y = 200;

    let x = p_ptr.x;
    let y = p_ptr.y;

    // 2. Address-of operator
    let a = 42;
    let b &i64 = &a;
    let val_a = @b; // Dereference

    // 3. Compiler macro #as_ref
    // Convert pointer to reference
    let p_ref &Point = #as_ref(p_ptr);
    let rx = p_ref.x;
    let ry = p_ref.y;

    free(p_ptr as i64);

    if x == 100 {
        if y == 200 {
            if val_a == 42 {
                if rx == 100 {
                    if ry == 200 {
                        // All tests passed!
                        return 0;
                    }
                }
            }
        }
    }
    return 1;
}
