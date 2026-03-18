use std::fmt;
use std::ops::Range;

use logos::Logos;

/// Byte-offset span in source text
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn from_range(range: Range<usize>) -> Self {
        Self {
            start: range.start,
            end: range.end,
        }
    }

    pub fn merge(self, other: Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        &source[self.start..self.end]
    }
}

impl fmt::Debug for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

#[derive(Logos, Clone, Debug, PartialEq)]
#[logos(skip r"[ \t\r\n\f]+")]
pub enum Token {
    // ── Keywords ──────────────────────────────────────────────
    #[token("fn")]
    Fn,
    #[token("let")]
    Let,
    #[token("mut")]
    Mut,
    #[token("return")]
    Return,
    #[token("if")]
    If,
    #[token("elif")]
    Elif,
    #[token("else")]
    Else,
    #[token("loop")]
    Loop,
    #[token("break")]
    Break,
    #[token("continue")]
    Continue,
    #[token("match")]
    Match,
    #[token("as")]
    As,
    #[token("type")]
    Type,
    #[token("struct")]
    Struct,
    #[token("enum")]
    Enum,
    #[token("trait")]
    Trait,
    #[token("impl")]
    Impl,
    #[token("for")]
    For,
    #[token("extern")]
    Extern,
    #[token("import")]
    Import,
    #[token("use")]
    Use,

    // ── Literals ──────────────────────────────────────────────
    #[token("true", |_| true)]
    #[token("false", |_| false)]
    Bool(bool),

    #[regex(r"[0-9]+", |lex| lex.slice().parse::<i64>().unwrap(), priority = 2)]
    Int(i64),

    #[regex(r"[0-9]+\.[0-9]+", |lex| lex.slice().parse::<f64>().unwrap())]
    Float(f64),

    #[regex(r#""([^"\\]|\\["\\bnfrt0]|\\u[a-fA-F0-9]{4})*""#, parse_string)]
    String(std::string::String),

    #[regex(r"'[^']'", |lex| {
        let s = lex.slice();
        s.chars().nth(1).unwrap()
    })]
    #[regex(r"'\\[nrt0\\']'", |lex| {
        let s = lex.slice();
        match &s[1..s.len()-1] {
            "\\n" => '\n',
            "\\r" => '\r',
            "\\t" => '\t',
            "\\0" => '\0',
            "\\\\" => '\\',
            "\\'" => '\'',
            _ => unreachable!(),
        }
    })]
    Char(char),

    // ── Identifiers ──────────────────────────────────────────
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_owned(), priority = 1)]
    Ident(std::string::String),

    #[regex(r"#[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice()[1..].to_owned())]
    CompilerFn(std::string::String),

    // ── Operators (multi-char first) ─────────────────────────
    #[token("@=")]
    AtAssign,
    #[token("==")]
    EqEq,
    #[token("!=")]
    NotEq,
    #[token(">=")]
    GtEq,
    #[token("<=")]
    LtEq,

    #[token("=")]
    Eq,
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("%")]
    Percent,
    #[token(">")]
    Gt,
    #[token("<")]
    Lt,
    #[token("@")]
    At,
    #[token("?")]
    Question,
    #[token("&")]
    Amp,

    // ── Delimiters ───────────────────────────────────────────
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,

    // ── Punctuation ──────────────────────────────────────────
    #[token(".")]
    Dot,
    #[token(",")]
    Comma,
    #[token(":")]
    Colon,
    #[token(";")]
    Semi,

    // ── Comments (skipped) ───────────────────────────────────
    #[regex(r"//[^\n]*", logos::skip)]
    Comment,
}

fn parse_string(lex: &mut logos::Lexer<'_, Token>) -> std::string::String {
    let s = lex.slice();
    // Strip surrounding quotes
    let inner = &s[1..s.len() - 1];
    let mut result = std::string::String::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => result.push('\n'),
                Some('r') => result.push('\r'),
                Some('t') => result.push('\t'),
                Some('0') => result.push('\0'),
                Some('\\') => result.push('\\'),
                Some('"') => result.push('"'),
                Some('b') => result.push('\u{0008}'),
                Some('f') => result.push('\u{000C}'),
                Some(other) => {
                    result.push('\\');
                    result.push(other);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// A token with its span in the source text.
#[derive(Clone, Debug)]
pub struct SpannedToken {
    pub token: Token,
    pub span: Span,
}

impl SpannedToken {
    pub fn new(token: Token, span: Span) -> Self {
        Self { token, span }
    }
}

/// Tokenize source into a vec of spanned tokens.
pub fn tokenize(source: &str) -> Result<Vec<SpannedToken>, LexError> {
    let mut lexer = Token::lexer(source);
    let mut tokens = Vec::new();
    while let Some(result) = lexer.next() {
        match result {
            Ok(token) => {
                let span = Span::from_range(lexer.span());
                tokens.push(SpannedToken::new(token, span));
            }
            Err(_) => {
                let span = Span::from_range(lexer.span());
                return Err(LexError {
                    span,
                    slice: source[span.start..span.end].to_owned(),
                });
            }
        }
    }
    Ok(tokens)
}

#[derive(Debug)]
pub struct LexError {
    pub span: Span,
    pub slice: std::string::String,
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unexpected token '{}' at {}",
            self.slice, self.span
        )
    }
}

impl std::error::Error for LexError {}
