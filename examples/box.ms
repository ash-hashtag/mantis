// Box[T] — Rust-style owning heap allocation with RAII.
//
// Box takes care of the raw pointer AND the cleanup for you: values are
// freed automatically when they go out of scope (via the Drop trait).
// Regular code should never need raw pointers (`@T`) unless doing FFI.

use std;

type Point = struct {
    x i64,
    y i64
}

fn main() i32 {
    // ── Box a primitive ────────────────────────────────────────────────
    let num = Box.new(42);
    let v = num.get();          // read: 42
    num.set(7);                 // overwrite
    let v2 = num.get();         // read: 7

    // ── Box a struct ───────────────────────────────────────────────────
    let p = Box.new(Point { x: 10, y: 20 });
    let sum = p.get().x + p.get().y;
    // `p` is freed automatically at the end of main's scope chain.

    // ── Escape hatch ───────────────────────────────────────────────────
    // Raw pointer access exists (boxed.as_ptr()) for FFI and manual
    // control, but you only need it when you really, really know it.

    if v == 42 {
        if v2 == 7 {
            if sum == 30 {
                return 0;
            }
        }
    }
    return 1;
}
