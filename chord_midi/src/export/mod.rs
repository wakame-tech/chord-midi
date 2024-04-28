use crate::model::ast::Ast;
use std::io::Write;

mod midi;
mod rechord;

pub trait Exporter {
    fn export(&self, f: &mut impl Write, ast: Ast) -> anyhow::Result<()>;
}

#[derive(Debug)]
pub struct RechordExporter;

#[derive(Debug)]
pub struct MidiExporter {
    pub bpm: u8,
}
