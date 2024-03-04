use crate::{
    de::ast::{Ast, Node},
    model::degree::Pitch,
};
use anyhow::Result;
use std::io::Write;

impl Ast {
    pub fn as_degree(&mut self, key: Pitch) {
        match self {
            Ast::Score(nodes) => {
                for node in nodes {
                    node.as_degree(key);
                }
            }
            Ast::Measure(nodes, _) => {
                for node in nodes {
                    if let Node::Chord(c) = node {
                        *node = Node::Degree(c.clone().into_degree_node(key));
                    }
                }
            }
            Ast::Comment(_) => {}
        }
    }
}

pub fn dump(f: &mut impl Write, ast: &Ast) -> Result<()> {
    match ast {
        Ast::Score(nodes) => {
            for node in nodes {
                dump(f, node)?;
            }
        }
        Ast::Comment(comment) => {
            write!(f, "# {}\n", comment)?;
        }
        Ast::Measure(nodes, br) => {
            write!(
                f,
                "{}",
                nodes
                    .iter()
                    .map(|n| n.to_string())
                    .collect::<Vec<_>>()
                    .join(" ")
            )?;
            write!(f, " | ")?;
            if *br {
                write!(f, "\n")?;
            }
        }
    }
    Ok(())
}
