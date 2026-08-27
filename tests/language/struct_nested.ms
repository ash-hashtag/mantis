type Point = struct {
    x i64,
    y i64
}

type Rect = struct {
    origin Point,
    width i64,
    height i64
}

fn main() i32 {
    let r = Rect {
        origin: Point { x: 3, y: 4 },
        width: 100,
        height: 200
    };
    let ox = r.origin.x;
    let oy = r.origin.y;
    let total = ox + oy + r.width + r.height;
    return total as i32;
}
