use crate::lexer::Lexer;
use std::{env, fs};
use lexer::token::TokenKind;
mod lexer;
mod parser;

fn main() {
    let mut  args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        println!("Usage: {} [filename]", args[0]);
        return;
    }
    args[1].push(' ');
    let file = &args[1];
    let source = fs::read_to_string(&file).unwrap();
    let mut  lexer = Lexer::new(source);
    loop{
        let token = lexer.next_token();
        match token.kind {
            TokenKind::Eof => break,
            TokenKind::Illegal =>{
                 println!("Illegal Token {:#?}",token);
                 break;
            }
            _=>{
                println!("{:?}",token);
            }
        }
    }
}

