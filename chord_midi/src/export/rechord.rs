use super::{Exporter, RechordExporter};
use crate::model::ast::{Ast, ChordNode, Node};
use std::{fmt::Display, io::Write};

impl Exporter for RechordExporter {
    fn export(&self, f: &mut impl Write, ast: Ast) -> anyhow::Result<()> {
        writeln!(f, "{}", ast)?;
        Ok(())
    }
}

impl Display for Ast {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Ast::Score(nodes) => {
                for node in nodes {
                    write!(f, "{}", node)?;
                }
                Ok(())
            }
            Ast::Comment(comment) => {
                writeln!(f, "# {}", comment)
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
                    writeln!(f)?;
                }
                Ok(())
            }
        }
    }
}

impl Display for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Node::Chord(chord) => write!(f, "{}", chord),
            Node::Rest => write!(f, "N.C."),
            Node::Sustain => write!(f, "="),
            Node::Repeat => write!(f, "%"),
        }
    }
}

impl Display for ChordNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mods = self
            .modifiers
            .iter()
            .map(|m| format!("{}", m))
            .collect::<Vec<_>>()
            .join("");
        let on = self
            .on
            .as_ref()
            .map(|p| format!("/{}", p))
            .unwrap_or("".to_string());
        write!(f, "{}{}{}", self.key, mods, on)
    }
}
