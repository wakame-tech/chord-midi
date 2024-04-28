use self::ast::Ast;

pub mod ast;
mod chord;
mod parser_util;
mod rechord;
mod sexp;

pub trait Importer {
    fn import(&self, code: &str) -> anyhow::Result<Ast>;
}

#[derive(Debug)]
pub struct RechordImporter;

#[derive(Debug)]
pub struct SexpImporter;
