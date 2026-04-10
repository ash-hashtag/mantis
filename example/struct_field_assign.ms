fn write(fd i64, buf i64, len i64) i64 extern;

type Point = struct {
    x i64,
    y i64
}

fn main() i32 {
    mut p = Point { x: 10, y: 20 };
    p.x = 5;
    p.y = 7;
    let sum = p.x + p.y;
    return sum as i32;
}
