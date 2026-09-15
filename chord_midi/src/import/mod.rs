use crate::model::ast::Expr;

mod chord;
mod parser_util;
mod rechord;
mod sexp;

pub trait Importer {
    fn import(&self, code: &str) -> anyhow::Result<Expr>;
}

#[derive(Debug)]
pub struct RechordImporter;

#[derive(Debug)]
pub struct SexpImporter;
