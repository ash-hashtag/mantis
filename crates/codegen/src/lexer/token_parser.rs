use std::ops::Deref;

use crate::utils::{bi_directional_iterator::BiDerectionalIterator, rc_str::RcStr, rc_vec::RcVec};

use super::{code_parser::CodeWord, types::Token};

pub struct TokenParser {
    code_words: RcVec<CodeWord>,
}

impl TokenParser {
    pub fn new(code_words: RcVec<CodeWord>) -> Self {
        Self { code_words }
    }

    pub fn parse(&self) -> Vec<Token> {
        let mut tokens = Vec::<Token>::with_capacity(self.code_words.len());
        let iterator = BiDerectionalIterator::new(self.code_words.clone(), 0);

        for c in iterator.into_iter() {
            match &*c {
                CodeWord::Symbol(symbol) => {}
                CodeWord::Word(word) => {}
            };
        }

        return tokens;
    }
}
