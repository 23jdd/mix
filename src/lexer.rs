

pub mod token;
mod dfa;

use std::iter::chain;
use token::Token;
use crate::lexer::dfa::State;
use crate::lexer::dfa::State::{Start};
use crate::lexer::token::{keyword, TokenKind, operator};

pub struct Lexer{
     source:String,
     lexeme_begin:usize,
     forward:usize,
}
impl Lexer {
    pub fn new(source: String) -> Self {
        Self {
            source,
            lexeme_begin: 0,
            forward: 0,
        }
    }
    fn len(&self) -> usize {
        self.source.len()
    }
    pub fn next_token(&mut self) -> Token<'_> {
        let mut state = State::Start;
        let chs: Vec<char> = self.source.chars().collect();
        loop {
            let (pre, next, reconsumed) = state.next(chs[self.forward]);
            state=next;
            if !reconsumed {
                self.forward += 1;
            }
            if self.forward == self.len() - 1 {
                break Token::new(TokenKind::Eof, &self.source[self.lexeme_begin..self.forward], self.lexeme_begin, self.forward);
            }
            if state == State::Start {
                let ident = &self.source[self.lexeme_begin..self.forward];

                let t = match pre {
                    State::Identifier => {
                        Token::new(keyword(ident), ident, self.lexeme_begin, self.forward)
                    }
                    State::Integer => {
                        Token::new(TokenKind::Integer, ident, self.lexeme_begin, self.forward)
                    }
                    State::Float => {
                        Token::new(TokenKind::Float, ident, self.lexeme_begin, self.forward)
                    }
                    State::String => {
                        Token::new(TokenKind::String, ident, self.lexeme_begin, self.forward)
                    }
                    State::MaybeEqual
                    | State::Minus
                    | State::Equal
                    | State::Less
                    | State::Greater
                    | State::AndDoubleSame
                    | State::OrDoubleSame
                    | State::Single => {
                        Token::new(operator(ident), ident, self.lexeme_begin, self.forward)
                    }
                    State::StringEscape | State::FloatDot => {
                        panic!("BUG")
                    }
                    _ => {
                        self.lexeme_begin = self.forward;
                        continue
                    }
                };
                self.lexeme_begin = self.forward;
                break t;
            } else if state == State::Dead {
                break Token::new(TokenKind::Illegal, &self.source[self.lexeme_begin..self.forward], self.lexeme_begin, self.forward);
            }
        }
    }
}