use self::ast::Ast;

pub mod ast;
mod chord;
mod parser_util;
mod rechord;
mod sexp;

pub trait Parser {
    fn parse(&self, code: &str) -> anyhow::Result<Ast>;
}

#[derive(Debug)]
pub struct RechordParser;

#[derive(Debug)]
pub struct SexpParser;
