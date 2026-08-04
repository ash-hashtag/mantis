use std::{ops::Deref, rc::Rc};

use crate::utils::{bi_directional_iterator::BiDerectionalIterator, rc_str::RcStr, rc_vec::RcVec};

use super::types::Token;

pub struct CodeParser {
    inner: RcStr,
}

const BREAKPOINTS: &str = "{}()[],.+=-*/$#|&;\"'";

#[derive(Debug)]
pub enum CodeWord {
    Symbol(RcStr),
    Word(RcStr),
}

impl CodeParser {
    pub fn new(s: RcStr) -> Self {
        // let inner: Rc<[char]> = s.chars().collect();
        // let inner = RcVec::from(inner);
        Self { inner: s }
    }

    pub fn parse(&self) -> Option<Vec<CodeWord>> {
        let mut tokens = Vec::<CodeWord>::with_capacity(self.inner.len() * 4);

        let mut parsed_till = 0;
        let mut inside_string_literal = false;
        let mut prev_char = None;
        for (idx, c) in self.inner.char_indices() {
            if inside_string_literal {
                if c == '"' && prev_char != Some('\\') {
                    inside_string_literal = false;
                    if parsed_till < idx {
                        let word = self.inner.clone().slice(parsed_till..idx)?.trim();
                        tokens.push(CodeWord::Word(word));
                    }
                    let symbol = self.inner.clone().slice(idx..idx + 1)?;
                    tokens.push(CodeWord::Symbol(symbol));
                    parsed_till = idx + 1;
                }
            } else {
                let is_breakpoint = BREAKPOINTS.chars().any(|x| x == c);
                if is_breakpoint {
                    if c == '"' {
                        inside_string_literal = true;
                    }
                    if parsed_till < idx {
                        let word = self.inner.clone().slice(parsed_till..idx)?.trim();
                        tokens.push(CodeWord::Word(word));
                    }
                    let symbol = self.inner.clone().slice(idx..idx + 1)?;
                    tokens.push(CodeWord::Symbol(symbol));
                    parsed_till = idx + 1;
                } else if c.is_whitespace() {
                    if parsed_till < idx {
                        let word = self.inner.clone().slice(parsed_till..idx)?.trim();
                        tokens.push(CodeWord::Word(word));
                    }
                    parsed_till = idx + 1;
                }
                // if is_breakpoint || c.is_whitespace() {
                //     print!("PI: {parsed_till} IDX: {idx} BP: {:x?}\n", c);
                //     if parsed_till < idx {
                //         let word = self.inner.clone().slice(parsed_till..idx)?.trim();
                //         tokens.push(CodeWord::Word(word));
                //     }
                //     if !c.is_whitespace() {
                //         let symbol = self.inner.clone().slice(idx..idx + 1)?;
                //         tokens.push(CodeWord::Symbol(symbol));
                //     }
                //     parsed_till = idx + 1;
                // }
            }
            prev_char = Some(c);
        }

        Some(tokens)
    }
}
