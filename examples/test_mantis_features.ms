// Comprehensive test covering Mantis language features

type Status = enum {
    Active(i64),
    Inactive
}

type Point = struct {
    x i64,
    y i64
}

impl Point {
    fn new(x i64, y i64) Point {
        let p = #init(Point);
        p.x = x;
        p.y = y;
        return p;
    }

    fn distance_sq(self Point) i64 {
        return self.x * self.x + self.y * self.y;
    }
}

// Auto-generic function with untyped parameters
fn multiply(a, b) {
    return a * b;
}

// Function with where clause
fn #compute[T](val T) i64 where T: Any {
    return 42;
}

fn test_pointers() i64 {
    let x = 100;
    let ptr = &x;
    let val = @ptr;
    return val;
}

fn test_enum_match(s Status) i64 {
    match s {
        Status.Active(val) => { return val; },
        Status.Inactive => { return 0; }
    }
}

fn main() i32 {
    let p = Point.new(3, 4);
    let dist = p.distance_sq();
    if dist != 25 {
        return 1;
    }

    let m1 = multiply(6, 7);
    let m2 = multiply(2.5, 2.0);

    let ptr_val = test_pointers();
    if ptr_val != 100 {
        return 2;
    }

    let active = Status.Active(42);
    let status_code = test_enum_match(active);
    if status_code != 42 {
        return 3;
    }

    let inactive = Status.Inactive;
    let inactive_code = test_enum_match(inactive);
    if inactive_code != 0 {
        return 4;
    }

    return 0;
}
