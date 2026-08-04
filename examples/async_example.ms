use std;

type Poll[T] = enum {
    Ready(T),
    Pending
}

type NumberGenerator = struct {
    state u32,
    count i32,
    max_count i32
}

impl NumberGenerator {
    fn new(max_count i32) @NumberGenerator {
        let gen = malloc(#size_of(NumberGenerator)) as @NumberGenerator;
        gen.state = 0;
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

fn main() i32 {
    print_str("=== Rust-style Async Generators & Pollable State Machines ===\n");

    let gen = NumberGenerator.new(3);

    let p1 = gen.poll();
    match p1 {
        Poll.Ready(val) : {
            print_str("Poll 1 Ready: ");
            print_int(val as i64);
            print_str("\n");
        },
        Poll.Pending : {
            print_str("Poll 1 Pending\n");
        }
    }

    let p2 = gen.poll();
    match p2 {
        Poll.Ready(val) : {
            print_str("Poll 2 Ready: ");
            print_int(val as i64);
            print_str("\n");
        },
        Poll.Pending : {
            print_str("Poll 2 Pending\n");
        }
    }

    let p3 = gen.poll();
    match p3 {
        Poll.Ready(val) : {
            print_str("Poll 3 Ready: ");
            print_int(val as i64);
            print_str("\n");
        },
        Poll.Pending : {
            print_str("Poll 3 Pending\n");
        }
    }

    let p4 = gen.poll();
    match p4 {
        Poll.Ready(val) : {
            print_str("Poll 4 Ready: ");
            print_int(val as i64);
            print_str("\n");
        },
        Poll.Pending : {
            print_str("Poll 4 Pending (Finished)\n");
        }
    }

    print_str("=== Async Function Evaluation ===\n");
    let sum = async_add(15, 27);
    print_str("Async sum result: ");
    print_int(sum as i64);
    print_str("\n");

    return 0;
}
