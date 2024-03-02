use crate::{
    de::ast::{DegreeNode, Node, AST},
    model::degree::{Degree, Pitch},
};
use anyhow::Result;
use std::io::Write;

impl AST {
    pub fn as_degree(&mut self, key: Pitch) {
        for measure in &mut self.0 {
            for node in &mut measure.0 {
                match node {
                    Node::Chord(c) => {
                        let d = Pitch::diff(&c.root, &key);
                        let (degree, _) = Degree::from_semitone(d);
                        *node = Node::Degree(DegreeNode {
                            root: degree,
                            modifiers: c.modifiers.clone(),
                        });
                    }
                    Node::Degree(_) => (),
                    Node::Rest => (),
                    Node::Sustain => (),
                    Node::Repeat => (),
                }
            }
        }
    }
}

// TODO
pub fn dump(f: &mut impl Write, ast: &AST) -> Result<()> {
    for measure in &ast.0 {
        for node in &measure.0 {
            match node {
                Node::Chord(_) => {
                    todo!()
                }
                Node::Degree(d) => {
                    write!(f, "{} ", d.root)?;
                }
                Node::Rest => write!(f, "")?,
                Node::Sustain => write!(f, "_ ")?,
                Node::Repeat => write!(f, "% ")?,
            }
        }
        write!(f, "| ")?;
    }
    Ok(())
}
