use std;
use async_http;

fn app_handler(req StrSlice) StrSlice {
    return "<h1>Hello World! Powered by Mantis HTTP Library</h1>\n";
}

fn main() i32 {
    return serve_http(8080, app_handler as i64);
}
