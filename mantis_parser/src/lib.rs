pub mod ast;
pub mod parser;
pub mod token;

use ast::Program;
use parser::{ParseError, Parser};
use token::{tokenize, LexError};

/// Parse a Mantis source file into a `Program` AST.
pub fn parse(source: &str) -> Result<Program, MantisError> {
    let tokens = tokenize(source).map_err(MantisError::Lex)?;
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().map_err(MantisError::Parse)?;
    Ok(program)
}

#[derive(Debug)]
pub enum MantisError {
    Lex(LexError),
    Parse(ParseError),
}

impl std::fmt::Display for MantisError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MantisError::Lex(e) => write!(f, "lex error: {}", e),
            MantisError::Parse(e) => write!(f, "parse error: {}", e),
        }
    }
}

impl std::error::Error for MantisError {}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Tests
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_main_latest() {
        let src = std::fs::read_to_string("../example/main-latest.ms")
            .expect("cannot read main-latest.ms");
        let result = parse(&src);
        match &result {
            Ok(prog) => {
                eprintln!("{:#?}", prog);
                assert!(
                    prog.declarations.len() > 0,
                    "expected declarations, got 0"
                );
            }
            Err(e) => {
                panic!("parse failed: {}", e);
            }
        }
    }

    #[test]
    fn test_parse_main() {
        let src = std::fs::read_to_string("../example/main.ms")
            .expect("cannot read main.ms");
        let result = parse(&src);
        match &result {
            Ok(prog) => {
                eprintln!("{:#?}", prog);
                assert!(
                    prog.declarations.len() > 0,
                    "expected declarations, got 0"
                );
            }
            Err(e) => {
                panic!("parse failed: {}", e);
            }
        }
    }



    #[test]
    fn test_expressions() {
        let cases = vec![
            "fn test() { let x = 1 + 2 * 3; }",
            "fn test() { let y = a.b.c; }",
            "fn test() { foo(a, b, c); }",
            "fn test() { let z = x as i64; }",
            "fn test() { a = 10; }",
            "fn test() { let v = [1, 2, 3]; }",
        ];
        for src in cases {
            let result = parse(src);
            assert!(result.is_ok(), "failed to parse: {} — {:?}", src, result.err());
        }
    }

    #[test]
    fn test_type_parsing() {
        let src = r#"
            type Pair[T, U] = struct { first T, second U }
            type Alias = i64;
            type Result[T, E] = enum { Ok(T), Err(E) }
        "#;
        let result = parse(src);
        assert!(result.is_ok(), "failed: {:?}", result.err());
        let prog = result.unwrap();
        assert_eq!(prog.declarations.len(), 3);
    }
}
