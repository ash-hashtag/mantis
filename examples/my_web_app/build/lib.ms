use std;

fn serve_http(port i32, handler_fn i64) i32 {
    let server_fd = socket(2, 1, 0);
    if server_fd < 0 {
        print_str("Error: Failed to create socket\n");
        return 1;
    }

    let mut opt = 1;
    setsockopt(server_fd, 1, 2, &opt, 4);

    let mut addr = sockaddr_in {
        sin_family: 2,
        sin_port: htons(port as u16),
        sin_addr: 0,
        zero_0: 0
    };

    let bind_res = bind(server_fd, &addr, 16);
    if bind_res < 0 {
        print_str("Error: Failed to bind socket\n");
        return 1;
    }

    listen(server_fd, 10);
    print_str("Mantis HTTP Server running on http://127.0.0.1:");
    print_int(port as i64);
    print_str(" ...\n");

    let mut client_addr = sockaddr_in { sin_family: 0, sin_port: 0, sin_addr: 0, zero_0: 0 };
    let mut client_len = 16 as u32;

    loop {
        let client_fd = accept(server_fd, &client_addr, &client_len);
        if client_fd >= 0 {
            let mut req_buf: [u8] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
            let req_len = read(client_fd as i64, req_buf as i64, 64);

            let header = "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\n\r\n";
            write(client_fd as i64, header.pointer, header.len);

            if handler_fn != 0 {
                let handler = handler_fn as (StrSlice) StrSlice;
                let body = handler(StrSlice { pointer: req_buf as i64, len: req_len });
                write(client_fd as i64, body.pointer, body.len);
            } else {
                let default_body = "<h1>Hello from Mantis async_http!</h1>";
                write(client_fd as i64, default_body.pointer, default_body.len);
            }

            shutdown(client_fd, 1);
            loop {
                let n = read(client_fd as i64, req_buf as i64, 64);
                if n <= 0 {
                    break;
                }
            }
            close(client_fd);
        }
    }
    return 0;
}
