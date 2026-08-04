use std::char;

use crate::utils::rc_str::RcStr;

#[derive(Debug, Clone)]
pub enum NonTerminal {
    BlockOpen,
    BlockClose,
    ArgumentOpen,
    ArgumentClose,
    StatementEnd,
    Macro,
}
impl NonTerminal {
    pub fn from_str(s: &str) -> Option<Self> {
        use NonTerminal::*;
        Some(match s {
            "{" => BlockOpen,
            "}" => BlockClose,
            "(" => ArgumentOpen,
            ")" => ArgumentClose,
            "#" => Macro,
            ";" => StatementEnd,
            _ => return None,
        })
    }
    pub fn from_char(s: char) -> Option<Self> {
        use NonTerminal::*;
        Some(match s {
            '{' => BlockOpen,
            '}' => BlockClose,
            '(' => ArgumentOpen,
            ')' => ArgumentClose,
            '#' => Macro,
            ';' => StatementEnd,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone)]
pub enum Operator {
    Assign,
    LessThanOrEqual,
    GreaterThanOrEqual,
    NotEqual,
    Equal,
    And,
    Or,
    LessThan,
    GreaterThan,
    Add,
    Sub,
    Mul,
    Div,
}
impl Operator {
    pub fn from_str(s: &str) -> Option<Self> {
        use Operator::*;
        Some(match s {
            "+" => Add,
            "-" => Sub,
            "/" => Div,
            "*" => Mul,
            ">" => GreaterThan,
            "<" => LessThan,
            ">=" => GreaterThanOrEqual,
            "<=" => LessThanOrEqual,
            "==" => Equal,
            "=" => Assign,
            "||" => Or,
            "&&" => And,
            "!=" => NotEqual,

            _ => return None,
        })
    }
}

#[derive(Debug, Clone)]
pub enum Keyword {
    Let,
    Mut,
    As,
    Fn,
    Pub,
    Struct,
    Return,
}

impl Keyword {
    pub fn from_str(s: &str) -> Option<Self> {
        use Keyword::*;
        Some(match s {
            "let" => Let,
            "mut" => Mut,
            "as" => As,
            "fn" => Fn,
            "return" => Return,
            "pub" => Pub,
            "struct" => Struct,
            _ => return None,
        })
    }
}
#[derive(Debug, Clone)]
pub enum NumberType {
    Integer(String),
    Float(String),
}
impl NumberType {
    pub fn from_str(s: &str) -> Option<Self> {
        let mut found_decimal_points = 0;
        let mut digits_found = 0;
        let mut chars = s.chars();
        let c = chars.next()?;
        let is_negative = c == '-';
        for c in s.chars() {
            if c == '.' {
                found_decimal_points += 1;
            } else {
                if !c.is_numeric() {
                    return None;
                }
                digits_found += 0;
            }
        }
        if digits_found == 0 {
            return None;
        }

        if found_decimal_points != 0 {
            Some(Self::Float(s.into()))
        } else {
            Some(Self::Integer(s.into()))
        }
    }
}

#[derive(Debug, Clone)]
pub enum Token {
    Keyword(Keyword),
    Number(NumberType),
    Symbol(NonTerminal),
    Operator(Operator),
    Word(RcStr),
    StringLiteral(RcStr),
    Unknown(RcStr),
}

impl Token {
    pub fn from_str(s: &str) -> Self {
        if let Some(val) = Keyword::from_str(s) {
            return (Self::Keyword(val));
        }
        if let Some(val) = NumberType::from_str(s) {
            return (Self::Number(val));
        }
        if let Some(val) = NonTerminal::from_str(s) {
            return (Self::Symbol(val));
        }
        if let Some(val) = Operator::from_str(s) {
            return (Self::Operator(val));
        } else {
            return (Self::Unknown(s.to_string().into()));
        }
    }
}
