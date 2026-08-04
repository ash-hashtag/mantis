// Example HTTP File Server using C libc socket bindings in Mantis

use std;

// ── C Libc Function Declarations ─────────────────────────────────────────────

fn socket(domain i32, ty i32, protocol i32) i32 extern ;
fn setsockopt(sockfd i32, level i32, optname i32, optval @i32, optlen u32) i32 extern ;
fn bind(sockfd i32, addr @sockaddr_in, addrlen u32) i32 extern ;
fn listen(sockfd i32, backlog i32) i32 extern ;
fn accept(sockfd i32, addr @sockaddr_in, addrlen @u32) i32 extern ;
fn read(fd i64, data_ptr i64, count i64) i64 extern ;
fn write(fd i64, data_ptr i64, count i64) i64 extern ;
fn close(fd i32) i32 extern ;
fn shutdown(sockfd i32, how i32) i32 extern ;
fn open(pathname i64, flags i32) i32 extern ;
fn atoi(str i64) i32 extern ;
fn strlen(str i64) u64 extern ;

// ── C Struct Layouts & Constants ─────────────────────────────────────────────

type sockaddr_in = struct {
    sin_family u16,
    sin_port u16,
    sin_addr u32,
    zero_0 u64
}

fn htons(port u16) u16 {
    let p = port as u64;
    let high = p / 256;
    let low = p % 256;
    return (low * 256 + high) as u16;
}

fn send_string(fd i32, msg StrSlice) {
    write(fd as i64, msg.pointer, msg.len);
}

fn drain_and_close(fd i32) {
    shutdown(fd, 1); // SHUT_WR = 1 sends FIN
    let mut buf: [u8] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    loop {
        let n = read(fd as i64, buf as i64, 64);
        if n <= 0 {
            break;
        }
    }
    close(fd);
}

fn serve_file(client_fd i32, default_file StrSlice) {
    let mut req_buf: [u8] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let req_len = read(client_fd as i64, req_buf as i64, 128);

    let mut target_path_ptr = default_file.pointer;
    if req_len > 5 {
        let req_ptr = req_buf as i64;
        let b0 = @(req_ptr as @u8);
        let b1 = @((req_ptr + 1) as @u8);
        let b2 = @((req_ptr + 2) as @u8);
        let b3 = @((req_ptr + 3) as @u8);

        // Check if request starts with "GET " ('G'=71, 'E'=69, 'T'=84, ' '=32)
        if b0 == 71 {
            if b1 == 69 {
                if b2 == 84 {
                    if b3 == 32 {
                        let mut path_end = 4;
                        loop {
                            if path_end >= req_len {
                                break;
                            }
                            let b = @((req_ptr + path_end) as @u8);
                            if b == 32 { // ' '
                                let end_ptr = (req_ptr + path_end) as @u8;
                                end_ptr @= 0;
                                break;
                            }
                            if b == 63 { // '?'
                                let end_ptr = (req_ptr + path_end) as @u8;
                                end_ptr @= 0;
                                break;
                            }
                            path_end = path_end + 1;
                        }

                        let raw_path_ptr = req_ptr + 4; // Skip "GET "
                        let first_char = @(raw_path_ptr as @u8);
                        let second_char = @((raw_path_ptr + 1) as @u8);

                        if first_char == 47 { // '/'
                            if second_char == 0 {
                                target_path_ptr = default_file.pointer;
                            } else {
                                target_path_ptr = raw_path_ptr + 1; // Strip leading '/'
                            }
                        } else {
                            target_path_ptr = raw_path_ptr;
                        }
                    }
                }
            }
        }
    }

    print_str("Serving path: ");
    let path_len = strlen(target_path_ptr) as i64;
    print_str(StrSlice { pointer: target_path_ptr, len: path_len });
    print_str("\n");

    let file_fd = open(target_path_ptr, 0); // O_RDONLY = 0
    if file_fd >= 0 {
        let header = "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\n\r\n";
        send_string(client_fd, header);

        let mut file_buf: [u8] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        loop {
            let bytes_read = read(file_fd as i64, file_buf as i64, 64);
            if bytes_read <= 0 {
                break;
            }
            write(client_fd as i64, file_buf as i64, bytes_read);
        }
        close(file_fd);
    } else {
        let not_found = "HTTP/1.1 404 Not Found\r\nContent-Type: text/plain\r\nContent-Length: 13\r\nConnection: close\r\n\r\n404 Not Found";
        send_string(client_fd, not_found);
    }
}

fn main(argc i32, argv i64) i32 {
    let mut port = 8080;
    if argc >= 2 {
        let arg1_ptr = @((argv + 8) as @i64);
        let parsed_port = atoi(arg1_ptr);
        if parsed_port > 0 {
            port = parsed_port;
        }
    }

    let mut file_to_serve = "index.html";
    if argc >= 3 {
        let arg2_ptr = @((argv + 16) as @i64);
        let arg2_len = strlen(arg2_ptr) as i64;
        file_to_serve = StrSlice { pointer: arg2_ptr, len: arg2_len };
    }

    print_str("Starting Mantis HTTP File Server on http://127.0.0.1:");
    print_int(port as i64);
    print_str(" ... serving '");
    print_str(file_to_serve);
    print_str("'\n");

    // Create TCP socket: AF_INET = 2, SOCK_STREAM = 1, Protocol = 0
    let server_fd = socket(2, 1, 0);
    if server_fd < 0 {
        print_str("Error: Failed to create socket\n");
        return 1;
    }

    // Set SO_REUSEADDR = 2 (SOL_SOCKET = 1)
    let mut opt = 1;
    setsockopt(server_fd, 1, 2, &opt, 4);

    // Bind to 127.0.0.1:port
    let mut addr = sockaddr_in {
        sin_family: 2,                     // AF_INET = 2
        sin_port: htons(port as u16),      // Port in network byte order
        sin_addr: 0,                       // INADDR_ANY = 0
        zero_0: 0
    };

    let bind_res = bind(server_fd, &addr, 16);
    if bind_res < 0 {
        print_str("Error: Failed to bind socket\n");
        close(server_fd);
        return 1;
    }

    // Listen with backlog of 10
    if listen(server_fd, 10) < 0 {
        print_str("Error: Failed to listen\n");
        close(server_fd);
        return 1;
    }

    print_str("Server listening... Ready for connections.\n");

    let mut client_addr = sockaddr_in { sin_family: 0, sin_port: 0, sin_addr: 0, zero_0: 0 };
    let mut client_len = 16 as u32;

    // Server loop to accept connections
    let mut request_count = 0;
    loop {
        let client_fd = accept(server_fd, &client_addr, &client_len);
        if client_fd >= 0 {
            print_str("Accepted client connection #");
            print_int(request_count + 1);
            print_str("\n");

            serve_file(client_fd, file_to_serve);
            drain_and_close(client_fd);

            request_count = request_count + 1;
            // Limit request count for test/example termination
            if request_count >= 100 {
                break;
            }
        }
    }

    close(server_fd);
    return 0;
}
