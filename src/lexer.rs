#[allow(dead_code, clippy::doc_lazy_continuation)]
mod dfa;
pub mod token;

use token::{Token, TokenKind, keyword, operator};

/// A byte-offset lexer. Source slices are only taken at UTF-8 boundaries.
pub struct Lexer {
    source: String,
    forward: usize,
}

impl Lexer {
    pub fn new(source: String) -> Self {
        Self { source, forward: 0 }
    }

    pub fn next_token(&mut self) -> Token {
        self.skip_ignored();
        let start = self.forward;
        let Some(ch) = self.current() else {
            return Token::new(TokenKind::Eof, "", start, start);
        };

        if ch.is_ascii_alphabetic() || ch == '_' {
            self.advance();
            while self
                .current()
                .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
            {
                self.advance();
            }
            return self.make_token(keyword(&self.source[start..self.forward]), start);
        }

        if ch.is_ascii_digit() {
            self.advance();
            while self.current().is_some_and(|c| c.is_ascii_digit()) {
                self.advance();
            }
            let mut kind = TokenKind::Integer;
            if self.current() == Some('.') && self.peek_char().is_some_and(|c| c.is_ascii_digit()) {
                kind = TokenKind::Float;
                self.advance();
                while self.current().is_some_and(|c| c.is_ascii_digit()) {
                    self.advance();
                }
            }
            if self
                .current()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '.')
            {
                self.consume_until_delimiter();
                return self.make_token(TokenKind::Illegal, start);
            }
            return self.make_token(kind, start);
        }

        if ch == '"' {
            self.advance();
            let mut valid = true;
            while let Some(c) = self.current() {
                match c {
                    '"' => {
                        self.advance();
                        return self.make_token(
                            if valid {
                                TokenKind::String
                            } else {
                                TokenKind::Illegal
                            },
                            start,
                        );
                    }
                    '\\' => {
                        self.advance();
                        match self.current() {
                            Some('"' | '\\' | 'n' | 'r' | 't' | '0') => self.advance(),
                            Some(_) => {
                                valid = false;
                                self.advance();
                            }
                            None => break,
                        }
                    }
                    '\n' | '\r' => {
                        self.advance();
                        return self.make_token(TokenKind::Illegal, start);
                    }
                    _ => self.advance(),
                }
            }
            return self.make_token(TokenKind::Illegal, start);
        }

        self.advance();
        if let Some(next) = self.current() {
            let can_pair = matches!(
                (ch, next),
                ('+', '=')
                    | ('-', '=')
                    | ('-', '>')
                    | ('*', '=')
                    | ('/', '=')
                    | ('%', '=')
                    | ('=', '=')
                    | ('=', '>')
                    | ('!', '=')
                    | ('<', '=')
                    | ('<', '<')
                    | ('>', '=')
                    | ('>', '>')
                    | ('&', '&')
                    | ('|', '|')
            );
            if can_pair {
                self.advance();
            }
        }
        self.make_token(operator(&self.source[start..self.forward]), start)
    }

    #[allow(dead_code)]
    pub fn peek(&mut self) -> Token {
        let saved = self.forward;
        let token = self.next_token();
        self.forward = saved;
        token
    }

    fn current(&self) -> Option<char> {
        self.source[self.forward..].chars().next()
    }

    fn peek_char(&self) -> Option<char> {
        let mut chars = self.source[self.forward..].chars();
        chars.next()?;
        chars.next()
    }

    fn advance(&mut self) {
        if let Some(ch) = self.current() {
            self.forward += ch.len_utf8();
        }
    }

    fn skip_ignored(&mut self) {
        loop {
            while self.current().is_some_and(char::is_whitespace) {
                self.advance();
            }
            if self.source[self.forward..].starts_with("//") {
                while self.current().is_some_and(|c| c != '\n') {
                    self.advance();
                }
            } else {
                break;
            }
        }
    }

    fn consume_until_delimiter(&mut self) {
        while self
            .current()
            .is_some_and(|c| !c.is_whitespace() && !"+-*/%!=<>&|^~(){}[],.:;?\"".contains(c))
        {
            self.advance();
        }
    }

    fn make_token(&self, kind: TokenKind, start: usize) -> Token {
        Token::new(kind, &self.source[start..self.forward], start, self.forward)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peek_does_not_consume() {
        let mut lexer = Lexer::new("let a=1;".into());
        assert_eq!(lexer.peek().kind, TokenKind::Let);
        assert_eq!(lexer.peek().kind, TokenKind::Let);
        assert_eq!(lexer.next_token().kind, TokenKind::Let);
    }

    #[test]
    fn eof_does_not_require_sentinel() {
        let mut lexer = Lexer::new("42".into());
        assert_eq!(lexer.next_token().kind, TokenKind::Integer);
        assert_eq!(lexer.next_token().kind, TokenKind::Eof);
    }

    #[test]
    fn string_can_contain_utf8() {
        let mut lexer = Lexer::new("\"你好\"".into());
        assert_eq!(lexer.next_token().kind, TokenKind::String);
        assert_eq!(lexer.next_token().kind, TokenKind::Eof);
    }
}
