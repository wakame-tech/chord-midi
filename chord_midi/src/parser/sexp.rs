use super::SexpParser;
use crate::syntax::Ast;
use anyhow::Result;

impl super::Parser for SexpParser {
    fn parse(&self, code: &str) -> Result<Ast> {
        todo!()
    }
}
