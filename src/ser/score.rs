use crate::{
    de::ast::{Node, AST},
    model::degree::Pitch,
};
use anyhow::Result;
use std::io::Write;

impl AST {
    pub fn as_degree(&mut self, key: Pitch) {
        for measure in &mut self.0 {
            for node in &mut measure.0 {
                if let Node::Chord(c) = node {
                    *node = Node::Degree(c.clone().into_degree_node(key));
                }
            }
        }
    }
}

pub fn dump(f: &mut impl Write, ast: &AST) -> Result<()> {
    for measure in &ast.0 {
        for node in &measure.0 {
            write!(f, "{} ", node)?;
        }
        write!(f, "| ")?;
    }
    Ok(())
}
