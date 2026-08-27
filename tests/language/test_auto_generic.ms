fn add(a, b) {
    return a + b;
}

fn identity(x) {
    return x;
}

fn bar(x) {
    if x > 10 {
        return 1;
    } else {
        return 0;
    }
}

fn do_nothing(x) {
    let y = x;
}

fn main() i32 {
    let x = add(1, 2);
    let y = add(1.5, 2.5);
    let id_int = identity(42);
    let id_float = identity(3.14);
    let b = bar(15);
    do_nothing(x);
    return 0;
}
