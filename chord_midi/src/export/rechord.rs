use super::{Exporter, RechordExporter};
use crate::model::ast::{Ast, ChordNode, Directive, Node};
use crate::model::modifier::Modifier;
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
            Ast::Directive(Directive::Key(p)) => writeln!(f, "key: {}", p),
            Ast::Directive(Directive::Scale(s)) => writeln!(f, "scale: {}", s),
            Ast::Apply(_, _) => Ok(()),
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
            Node::Brush(chord) => write!(f, "{}", chord),
            Node::Arpeggio(chord) => write!(f, "arp({})", chord),
            Node::Transform(t, c) => write!(f, "{}({})", transform_prefix(t), c),
            Node::Rest => write!(f, "N.C."),
            Node::Sustain => write!(f, "="),
            Node::Repeat => write!(f, "%"),
        }
    }
}

fn transform_prefix(t: &crate::model::ast::Transform) -> &'static str {
    use crate::model::ast::Transform::*;
    match t {
        S25 => "s25",
        Sub => "sub",
        Dom => "dom",
        Blk => "blk",
    }
}

impl Display for ChordNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // テンションはまとめて `(9,b13)` の形で出す。パーサが読むのはこの形
        // だけなので、素で並べると `C7(9)` が `C79` になって読み戻せない
        let (tensions, mods): (Vec<_>, Vec<_>) = self
            .modifiers
            .iter()
            .partition(|m| matches!(m, Modifier::Tension(_)));
        let mods = mods
            .iter()
            .map(|m| format!("{}", m))
            .collect::<Vec<_>>()
            .join("");
        let tensions = if tensions.is_empty() {
            String::new()
        } else {
            format!(
                "({})",
                tensions
                    .iter()
                    .map(|m| format!("{}", m))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        };
        let on = self
            .on
            .as_ref()
            .map(|p| format!("/{}", p))
            .unwrap_or("".to_string());
        write!(f, "{}{}{}{}", self.key, mods, tensions, on)
    }
}

#[cfg(test)]
mod tests {
    use crate::import::{Importer, RechordImporter};

    #[test]
    fn test_arpeggio_roundtrip() {
        let ast = RechordImporter.import("arp(C) |\n").unwrap();
        let s = format!("{}", ast);
        assert!(s.contains("arp(C)"));
    }
}
