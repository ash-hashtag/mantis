use libc;

pub fn panic(msg Str) {
    let prefix = "panic: ";
    libc.write(2, prefix.pointer, prefix.len);
    libc.write(2, msg.pointer, msg.len);
    let nl = "\n";
    libc.write(2, nl.pointer, 1);
    libc.exit(1);
}

pub fn print_str(s Str) {
    libc.write(1, s.pointer, s.len);
}

pub fn println_str(s Str) {
    libc.write(1, s.pointer, s.len);
    let nl = "\n";
    libc.write(1, nl.pointer, 1);
}

pub fn print_int(n i64) {
    if n == 0 {
        let zero = "0";
        libc.write(1, zero.pointer, 1);
        return;
    }
    let mut temp = n;
    if temp < 0 {
        let minus = "-";
        libc.write(1, minus.pointer, 1);
        temp = -temp;
    }
    let mut buf = [0 as u8, 0 as u8, 0 as u8, 0 as u8, 0 as u8, 0 as u8, 0 as u8, 0 as u8, 0 as u8, 0 as u8, 0 as u8, 0 as u8, 0 as u8, 0 as u8, 0 as u8, 0 as u8, 0 as u8, 0 as u8, 0 as u8, 0 as u8];
    let mut count = 0;
    loop {
        if temp == 0 {
            break;
        }
        let rem = temp % 10;
        let digit = (48 + rem) as u8;
        let ptr = (buf as @u8) + count;
        ptr @= digit;
        count = count + 1;
        temp = temp / 10;
    }
    // Print reversed digits
    loop {
        if count == 0 {
            break;
        }
        count = count - 1;
        let ptr = (buf as @u8) + count;
        libc.write(1, ptr as i64, 1);
    }
}
