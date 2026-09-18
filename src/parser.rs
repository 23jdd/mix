use std::fmt;

use crate::ast::{Block, Expression, Program, Statement};
use crate::lexer::Lexer;
use crate::lexer::token::{Span, Token, TokenKind};

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
    lexer: Lexer,
    current: Token,
}

impl Parser {
    pub fn new(mut lexer: Lexer) -> Self {
        let current = lexer.next_token();
        Self { lexer, current }
    }

    pub fn parse(&mut self) -> Result<Program, ParseError> {
        let mut statements = Vec::new();
        while !self.at(&TokenKind::Eof) {
            statements.push(self.parse_statement()?);
        }
        Ok(Program { statements })
    }

    fn parse_statement(&mut self) -> Result<Statement, ParseError> {
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

    fn parse_if(&mut self) -> Result<Statement, ParseError> {
        self.advance();
        let condition = self.parse_expression()?;
        let then_branch = self.parse_block()?;
        let else_branch = if self.take(&TokenKind::Else) {
            if self.at(&TokenKind::If) {
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

    fn parse_while(&mut self) -> Result<Statement, ParseError> {
        self.advance();
        let condition = self.parse_expression()?;
        let body = self.parse_block()?;
        Ok(Statement::While { condition, body })
    }

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

    fn parse_block(&mut self) -> Result<Block, ParseError> {
        self.expect(TokenKind::LeftBrace, "expected `{`")?;
        let mut statements = Vec::new();
        while !self.at(&TokenKind::RightBrace) && !self.at(&TokenKind::Eof) {
            statements.push(self.parse_statement()?);
        }
        self.expect(TokenKind::RightBrace, "expected `}` after block")?;
        Ok(statements)
    }

    fn parse_expression(&mut self) -> Result<Expression, ParseError> {
        self.parse_assignment()
    }

    fn parse_assignment(&mut self) -> Result<Expression, ParseError> {
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

    fn parse_binary(&mut self, min_precedence: u8) -> Result<Expression, ParseError> {
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

    fn parse_postfix(&mut self) -> Result<Expression, ParseError> {
        let mut expression = self.parse_primary()?;
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

    fn at(&self, kind: &TokenKind) -> bool {
        &self.current.kind == kind
    }

    fn take(&mut self, kind: &TokenKind) -> bool {
        if self.at(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, kind: TokenKind, message: &str) -> Result<Token, ParseError> {
        if self.at(&kind) {
            Ok(self.advance())
        } else {
            Err(self.error(message))
        }
    }

    fn expect_identifier(&mut self, message: &str) -> Result<String, ParseError> {
        Ok(self.expect(TokenKind::Identifier, message)?.lexeme)
    }

    fn advance(&mut self) -> Token {
        std::mem::replace(&mut self.current, self.lexer.next_token())
    }

    fn error(&self, message: impl Into<String>) -> ParseError {
        ParseError {
            message: message.into(),
            span: self.current.span,
        }
    }
}

fn binary_precedence(kind: &TokenKind) -> Option<u8> {
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
