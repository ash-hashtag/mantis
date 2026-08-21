use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use mantis_parser::ast::{Declaration, Program};
use mantis_parser::borrow_checker::BorrowChecker;
use mantis_parser::token::Span;
use mantis_parser::{parse, MantisError};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  LSP Protocol Types
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub range: Range,
    pub severity: Option<u32>, // 1: Error, 2: Warning, 3: Information, 4: Hint
    pub code: Option<String>,
    pub source: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishDiagnosticsParams {
    pub uri: String,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hover {
    pub contents: MarkupContent,
    pub range: Option<Range>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkupContent {
    pub kind: String, // "markdown" or "plaintext"
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionItem {
    pub label: String,
    pub kind: Option<u32>, // 1: Text, 2: Method, 3: Function, 6: Variable, 14: Keyword, 22: Struct, 13: Enum
    pub detail: Option<String>,
    pub documentation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSymbol {
    pub name: String,
    pub detail: Option<String>,
    pub kind: u32, // 5: Class/Type, 6: Method, 12: Function, 23: Struct, 10: Enum
    pub range: Range,
    pub selection_range: Range,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Language Server State & Diagnostics
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct MantisLanguageServer {
    documents: HashMap<String, String>,
}

impl MantisLanguageServer {
    pub fn new() -> Self {
        Self {
            documents: HashMap::new(),
        }
    }

    pub fn did_open(&mut self, uri: String, text: String) -> PublishDiagnosticsParams {
        self.documents.insert(uri.clone(), text.clone());
        let diagnostics = compute_diagnostics(&text);
        PublishDiagnosticsParams { uri, diagnostics }
    }

    pub fn did_change(&mut self, uri: String, text: String) -> PublishDiagnosticsParams {
        self.documents.insert(uri.clone(), text.clone());
        let diagnostics = compute_diagnostics(&text);
        PublishDiagnosticsParams { uri, diagnostics }
    }

    pub fn hover(&self, uri: &str, pos: Position) -> Option<Hover> {
        let text = self.documents.get(uri)?;
        let offset = position_to_offset(text, pos)?;
        let prog = parse(text).ok()?;

        // Find symbol under position
        let sym_info = find_symbol_at_offset(&prog, offset)?;
        Some(Hover {
            contents: MarkupContent {
                kind: "markdown".to_string(),
                value: format!("```mantis\n{}\n```", sym_info),
            },
            range: None,
        })
    }

    pub fn completion(&self, uri: &str, _pos: Position) -> Vec<CompletionItem> {
        let mut items = Vec::new();

        // Standard keywords
        let keywords = vec![
            "fn", "let", "mut", "return", "if", "elif", "else", "loop", "break",
            "continue", "match", "type", "struct", "enum", "trait", "impl",
            "extern", "import", "use", "async", "await", "yield",
        ];
        for kw in keywords {
            items.push(CompletionItem {
                label: kw.to_string(),
                kind: Some(14), // Keyword
                detail: Some("Mantis keyword".to_string()),
                documentation: None,
            });
        }

        // Built-in types
        let types = vec!["i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "f32", "f64", "bool", "char", "String"];
        for ty in types {
            items.push(CompletionItem {
                label: ty.to_string(),
                kind: Some(22), // Struct/Type
                detail: Some("Primitive type".to_string()),
                documentation: None,
            });
        }

        // Symbols in current document
        if let Some(text) = self.documents.get(uri) {
            if let Ok(prog) = parse(text) {
                for decl in &prog.declarations {
                    match decl {
                        Declaration::Function(f) => {
                            if let Some(n) = &f.name {
                                items.push(CompletionItem {
                                    label: n.as_name().unwrap_or("fn").to_string(),
                                    kind: Some(3), // Function
                                    detail: Some("Function definition".to_string()),
                                    documentation: None,
                                });
                            }
                        }
                        Declaration::TypeDef(t) => {
                            if let Some(n) = t.name.as_name() {
                                items.push(CompletionItem {
                                    label: n.to_string(),
                                    kind: Some(22), // Type
                                    detail: Some("Type definition".to_string()),
                                    documentation: None,
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        items
    }

    pub fn document_symbols(&self, uri: &str) -> Vec<DocumentSymbol> {
        let mut symbols = Vec::new();
        let text = match self.documents.get(uri) {
            Some(t) => t,
            None => return symbols,
        };

        if let Ok(prog) = parse(text) {
            for decl in &prog.declarations {
                match decl {
                    Declaration::Function(f) => {
                        let name = f.name.as_ref().and_then(|n| n.as_name()).unwrap_or("fn").to_string();
                        let range = span_to_range(text, f.span);
                        symbols.push(DocumentSymbol {
                            name,
                            detail: Some("Function".to_string()),
                            kind: 12, // Function
                            range: range.clone(),
                            selection_range: range,
                        });
                    }
                    Declaration::TypeDef(t) => {
                        let name = t.name.as_name().unwrap_or("Type").to_string();
                        let range = span_to_range(text, t.span);
                        symbols.push(DocumentSymbol {
                            name,
                            detail: Some("Type Definition".to_string()),
                            kind: 23, // Struct
                            range: range.clone(),
                            selection_range: range,
                        });
                    }
                    Declaration::Trait(tr) => {
                        let name = tr.name.as_name().unwrap_or("Trait").to_string();
                        let range = span_to_range(text, tr.span);
                        symbols.push(DocumentSymbol {
                            name,
                            detail: Some("Trait".to_string()),
                            kind: 5, // Class
                            range: range.clone(),
                            selection_range: range,
                        });
                    }
                    _ => {}
                }
            }
        }

        symbols
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Diagnostic Computation & Position Conversion
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub fn compute_diagnostics(source: &str) -> Vec<Diagnostic> {
    let mut diags = Vec::new();

    match parse(source) {
        Ok(prog) => {
            // Run borrow checker
            let borrow_errors = BorrowChecker::check_program(source, &prog);
            for b in borrow_errors {
                diags.push(Diagnostic {
                    range: span_to_range(source, b.span),
                    severity: Some(1), // Error
                    code: Some("E0001".to_string()),
                    source: Some("mantis-borrow-checker".to_string()),
                    message: b.format(source),
                });
            }
        }
        Err(err) => match err {
            MantisError::Lex(e) => {
                diags.push(Diagnostic {
                    range: span_to_range(source, e.span),
                    severity: Some(1),
                    code: Some("E0002".to_string()),
                    source: Some("mantis-lexer".to_string()),
                    message: format!("unexpected token '{}'", e.slice),
                });
            }
            MantisError::Parse(e) => {
                diags.push(Diagnostic {
                    range: span_to_range(source, e.span),
                    severity: Some(1),
                    code: Some("E0003".to_string()),
                    source: Some("mantis-parser".to_string()),
                    message: format!("parse error: {}", e.message),
                });
            }
        },
    }

    diags
}

pub fn span_to_range(source: &str, span: Span) -> Range {
    let start_pos = offset_to_position(source, span.start);
    let end_pos = offset_to_position(source, span.end);
    Range {
        start: start_pos,
        end: end_pos,
    }
}

pub fn offset_to_position(source: &str, offset: usize) -> Position {
    let mut line = 0;
    let mut col = 0;
    for (i, ch) in source.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    Position {
        line,
        character: col,
    }
}

pub fn position_to_offset(source: &str, pos: Position) -> Option<usize> {
    let mut current_line = 0;
    let mut current_col = 0;
    for (i, ch) in source.char_indices() {
        if current_line == pos.line && current_col == pos.character {
            return Some(i);
        }
        if ch == '\n' {
            if current_line == pos.line {
                return Some(i);
            }
            current_line += 1;
            current_col = 0;
        } else {
            current_col += 1;
        }
    }
    None
}

fn find_symbol_at_offset(prog: &Program, offset: usize) -> Option<String> {
    for decl in &prog.declarations {
        if let Declaration::Function(f) = decl {
            if f.span.start <= offset && offset <= f.span.end {
                let name = f.name.as_ref().and_then(|n| n.as_name()).unwrap_or("fn");
                return Some(format!("fn {}(...)", name));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lsp_diagnostics() {
        let source = "fn main() { let x = 10; let f = [@mut x] () i64 { return x; }; }";
        let diags = compute_diagnostics(source);
        assert!(!diags.is_empty());
        assert!(diags[0].message.contains("cannot capture immutable variable"));
    }
}
