// Dynamic arrays and heap values WITHOUT manual pointer juggling.
//
// This used to demonstrate hand-rolled malloc/free arrays. With
// std::Vec the allocator handles growth AND cleanup (RAII) for you.

use std;

fn write(fd i64, buf i64, len i64) i64 extern;

fn main() i32 {
    // A growable vector of bytes — no malloc/free in sight.
    let mut v = Vec.with_capacity[u8](8);
    v.push(104 as u8); // 'h'
    v.push(101 as u8); // 'e'
    v.push(108 as u8); // 'l'
    v.push(108 as u8); // 'l'
    v.push(111 as u8); // 'o'

    write(1, v.ptr as i64, v.len() as i64);
    write(1, "\n".pointer, 1);

    let last = v.pop();        // pops 'o'
    v.push(33 as u8);          // '!'

    write(1, v.ptr as i64, v.len() as i64);
    write(1, "\n".pointer, 1);

    let n = v.len();
    // `v` is freed automatically when main returns (RAII).

    if last == 111 as u8 {
        if n == 5 {
            return 0;
        }
    }
    return 1;
}

// ── When you truly need one value on the heap ────────────────────────────
//
//   let b = Box.new(Point { x: 1, y: 2 });
//   let x = b.get().x;
//   // freed automatically at end of scope
//
// Raw pointers (@T) are reserved for FFI and low-level code — see
// examples/box.ms for the escape hatch.
