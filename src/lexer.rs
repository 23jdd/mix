/*
Lexer
 ├── next_token()
 ├── peek()
 ├── advance()
 ├── match_char()
 ├── skip_whitespace()
 ├── scan_identifier()
 ├── scan_number()
 ├── scan_string()
 └── scan_operator()
 */

mod token;
mod dfa;
use token::Token;
pub struct Lexer{
     source:String,

}
impl Lexer {
    pub fn new(source: String) -> Self {
        Self {
            source
        }
    }
    pub fn next_token(&self)->Token{
          todo!()
    }
}