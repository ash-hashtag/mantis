use std;

type NumberGenerator = struct {
    state u32,
    count i32,
    max_count i32
}

impl NumberGenerator {
    fn new(max_count i32) @NumberGenerator {
        let gen = malloc(#size_of(NumberGenerator)) as @NumberGenerator;
        gen.state = 0 as u32;
        gen.count = 0;
        gen.max_count = max_count;
        return gen;
    }

    fn poll(self @NumberGenerator) Poll[i32] {
        if self.count >= self.max_count {
            return Poll.Pending;
        }
        self.count = self.count + 1;
        let val = self.count * 10;
        return Poll.Ready(val);
    }
}

async fn async_add(a i32, b i32) i32 {
    let sum = a + b;
    return sum;
}

async fn async_multiply(x i32, y i32) i32 {
    let product = x * y;
    return product;
}

fn main() i32 {
    print_str("=== Rust-style Async Generators & Pollable State Machines ===
");

    let gen = NumberGenerator.new(3);

    let p1 = gen.poll();
    match p1 {
        Poll.Ready(val) : {
            print_str("Poll 1 Ready: ");
            print_int(val as i64);
            print_str("
");
        },
        Poll.Pending : {
            print_str("Poll 1 Pending
");
        }
    }

    let p2 = gen.poll();
    match p2 {
        Poll.Ready(val) : {
            print_str("Poll 2 Ready: ");
            print_int(val as i64);
            print_str("
");
        },
        Poll.Pending : {
            print_str("Poll 2 Pending
");
        }
    }

    let p3 = gen.poll();
    match p3 {
        Poll.Ready(val) : {
            print_str("Poll 3 Ready: ");
            print_int(val as i64);
            print_str("
");
        },
        Poll.Pending : {
            print_str("Poll 3 Pending
");
        }
    }

    let p4 = gen.poll();
    match p4 {
        Poll.Ready(val) : {
            print_str("Poll 4 Ready: ");
            print_int(val as i64);
            print_str("
");
        },
        Poll.Pending : {
            print_str("Poll 4 Pending (Finished)
");
        }
    }

    print_str("=== Async Function & Anonymous Future Struct Evaluation ===
");
    // 1. Calling async fn directly returns an anonymous Future struct:
    let fut = async_add(10, 20);
    let poll_res = fut.poll();
    match poll_res {
        Poll.Ready(val) : {
            print_str("Direct Future .poll() result: ");
            print_int(val as i64);
            print_str("
");
        },
        Poll.Pending : {
            print_str("Direct Future Pending
");
        }
    }

    // 2. Using .await on async function call directly:
    let sum = async_add(15, 27).await;
    print_str("Async .await sum result: ");
    print_int(sum as i64);
    print_str("
");

    // 3. Using .await on an existing Future instance:
    let fut_mult = async_multiply(7, 8);
    let mult_res = fut_mult.await;
    print_str("Async .await multiply result: ");
    print_int(mult_res as i64);
    print_str("
");

    return 0;
}
