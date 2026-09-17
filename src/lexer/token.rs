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
    pub fn new(start:usize,end:usize)->Self{
         Self{
               start,end
         }
    }
}
pub struct Token<'a>{
    pub kind: TokenKind,
    pub lexeme:&'a str,
    pub span: Span,
}
impl<'a> Token<'a> {
     pub fn new(kind:TokenKind,lexeme:&'a str,start:usize,end:usize)->Self{
          Self{
                kind,
                lexeme,
                span: Span::new(start,end),
          }
     }
}
// keyword match
pub fn keyword(ident: &str) -> TokenKind {
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
pub fn operator(opt: &str) -> TokenKind {
    match opt {
        "+" => TokenKind::Plus,
        "-" => TokenKind::Minus,
        "*" => TokenKind::Star,
        "/" => TokenKind::Slash,
        "%" => TokenKind::Percent,
        "=" => TokenKind::Equal,
        "+=" => TokenKind::PlusEqual,
        "-=" => TokenKind::MinusEqual,
        "*=" => TokenKind::StarEqual,
        "/=" => TokenKind::SlashEqual,
        "%=" => TokenKind::PercentEqual,
        "==" => TokenKind::EqualEqual,
        "!=" => TokenKind::BangEqual,
        "<" => TokenKind::Less,
        "<=" => TokenKind::LessEqual,
        ">" => TokenKind::Greater,
        ">=" => TokenKind::GreaterEqual,
        "!" => TokenKind::Bang,
        "&&" => TokenKind::AndAnd,
        "||" => TokenKind::OrOr,
        "&" => TokenKind::Ampersand,
        "|" => TokenKind::Pipe,
        "^" => TokenKind::Caret,
        "~" => TokenKind::Tilde,
        "<<" => TokenKind::ShiftLeft,
        ">>" => TokenKind::ShiftRight,
        "(" => TokenKind::LeftParen,
        ")" => TokenKind::RightParen,
        "{" => TokenKind::LeftBrace,
        "}" => TokenKind::RightBrace,
        "[" => TokenKind::LeftBracket,
        "]" => TokenKind::RightBracket,
        "," => TokenKind::Comma,
        "." => TokenKind::Dot,
        ":" => TokenKind::Colon,
        ";" => TokenKind::Semicolon,
        "->" => TokenKind::Arrow,
        "=>" => TokenKind::FatArrow,
        "?" => TokenKind::Question,
        _ => TokenKind::Illegal,
    }
}

