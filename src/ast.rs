use crate::lexer::token::TokenKind;

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub statements: Vec<Statement>,
}

pub type Block = Vec<Statement>;

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Variable {
        mutable: bool,
        name: String,
        value: Expression,
    },
    Function {
        name: String,
        parameters: Vec<String>,
        body: Block,
    },
    If {
        condition: Expression,
        then_branch: Block,
        else_branch: Option<Block>,
    },
    While {
        condition: Expression,
        body: Block,
    },
    For {
        name: String,
        iterable: Expression,
        body: Block,
    },
    Return(Option<Expression>),
    Break,
    Continue,
    Expression(Expression),
    Block(Block),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    Integer(i64),
    Float(f64),
    String(String),
    Boolean(bool),
    Null,
    Identifier(String),
    Array(Vec<Expression>),
    Unary {
        operator: TokenKind,
        operand: Box<Expression>,
    },
    Binary {
        left: Box<Expression>,
        operator: TokenKind,
        right: Box<Expression>,
    },
    Assignment {
        target: Box<Expression>,
        operator: TokenKind,
        value: Box<Expression>,
    },
    Call {
        callee: Box<Expression>,
        arguments: Vec<Expression>,
    },
    Index {
        object: Box<Expression>,
        index: Box<Expression>,
    },
    Member {
        object: Box<Expression>,
        property: String,
    },
}

impl Expression {
    pub fn is_assignable(&self) -> bool {
        matches!(
            self,
            Self::Identifier(_) | Self::Index { .. } | Self::Member { .. }
        )
    }
}
