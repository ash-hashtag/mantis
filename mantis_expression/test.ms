use std.libc as c;
use std.heap.ArenaAllocator;

type Foo = struct { a i32, b f32 }
type Gen[T] = struct { a T, b ptr[T] }
type Doo = i32
type Node[T] = enum { None, Binary(Op, T, ptr[T]), Unary(Op, T) }


fn malloc(size i64) i64 extern;
fn foo() i32 { return 74; }

fn main(argc i32, argv i64) i32 { let a = 29; mut b = 2 + a * 6; return a + b; }


fn fibonacci(n i32) i32 {
    let i = n;
    if n == 0  {
        return 0;
    } elif n == 1 { 
        return 1;
    } else {
        return fibonacci(n-1) + fibonacci(n - 2);
    }
}



fn loop_test(n i32) i32 {
    mut i = n;
    loop my_loop {
        println("{}", i);
        i = i - 1;
        if i < 0 {
            break;
        }
    }

    return i;
}


fn lamda_test()  {
    let v = Vec[i32].with_capacity(1024);
    let f = fn (x i32) i64 {
        return x as i64;
    };

    {           
        let fv = v.iter().map(v).collect[Vec[i64]]();
        let ufv = v.iter().map(fn (x i32) f64 { return x as f64; }).collect[Vec[f64]]();
        return fv;
    }
}


fn enum_test() {
    mut e: Node[i64] = Node.None;
    e = Node.Unary(Op.add, 68);

    match e  {
      Node.Unary(op, value) {
          println("op case");
      }
      Node.Binary(op, lhs, rhs) {
          println("{} {} {}", lhs, op, rhs);
      }
      _ {
          println("unhandled case");
      }
    }


    if Node.Unary(op, value) = e {
        return value;
    } else {
        return 10;
    }
}

fn struct_test() Foo {
	let foo = Foo { a = 10, b = 20, doo = Doo { a = 10, b = 69 } };
	return foo;
}