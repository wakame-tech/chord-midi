use crate::model::ast::Ast;

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
