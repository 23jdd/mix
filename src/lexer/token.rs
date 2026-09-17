#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // =========================
    // Special
    // =========================
    Eof,

    // =========================
    // Literals
    // =========================
    Identifier,

    Integer,
    Float,
    String,

    True,
    False,
    Null,

    // =========================
    // Arithmetic operators
    // =========================
    Plus,       // +
    Minus,      // -
    Star,       // *
    Slash,      // /
    Percent,    // %

    // =========================
    // Assignment
    // =========================
    Equal,          // =
    PlusEqual,      // +=
    MinusEqual,     // -=
    StarEqual,      // *=
    SlashEqual,     // /=
    PercentEqual,   // %=

    // =========================
    // Comparison
    // =========================
    EqualEqual,     // ==
    BangEqual,      // !=

    Less,           // <
    LessEqual,      // <=
    Greater,        // >
    GreaterEqual,   // >=

    // =========================
    // Logical
    // =========================
    Bang,           // !
    AndAnd,         // &&
    OrOr,           // ||

    // =========================
    // Bitwise
    // =========================
    Ampersand,      // &
    Pipe,           // |
    Caret,          // ^
    Tilde,          // ~

    ShiftLeft,      // <<
    ShiftRight,     // >>


    // =========================
    // Delimiters
    // =========================
    LeftParen,      // (
    RightParen,     // )

    LeftBrace,      // {
    RightBrace,     // }

    LeftBracket,    // [
    RightBracket,   // ]

    // =========================
    // Separators
    // =========================
    Comma,          // ,
    Dot,            // .
    Colon,          // :
    Semicolon,      // ;

    // =========================
    // Other punctuation
    // =========================
    Arrow,          // ->
    FatArrow,       // =>

    Question,       // ?

    // =========================
    // Variables
    // =========================
    Let,            // let
    Const,          // const

    // =========================
    // Control flow
    // =========================
    If,             // if
    Else,           // else

    While,          // while
    For,            // for
    In,             // in

    Break,          // break
    Continue,       // continue

    // =========================
    // Functions
    // =========================
    Fn,             // fn
    Return,         // return


    // =========================
    // Error
    // =========================
    Illegal,
}
/// Span
pub struct Span{
    start:usize,
    end:usize,
}
impl Span {
    fn new(start:usize,end:usize)->Self{
         Self{
               start,end
         }
    }
}
pub struct Token {
    pub kind: TokenKind,
    pub lexeme: String,
    pub span: Span,
}
impl Token {

}
// keyword match
fn keyword(ident: &str) -> TokenKind {
    match ident {
        "let" => TokenKind::Let,
        "const" => TokenKind::Const,

        "fn" => TokenKind::Fn,
        "return" => TokenKind::Return,

        "if" => TokenKind::If,
        "else" => TokenKind::Else,

        "while" => TokenKind::While,
        "for" => TokenKind::For,
        "in" => TokenKind::In,

        "break" => TokenKind::Break,
        "continue" => TokenKind::Continue,

        "true" => TokenKind::True,
        "false" => TokenKind::False,
        "null" => TokenKind::Null,

        _ => TokenKind::Identifier,
    }
}

fn unary(){

}
fn binary(){

}