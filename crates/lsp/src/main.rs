use mantis_lsp::MantisLanguageServer;
use serde_json::{json, Value};
use std::io::{self, BufRead, Read, Write};

fn main() -> io::Result<()> {
    let mut server = MantisLanguageServer::new();
    let stdin = io::stdin();
    let mut reader = stdin.lock();

    loop {
        let mut content_length: Option<usize> = None;
        let mut header_buf = String::new();

        loop {
            header_buf.clear();
            let bytes_read = reader.read_line(&mut header_buf)?;
            if bytes_read == 0 {
                return Ok(()); // EOF
            }
            let trimmed = header_buf.trim();
            if trimmed.is_empty() {
                break; // Header section ended
            }
            if trimmed.starts_with("Content-Length:") {
                if let Some(len_str) = trimmed.split(':').nth(1) {
                    content_length = len_str.trim().parse::<usize>().ok();
                }
            }
        }

        let len = match content_length {
            Some(l) => l,
            None => continue,
        };

        let mut body_buf = vec![0u8; len];
        reader.read_exact(&mut body_buf)?;

        let req: Value = match serde_json::from_slice(&body_buf) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let id = req.get("id").cloned();

        match method {
            "initialize" => {
                let resp = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "capabilities": {
                            "textDocumentSync": 1,
                            "hoverProvider": true,
                            "definitionProvider": true,
                            "completionProvider": { "resolveProvider": false },
                            "documentSymbolProvider": true
                        }
                    }
                });
                send_response(&resp)?;
            }
            "textDocument/didOpen" => {
                if let Some(params) = req.get("params") {
                    let uri = params["textDocument"]["uri"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();
                    let text = params["textDocument"]["text"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();
                    let diag_params = server.did_open(uri, text);
                    send_notification(
                        "textDocument/publishDiagnostics",
                        serde_json::to_value(diag_params).unwrap(),
                    )?;
                }
            }
            "textDocument/didChange" => {
                if let Some(params) = req.get("params") {
                    let uri = params["textDocument"]["uri"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();
                    if let Some(changes) = params["contentChanges"].as_array() {
                        if let Some(first_change) = changes.first() {
                            let text = first_change["text"].as_str().unwrap_or("").to_string();
                            let diag_params = server.did_change(uri, text);
                            send_notification(
                                "textDocument/publishDiagnostics",
                                serde_json::to_value(diag_params).unwrap(),
                            )?;
                        }
                    }
                }
            }
            "textDocument/hover" => {
                if let Some(params) = req.get("params") {
                    let uri = params["textDocument"]["uri"].as_str().unwrap_or("");
                    let line = params["position"]["line"].as_u64().unwrap_or(0) as u32;
                    let character = params["position"]["character"].as_u64().unwrap_or(0) as u32;
                    let hover_res = server.hover(uri, mantis_lsp::Position { line, character });
                    let resp = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": hover_res
                    });
                    send_response(&resp)?;
                }
            }
            "textDocument/definition" => {
                if let Some(params) = req.get("params") {
                    let uri = params["textDocument"]["uri"].as_str().unwrap_or("");
                    let line = params["position"]["line"].as_u64().unwrap_or(0) as u32;
                    let character = params["position"]["character"].as_u64().unwrap_or(0) as u32;
                    let location = server.definition_location(uri, mantis_lsp::Position { line, character });
                    let result = match location {
                        Some(location) => json!(location),
                        None => Value::Null,
                    };
                    let resp = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": result
                    });
                    send_response(&resp)?;
                }
            }
            "textDocument/completion" => {
                if let Some(params) = req.get("params") {
                    let uri = params["textDocument"]["uri"].as_str().unwrap_or("");
                    let line = params["position"]["line"].as_u64().unwrap_or(0) as u32;
                    let character = params["position"]["character"].as_u64().unwrap_or(0) as u32;
                    let items = server.completion(uri, mantis_lsp::Position { line, character });
                    let resp = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": items
                    });
                    send_response(&resp)?;
                }
            }
            "textDocument/documentSymbol" => {
                if let Some(params) = req.get("params") {
                    let uri = params["textDocument"]["uri"].as_str().unwrap_or("");
                    let symbols = server.document_symbols(uri);
                    let resp = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": symbols
                    });
                    send_response(&resp)?;
                }
            }
            "shutdown" => {
                let resp = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": null
                });
                send_response(&resp)?;
            }
            "exit" => {
                return Ok(());
            }
            _ => {
                if id.is_some() {
                    let resp = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": {
                            "code": -32601,
                            "message": "Method not found"
                        }
                    });
                    send_response(&resp)?;
                }
            }
        }
    }
}

fn send_response(val: &Value) -> io::Result<()> {
    let body = serde_json::to_string(val).unwrap();
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    let mut stdout = io::stdout().lock();
    stdout.write_all(header.as_bytes())?;
    stdout.write_all(body.as_bytes())?;
    stdout.flush()
}

fn send_notification(method: &str, params: Value) -> io::Result<()> {
    let val = json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params
    });
    send_response(&val)
}
