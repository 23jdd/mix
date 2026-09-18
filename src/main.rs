use crate::lexer::Lexer;
use parser::Parser;
use std::{env, fs};
mod ast;
mod lexer;
mod parser;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <filename>", args[0]);
        return;
    }

    let source = match fs::read_to_string(&args[1]) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("failed to read {}: {error}", args[1]);
            std::process::exit(1);
        }
    };
    let mut parser = Parser::new(Lexer::new(source.clone()));
    match parser.parse() {
        Ok(program) => println!("{program:#?}"),
        Err(error) => {
            let (line, column) = line_column(&source, error.span.start);
            eprintln!("{}:{}:{}: {}", args[1], line, column, error.message);
            std::process::exit(1);
        }
    }
}

fn line_column(source: &str, byte_offset: usize) -> (usize, usize) {
    let prefix = &source[..byte_offset.min(source.len())];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = prefix
        .rsplit_once('\n')
        .map_or(prefix, |(_, tail)| tail)
        .chars()
        .count()
        + 1;
    (line, column)
}
