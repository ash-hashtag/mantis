use cranelift::codegen::ir::{types, Type};
use logos::{Lexer, Logos};

use crate::frontend::tokens::MantisLexerTokens;

use super::tokens::{
    ConstLiteral, Expression, FunctionDeclaration, Keyword, MsVariable, Node, Token, VariableType,
};

pub fn read_to_tokens(input: String) -> Vec<FunctionDeclaration> {
    let mut lexer = MantisLexerTokens::lexer(&input);

    let mut functions = Vec::new();

    while let Some(token) = lexer.next() {
        match token {
            Ok(MantisLexerTokens::FunctionDecl) => {
                let decl = parse_fn_declaration(&mut lexer);
                functions.push(decl);
            }

            // Ok(MantisLexerTokens::Extern) => {
            //     let decl = parse_fn_declaration(&mut lexer);
            //     functions.push(decl)
            // }
            _ => log::error!("Unsupported token {:?} {}\n", lexer.span(), lexer.slice()),
        };
    }

    return functions;
}

pub fn map_type_to_native(val: &str) -> Option<VariableType> {
    let t = match val {
        "i32" => VariableType::Native(types::I32),
        "i64" => VariableType::Native(types::I64),
        "f64" => VariableType::Native(types::F64),
        "f32" => VariableType::Native(types::F32),
        _ => {
            return None;
        }
    };
    Some(t)
}

pub fn parse_fn_declaration(lexer: &mut Lexer<'_, MantisLexerTokens>) -> FunctionDeclaration {
    let mut fn_name = String::new();
    let mut arguments = Vec::new();
    let mut is_external = false;
    let mut fn_scope = None;
    let mut return_type = None;
    while let Some(token) = lexer.next() {
        match token {
            Ok(MantisLexerTokens::Word(value)) => {
                if fn_name.is_empty() {
                    fn_name = value;
                } else {
                    return_type = Some(map_type_to_native(&value).expect("Invalid Return Type"));
                }
            }
            Ok(MantisLexerTokens::BracketOpen) => {
                arguments = parse_fn_declared_arguments(lexer);
            }

            Ok(MantisLexerTokens::Extern) => {
                is_external = true;
            }
            Ok(MantisLexerTokens::SemiColon) => {
                break;
            }

            Ok(MantisLexerTokens::BraceOpen) => {
                if !is_external {
                    fn_scope = Some(parse_scope(lexer));
                    break;
                } else {
                    panic!("External Function can't have function scope");
                }
            }
            _ => log::error!("Unsupported token {:?} {}\n", lexer.span(), lexer.slice()),
        }
    }

    return FunctionDeclaration {
        name: fn_name,
        arguments,
        body: fn_scope,
        return_type,
    };
}

pub fn parse_scope(lexer: &mut Lexer<'_, MantisLexerTokens>) -> Vec<Expression> {
    let mut expressions = Vec::<Expression>::new();

    loop {
        let expression = parse_expression(lexer);
        if matches!(expression, Expression::Nil) {
            break;
        }
        expressions.push(expression);
    }

    expressions
}

pub fn parse_expression(lexer: &mut Lexer<'_, MantisLexerTokens>) -> Expression {
    let mut expression = Expression::Nil;
    let mut tokens = Vec::new();
    while let Some(token) = lexer.next() {
        match token {
            Ok(MantisLexerTokens::SemiColon) => {
                expression = parse_line_expression(tokens);
                break;
            }
            Ok(MantisLexerTokens::BraceClose) => {
                expression = parse_line_expression(tokens);
                break;
            }
            Ok(token) => {
                tokens.push(token);
            }
            _ => panic!("Unsupported token {:?}, {}\n", lexer.span(), lexer.slice()),
        }
    }

    expression
}

pub fn parse_let_expression(tokens: &[MantisLexerTokens]) -> Option<Expression> {
    if let (Some(MantisLexerTokens::Word(var_name)), Some(MantisLexerTokens::Assign)) =
        (tokens.get(0), tokens.get(1))
    {
        return Some(Expression::Declare(
            MsVariable::new(var_name, VariableType::Native(types::I64)),
            Node::parse(&tokens[2..]).unwrap(),
        ));
    }

    None
}

pub fn parse_line_expression(tokens: Vec<MantisLexerTokens>) -> Expression {
    if let Some(token) = tokens.first() {
        match token {
            MantisLexerTokens::Let => return parse_let_expression(&tokens[1..]).unwrap(),
            MantisLexerTokens::Return => {
                return Expression::Return(Node::parse(&tokens[1..]).unwrap());
            }
            _ => {
                return Expression::Operation(Node::parse(&tokens).unwrap());
            }
        }
    } else {
        return Expression::Nil;
    }
    panic!("Invalid expression not supported");
}

pub fn parse_fn_call_args(tokens: &[MantisLexerTokens]) -> Vec<Node> {
    let mut args = Vec::new();

    for chunk in tokens.split(|x| *x == MantisLexerTokens::Comma) {
        args.push(Node::parse(chunk).unwrap());
    }

    return args;
}

pub fn parse_fn_declared_arguments(tokens: &mut Lexer<MantisLexerTokens>) -> Vec<MsVariable> {
    let mut variables = Vec::new();
    let mut next_is_type = false;
    let mut next_is_variable = true;

    let mut lexer = tokens;

    while let Some(Ok(token)) = lexer.next() {
        match token {
            MantisLexerTokens::Word(value) => {
                if next_is_variable {
                    let variable = MsVariable::new(value, VariableType::Native(types::I64));
                    variables.push(variable);
                    next_is_variable = false;
                } else if next_is_type {
                    variables.last_mut().unwrap().var_type = VariableType::Native(types::I64);
                    next_is_type = false;
                }
            }

            MantisLexerTokens::Colon => {
                next_is_type = true;
            }
            MantisLexerTokens::Comma => {
                next_is_variable = true;
            }

            MantisLexerTokens::BracketClose => {
                break;
            }
            _ => log::error!("Unsupported token {:?}\n", token),
        }
    }

    return variables;
}
