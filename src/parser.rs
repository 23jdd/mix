use std::fmt;

use crate::ast::{Block, Expression, Program, Statement};
use crate::lexer::Lexer;
use crate::lexer::token::{Span, Token, TokenKind};

/// 语法错误以及错误 token 在源文件中的范围。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} at bytes {}..{}",
            self.message, self.span.start, self.span.end
        )
    }
}

impl std::error::Error for ParseError {}

pub struct Parser {
    /// 按需产生 Token；Parser 不需要一次性保存整个 Token 列表。
    lexer: Lexer,
    /// 始终指向尚未消费的下一个 token。
    current: Token,
}

impl Parser {
    pub fn new(mut lexer: Lexer) -> Self {
        // 初始化时预读一个 token，后续判断语法时无需反复调用 peek。
        let current = lexer.next_token();
        Self { lexer, current }
    }

    /// 解析整个源文件，直到遇到 EOF。
    ///
    /// 文法：`program -> statement* EOF`
    /// 每次 `parse_statement` 至少消费一个完整语句，因此循环不会停在原地。
    pub fn parse(&mut self) -> Result<Program, ParseError> {
        let mut statements = Vec::new();
        while !self.at(&TokenKind::Eof) {
            statements.push(self.parse_statement()?);
        }
        Ok(Program { statements })
    }

