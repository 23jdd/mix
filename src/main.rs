use crate::lexer::Lexer;
use parser::Parser;
use runtime::Runtime;
use std::{env, fs};
mod ast;
mod lexer;
mod parser;
mod runtime;
mod value;
fn main() {
    // 当前命令行接口只接收一个 .mix 源文件路径。
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <filename>", args[0]);
        return;
    }

    // 保留 source 的副本用于把 Parser 的字节 Span 转换成行列号。
    let source = match fs::read_to_string(&args[1]) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("failed to read {}: {error}", args[1]);
            std::process::exit(1);
        }
    };
    // 前端负责 Source -> Token -> AST；解析成功后再交给 Runtime 执行。
    let mut parser = Parser::new(Lexer::new(source.clone()));
    match parser.parse() {
        Ok(program) => {
            if let Err(error) = Runtime::new(program).run() {
                eprintln!("runtime error: {error}");
                std::process::exit(1);
            }
        }
        Err(error) => {
            let (line, column) = line_column(&source, error.span.start);
            eprintln!("{}:{}:{}: {}", args[1], line, column, error.message);
            std::process::exit(1);
        }
    }
}

fn line_column(source: &str, byte_offset: usize) -> (usize, usize) {
    // Parser 记录字节偏移；输出错误时转换为用户更容易定位的行列号。
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
