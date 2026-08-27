// Dynamic arrays with RAII, Box[Array[T]] allocation and Array[T] slicing.
//
// std::Vec handles growth and automatic destruction without manual malloc/free.
use std;

fn main() i32 {
    let mut v = Vec.with_capacity[u8](8);
    v.push(104 as u8); // 'h'
    v.push(101 as u8); // 'e'
    v.push(108 as u8); // 'l'
    v.push(108 as u8); // 'l'
    v.push(111 as u8); // 'o'

    let arr = v.as_array();
    libc.write(1, arr.ptr as i64, arr.len() as i64);
    libc.write(1, "\n".pointer, 1);

    let last = v.pop();        // pops 'o'
    v.push(33 as u8);          // '!'

    let arr2 = v.as_array();
    libc.write(1, arr2.ptr as i64, arr2.len() as i64);
    libc.write(1, "\n".pointer, 1);

    let n = v.len();
    // `v` is freed automatically when main returns via Box / RAII drop.

    if last == 111 as u8 {
        if n == 5 {
            return 0;
        }
    }
    return 1;
}
