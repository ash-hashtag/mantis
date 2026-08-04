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
    #[test]
    fn test_precedence() {
        let src = r#"
            fn test() {
                let a = 1 + 2 * 3;
                let b = 1 * 2 + 3;
                let c = -1 + 2;
                let d = 1 < 2 == 3 < 4;
                let e = 1 & 2 == 3 | 4;
            }
        "#;

        let result = parse(src).expect("Failed to parse precedence test");
        let decl = &result.declarations[0];
        
        let ast::Declaration::Function(fn_decl) = decl else {
            panic!("Expected function declaration");
        };
        let body = fn_decl.body.as_ref().expect("Expected function body");

        // Helper to extract let value
        macro_rules! get_let_val {
            ($item:expr) => {
                match $item {
                    ast::BlockItem::Statement(ast::Statement::Let { value, .. }) => value,
                    _ => panic!("Expected Let statement, got: {:?}", $item),
                }
            };
        }

        // 1 + 2 * 3 => 1 + (2 * 3)
        let a_val = get_let_val!(&body.items[0]);
        if let ast::Expr::Binary { op: ast::BinOp::Add, rhs, .. } = a_val {
            if let ast::Expr::Binary { op: ast::BinOp::Mul, .. } = &**rhs {
                // ok
            } else { panic!("a_val rhs is not Mul: {:?}", rhs) }
        } else { panic!("a_val is not Add: {:?}", a_val) }

        // 1 * 2 + 3 => (1 * 2) + 3
        let b_val = get_let_val!(&body.items[1]);
        if let ast::Expr::Binary { op: ast::BinOp::Add, lhs, .. } = b_val {
            if let ast::Expr::Binary { op: ast::BinOp::Mul, .. } = &**lhs {
                // ok
            } else { panic!("b_val lhs is not Mul: {:?}", lhs) }
        } else { panic!("b_val is not Add: {:?}", b_val) }

        // -1 + 2 => (-1) + 2
        let c_val = get_let_val!(&body.items[2]);
        if let ast::Expr::Binary { op: ast::BinOp::Add, lhs, .. } = c_val {
            if let ast::Expr::Unary { op: ast::UnaryOp::Neg, .. } = &**lhs {
                // ok
            } else { panic!("c_val lhs is not Neg: {:?}", lhs) }
        } else { panic!("c_val is not Add: {:?}", c_val) }

        // 1 < 2 == 3 < 4 => (1 < 2) == (3 < 4)
        let d_val = get_let_val!(&body.items[3]);
        if let ast::Expr::Binary { op: ast::BinOp::Eq, lhs, rhs, .. } = d_val {
            if let ast::Expr::Binary { op: ast::BinOp::Lt, .. } = &**lhs {
                if let ast::Expr::Binary { op: ast::BinOp::Lt, .. } = &**rhs {
                    // ok
                } else { panic!("d_val rhs is not Lt: {:?}", rhs) }
            } else { panic!("d_val lhs is not Lt: {:?}", lhs) }
        } else { panic!("d_val is not Eq: {:?}", d_val) }

        // 1 & 2 == 3 | 4 => (1 & 2) == (3 | 4)
        let e_val = get_let_val!(&body.items[4]);
        if let ast::Expr::Binary { op: ast::BinOp::Eq, lhs, rhs, .. } = e_val {
            if let ast::Expr::Binary { op: ast::BinOp::BitAnd, .. } = &**lhs {
                if let ast::Expr::Binary { op: ast::BinOp::BitOr, .. } = &**rhs {
                    // ok
                } else { panic!("e_val rhs is not BitOr: {:?}", rhs) }
            } else { panic!("e_val lhs is not BitAnd: {:?}", lhs) }
        } else { panic!("e_val is not Eq: {:?}", e_val) }
    }
}
