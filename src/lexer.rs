#[allow(dead_code, clippy::doc_lazy_continuation)]
mod dfa;
pub mod token;

use token::{Token, TokenKind, keyword, operator};

/// 词法分析器。
///
/// `forward` 使用字节偏移，但每次按照 char 的 UTF-8 长度移动，
/// 因此可以安全切片，并让 Span 直接对应源文件的字节位置。
pub struct Lexer {
    source: String,
    forward: usize,
}

impl Lexer {
    /// 创建 Lexer。游标从源文件第一个字节开始。
    pub fn new(source: String) -> Self {
        Self { source, forward: 0 }
    }

    /// 扫描并消费下一个 Token。
    ///
    /// 扫描顺序是：忽略内容、标识符/关键字、数字、字符串、运算符。
    /// 无法归类的输入会产生 Illegal，而不是让 Lexer panic。
    pub fn next_token(&mut self) -> Token {
        // 空白和注释不参与语法分析，读取 token 前统一跳过。
        self.skip_ignored();
        let start = self.forward;
        let Some(ch) = self.current() else {
            return Token::new(TokenKind::Eof, "", start, start);
        };

        if ch.is_ascii_alphabetic() || ch == '_' {
            // 关键字先按标识符扫描，完成后再由 keyword 进行分类。
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
            // 小数点后必须有数字，避免把成员访问误判成浮点数。
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
            // Token 保留原始字符串；转义字符在构造 AST 时再转换。
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
            // 双字符运算符使用最长匹配。
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
        // 保存并恢复游标，使预读不会改变 Lexer 状态。
        let saved = self.forward;
        let token = self.next_token();
        self.forward = saved;
        token
    }

    /// 返回游标所在字符；到达源文件末尾时返回 None。
    fn current(&self) -> Option<char> {
        self.source[self.forward..].chars().next()
    }

    /// 返回当前字符之后的一个字符，不移动游标。
    /// 目前用于判断 `.` 是小数点还是独立标点。
    fn peek_char(&self) -> Option<char> {
        let mut chars = self.source[self.forward..].chars();
        chars.next()?;
        chars.next()
    }

    /// 消费一个完整 Unicode 标量值。
    ///
    /// forward 是字节偏移，必须增加 len_utf8，而不是固定增加 1。
    fn advance(&mut self) {
        if let Some(ch) = self.current() {
            self.forward += ch.len_utf8();
        }
    }

    /// 跳过空白和 `//` 单行注释。
    ///
    /// 使用循环是因为一段注释后面可能继续跟空白或下一段注释。
    fn skip_ignored(&mut self) {
        loop {
            while self.current().is_some_and(char::is_whitespace) {
                self.advance();
            }
            if self.source[self.forward..].starts_with("//") {
                // 换行由下一轮的空白处理消费。
                while self.current().is_some_and(|c| c != '\n') {
                    self.advance();
                }
            } else {
                break;
            }
        }
    }

    /// 数字后出现字母等非法连续字符时，一直读到下一个分隔符。
    /// 这样 `123abc` 会生成一个完整 Illegal Token，而不是误拆成两个合法 Token。
    fn consume_until_delimiter(&mut self) {
        while self
            .current()
            .is_some_and(|c| !c.is_whitespace() && !"+-*/%!=<>&|^~(){}[],.:;?\"".contains(c))
        {
            self.advance();
        }
    }

    /// 用起点和当前游标截取原始文本，并创建带 Span 的 Token。
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
