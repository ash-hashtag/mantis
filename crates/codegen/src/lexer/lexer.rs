use crate::utils::rc_str::RcStr;

use super::types::Token;

pub struct Lexer {
    code: RcStr,
}
impl Lexer {
    pub fn new(code: RcStr) -> Self {
        Self { code }
    }

    pub fn get_tokens(&self) -> Vec<Token> {
        let s = self.code.as_str();
        s.split_whitespace().map(Token::from_str).collect()
    }
}