    /// 根据当前 Token 分派到具体语句解析函数。
    ///
    /// 简单语句（声明、return、break、表达式）负责消费末尾分号；
    /// 函数和控制流语句以代码块结束，不需要额外分号。
    fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        // 语句的第一个 token 决定进入哪个解析分支。
        match &self.current.kind {
            TokenKind::Let => self.parse_variable(true),
            TokenKind::Const => self.parse_variable(false),
            TokenKind::Fn => self.parse_function(),
            TokenKind::If => self.parse_if(),
            TokenKind::While => self.parse_while(),
            TokenKind::For => self.parse_for(),
            TokenKind::Return => self.parse_return(),
            TokenKind::Break => {
                self.advance();
                self.expect(TokenKind::Semicolon, "expected `;` after `break`")?;
                Ok(Statement::Break)
            }
            TokenKind::Continue => {
                self.advance();
                self.expect(TokenKind::Semicolon, "expected `;` after `continue`")?;
                Ok(Statement::Continue)
            }
            TokenKind::LeftBrace => Ok(Statement::Block(self.parse_block()?)),
            TokenKind::Illegal => {
                Err(self.error(format!("illegal token `{}`", self.current.lexeme)))
            }
            _ => {
                let expression = self.parse_expression()?;
                self.expect(TokenKind::Semicolon, "expected `;` after expression")?;
                Ok(Statement::Expression(expression))
            }
        }
    }

    /// 解析变量声明。
    ///
    /// 文法：`variable -> ("let" | "const") Identifier "=" expression ";"`
    /// 调用者已经根据首 Token 决定 mutable 的值，此处从变量名开始继续读取。
    fn parse_variable(&mut self, mutable: bool) -> Result<Statement, ParseError> {
        self.advance();
        let name = self.expect_identifier("expected variable name")?;
        self.expect(TokenKind::Equal, "expected `=` after variable name")?;
        let value = self.parse_expression()?;
        self.expect(
            TokenKind::Semicolon,
            "expected `;` after variable declaration",
        )?;
        Ok(Statement::Variable {
            mutable,
            name,
            value,
        })
    }

    /// 解析具名函数声明。
    ///
    /// 文法：`function -> "fn" Identifier "(" parameters? ")" block`
    /// 参数目前只有名称，没有类型、默认值或可变参数。
    fn parse_function(&mut self) -> Result<Statement, ParseError> {
        self.advance();
        let name = self.expect_identifier("expected function name")?;
        self.expect(TokenKind::LeftParen, "expected `(` after function name")?;
        let mut parameters = Vec::new();
        if !self.at(&TokenKind::RightParen) {
            loop {
                parameters.push(self.expect_identifier("expected parameter name")?);
                if !self.take(&TokenKind::Comma) {
                    break;
                }
            }
        }
        self.expect(TokenKind::RightParen, "expected `)` after parameters")?;
        let body = self.parse_block()?;
        Ok(Statement::Function {
            name,
            parameters,
            body,
        })
    }

    /// 解析 if/else if/else。
    ///
    /// 文法：`if -> "if" expression block ("else" (if | block))?`
    /// 条件外的括号不是必须的；如果写了括号，会由普通分组表达式处理。
    fn parse_if(&mut self) -> Result<Statement, ParseError> {
        self.advance();
        let condition = self.parse_expression()?;
        let then_branch = self.parse_block()?;
        let else_branch = if self.take(&TokenKind::Else) {
            if self.at(&TokenKind::If) {
                // AST 的 else 分支是 Block，因此将 `else if` 包成单语句块。
                Some(vec![self.parse_if()?])
            } else {
                Some(self.parse_block()?)
            }
        } else {
            None
        };
        Ok(Statement::If {
            condition,
            then_branch,
            else_branch,
        })
    }

    /// 解析条件循环。
    ///
    /// 文法：`while -> "while" expression block`
    fn parse_while(&mut self) -> Result<Statement, ParseError> {
        self.advance();
        let condition = self.parse_expression()?;
        let body = self.parse_block()?;
        Ok(Statement::While { condition, body })
    }

    /// 解析遍历循环。
    ///
    /// 文法：`for -> "for" Identifier "in" expression block`
    /// iterable 可以是任意表达式，例如数组、变量或 `range(...)` 调用。
    fn parse_for(&mut self) -> Result<Statement, ParseError> {
        self.advance();
        let name = self.expect_identifier("expected loop variable after `for`")?;
        self.expect(TokenKind::In, "expected `in` after loop variable")?;
        let iterable = self.parse_expression()?;
        let body = self.parse_block()?;
        Ok(Statement::For {
            name,
            iterable,
            body,
        })
    }

    /// 解析有返回值或无返回值的 return。
    ///
    /// 文法：`return -> "return" expression? ";"`
    /// 当前 Token 紧接分号时表示没有返回值。
    fn parse_return(&mut self) -> Result<Statement, ParseError> {
        self.advance();
        let value = if self.at(&TokenKind::Semicolon) {
            None
        } else {
            Some(self.parse_expression()?)
        };
        self.expect(TokenKind::Semicolon, "expected `;` after return value")?;
        Ok(Statement::Return(value))
    }

    /// 解析花括号包围的语句列表。
    ///
    /// 文法：`block -> "{" statement* "}"`
    /// 如果读取到 EOF 仍未遇到右花括号，expect 会报告缺少 `}`。
    fn parse_block(&mut self) -> Result<Block, ParseError> {
        self.expect(TokenKind::LeftBrace, "expected `{`")?;
        let mut statements = Vec::new();
        while !self.at(&TokenKind::RightBrace) && !self.at(&TokenKind::Eof) {
            statements.push(self.parse_statement()?);
        }
        self.expect(TokenKind::RightBrace, "expected `}` after block")?;
        Ok(statements)
    }

    /// 表达式总入口。赋值是当前语言中优先级最低的表达式。
    fn parse_expression(&mut self) -> Result<Expression, ParseError> {
        self.parse_assignment()
    }

    /// 解析普通赋值和复合赋值。
    ///
    /// 文法：`assignment -> binary (assignment_operator assignment)?`
    /// 赋值左侧会额外检查是否为 Identifier、Index 或 Member。
    fn parse_assignment(&mut self) -> Result<Expression, ParseError> {
        // 先解析优先级更高的二元表达式，再判断后面是否为赋值。
        let target = self.parse_binary(1)?;
        if matches!(
            self.current.kind,
            TokenKind::Equal
                | TokenKind::PlusEqual
                | TokenKind::MinusEqual
                | TokenKind::StarEqual
                | TokenKind::SlashEqual
                | TokenKind::PercentEqual
        ) {
            let operator = self.advance().kind;
            if !target.is_assignable() {
                return Err(self.error("invalid assignment target"));
            }
            // 递归解析右侧，使 `a = b = 1` 按 `a = (b = 1)` 右结合。
            let value = self.parse_assignment()?;
            Ok(Expression::Assignment {
                target: Box::new(target),
                operator,
                value: Box::new(value),
            })
        } else {
            Ok(target)
        }
    }

    /// 使用 precedence climbing 算法解析所有左结合二元运算。
    ///
    /// `min_precedence` 表示本层允许接收的最低优先级。解析右操作数时
    /// 使用 `precedence + 1`，所以 `10 - 3 - 2` 会构造成 `(10 - 3) - 2`。
    fn parse_binary(&mut self, min_precedence: u8) -> Result<Expression, ParseError> {
        // precedence climbing：只接收不低于当前门槛的运算符。
        let mut left = self.parse_unary()?;
        while let Some(precedence) = binary_precedence(&self.current.kind) {
            if precedence < min_precedence {
                break;
            }
            let operator = self.advance().kind;
            let right = self.parse_binary(precedence + 1)?;
            left = Expression::Binary {
                left: Box::new(left),
                operator,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    /// 解析前缀一元运算。
    ///
    /// 递归调用自身使连续前缀自然右结合，例如 `!!value` 表示 `!(!value)`。
    fn parse_unary(&mut self) -> Result<Expression, ParseError> {
        if matches!(
            self.current.kind,
            TokenKind::Bang | TokenKind::Minus | TokenKind::Plus | TokenKind::Tilde
        ) {
            let operator = self.advance().kind;
            let operand = self.parse_unary()?;
            Ok(Expression::Unary {
                operator,
                operand: Box::new(operand),
            })
        } else {
            self.parse_postfix()
        }
    }

    /// 在基础表达式后持续读取调用、索引和成员访问。
    ///
    /// 文法：`postfix -> primary (call | index | member)*`
    /// 每次循环以上一轮结果作为新的 object/callee，从而构造完整调用链。
    fn parse_postfix(&mut self) -> Result<Expression, ParseError> {
        let mut expression = self.parse_primary()?;
        // 循环允许连续后缀，例如 `factory()(arg)[0].name`。
        loop {
            if self.take(&TokenKind::LeftParen) {
                let mut arguments = Vec::new();
                if !self.at(&TokenKind::RightParen) {
                    loop {
                        arguments.push(self.parse_expression()?);
                        if !self.take(&TokenKind::Comma) {
                            break;
                        }
                    }
                }
                self.expect(TokenKind::RightParen, "expected `)` after arguments")?;
                expression = Expression::Call {
                    callee: Box::new(expression),
                    arguments,
                };
            } else if self.take(&TokenKind::LeftBracket) {
                let index = self.parse_expression()?;
                self.expect(TokenKind::RightBracket, "expected `]` after index")?;
                expression = Expression::Index {
                    object: Box::new(expression),
                    index: Box::new(index),
                };
            } else if self.take(&TokenKind::Dot) {
                let property = self.expect_identifier("expected property name after `.`")?;
                expression = Expression::Member {
                    object: Box::new(expression),
                    property,
                };
            } else {
                break;
            }
        }
        Ok(expression)
    }

    /// 解析不含运算符的最小表达式单元。
    ///
    /// 包括字面量、标识符、括号分组、数组和对象字面量。这里先消费当前 Token，
    /// 再依据其类型创建 AST；不合法的起始 Token 也会在这里报告。
    fn parse_primary(&mut self) -> Result<Expression, ParseError> {
        let token = self.advance();
        match token.kind {
            TokenKind::Integer => {
                token
                    .lexeme
                    .parse()
                    .map(Expression::Integer)
                    .map_err(|_| ParseError {
                        message: "integer literal is out of range".into(),
                        span: token.span,
                    })
            }
            TokenKind::Float => {
                token
                    .lexeme
                    .parse()
                    .map(Expression::Float)
                    .map_err(|_| ParseError {
                        message: "invalid float literal".into(),
                        span: token.span,
                    })
            }
            TokenKind::String => Ok(Expression::String(unescape_string(&token.lexeme))),
            TokenKind::True => Ok(Expression::Boolean(true)),
            TokenKind::False => Ok(Expression::Boolean(false)),
            TokenKind::Null => Ok(Expression::Null),
            TokenKind::Identifier => Ok(Expression::Identifier(token.lexeme)),
            TokenKind::LeftParen => {
                let expression = self.parse_expression()?;
                self.expect(TokenKind::RightParen, "expected `)` after expression")?;
                Ok(expression)
            }
            TokenKind::LeftBracket => {
                let mut elements = Vec::new();
                if !self.at(&TokenKind::RightBracket) {
                    loop {
                        elements.push(self.parse_expression()?);
                        if !self.take(&TokenKind::Comma) {
                            break;
                        }
                        if self.at(&TokenKind::RightBracket) {
                            break;
                        }
                    }
                }
                self.expect(TokenKind::RightBracket, "expected `]` after array")?;
                Ok(Expression::Array(elements))
            }
            TokenKind::LeftBrace => {
                let mut entries = Vec::new();
                if !self.at(&TokenKind::RightBrace) {
                    loop {
                        let key = match self.advance() {
                            Token {
                                kind: TokenKind::Identifier,
                                lexeme,
                                ..
                            } => lexeme,
                            Token {
                                kind: TokenKind::String,
                                lexeme,
                                ..
                            } => unescape_string(&lexeme),
                            token => {
                                return Err(ParseError {
                                    message: "expected identifier or string as object key".into(),
                                    span: token.span,
                                });
                            }
                        };
                        self.expect(TokenKind::Colon, "expected `:` after object key")?;
                        let value = self.parse_expression()?;
                        entries.push((key, value));
                        if !self.take(&TokenKind::Comma) {
                            break;
                        }
                        if self.at(&TokenKind::RightBrace) {
                            break;
                        }
                    }
                }
                self.expect(TokenKind::RightBrace, "expected `}` after object")?;
                Ok(Expression::Object(entries))
            }
            TokenKind::Illegal => Err(ParseError {
                message: format!("illegal token `{}`", token.lexeme),
                span: token.span,
            }),
            _ => Err(ParseError {
                message: format!("expected expression, found `{}`", token.lexeme),
                span: token.span,
            }),
        }
    }

    /// 只检查当前 Token 类型，不消费它。
    fn at(&self, kind: &TokenKind) -> bool {
        &self.current.kind == kind
    }

    /// 当前 Token 匹配时消费并返回 true，否则保持状态并返回 false。
    /// 适合处理逗号、else 等可选语法。
    fn take(&mut self, kind: &TokenKind) -> bool {
        if self.at(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    /// 要求当前 Token 为指定类型。成功时消费，失败时在当前位置报错。
    fn expect(&mut self, kind: TokenKind, message: &str) -> Result<Token, ParseError> {
        if self.at(&kind) {
            Ok(self.advance())
        } else {
            Err(self.error(message))
        }
    }

    /// expect 的常用特化：读取一个标识符并直接返回它的文本。
    fn expect_identifier(&mut self, message: &str) -> Result<String, ParseError> {
        Ok(self.expect(TokenKind::Identifier, message)?.lexeme)
    }

    fn advance(&mut self) -> Token {
        // 返回旧 token，并把 Lexer 中的下一个 token 放入 current。
        std::mem::replace(&mut self.current, self.lexer.next_token())
    }

    /// 使用尚未消费的当前 Token 范围构造语法错误。
    fn error(&self, message: impl Into<String>) -> ParseError {
        ParseError {
            message: message.into(),
            span: self.current.span,
        }
    }
}

fn binary_precedence(kind: &TokenKind) -> Option<u8> {
    // 数值越大绑定越紧；赋值不在此表中，由 parse_assignment 单独处理。
    match kind {
        TokenKind::OrOr => Some(1),
        TokenKind::AndAnd => Some(2),
        TokenKind::Pipe => Some(3),
        TokenKind::Caret => Some(4),
        TokenKind::Ampersand => Some(5),
        TokenKind::EqualEqual | TokenKind::BangEqual => Some(6),
        TokenKind::Less | TokenKind::LessEqual | TokenKind::Greater | TokenKind::GreaterEqual => {
            Some(7)
        }
        TokenKind::ShiftLeft | TokenKind::ShiftRight => Some(8),
        TokenKind::Plus | TokenKind::Minus => Some(9),
        TokenKind::Star | TokenKind::Slash | TokenKind::Percent => Some(10),
        _ => None,
    }
}

fn unescape_string(source: &str) -> String {
    // Lexer 已验证字符串边界和转义是否合法，这里只生成实际字符串值。
    let inner = &source[1..source.len() - 1];
    let mut chars = inner.chars();
    let mut result = String::new();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            result.push(ch);
            continue;
        }
        result.push(match chars.next().expect("lexer validates escapes") {
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            '0' => '\0',
            '"' => '"',
            '\\' => '\\',
            _ => unreachable!("lexer validates escapes"),
        });
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> Result<Program, ParseError> {
        Parser::new(Lexer::new(source.into())).parse()
    }

    #[test]
    fn parses_precedence_and_assignment() {
        let program = parse("let x = 1 + 2 * 3; x += 4;").unwrap();
        assert_eq!(program.statements.len(), 2);
        let Statement::Variable { value, .. } = &program.statements[0] else {
            panic!()
        };
        let Expression::Binary {
            operator, right, ..
        } = value
        else {
            panic!()
        };
        assert_eq!(*operator, TokenKind::Plus);
        assert!(matches!(
            **right,
            Expression::Binary {
                operator: TokenKind::Star,
                ..
            }
        ));
        assert!(matches!(
            program.statements[1],
            Statement::Expression(Expression::Assignment { .. })
        ));
    }

    #[test]
    fn parses_control_flow_functions_and_arrays() {
        let source = r#"
            fn add(a, b) { return a + b; }
            if true { let xs = [1, 2, 3]; } else { while false { break; } }
            for item in values { print(item); }
        "#;
        let program = parse(source).unwrap();
        assert_eq!(program.statements.len(), 3);
        assert!(matches!(program.statements[0], Statement::Function { .. }));
        assert!(matches!(program.statements[1], Statement::If { .. }));
        assert!(matches!(program.statements[2], Statement::For { .. }));
    }

    #[test]
    fn rejects_invalid_assignment_target() {
        let error = parse("(1 + 2) = 3;").unwrap_err();
        assert!(error.message.contains("assignment target"));
    }

    #[test]
    fn reports_missing_delimiter() {
        let error = parse("let x = [1, 2;").unwrap_err();
        assert!(error.message.contains("expected `]`"));
    }
}
