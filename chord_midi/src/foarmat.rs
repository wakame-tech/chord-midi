use crate::syntax::{Accidental, Ast, ChordNode, Degree, DegreeNode, ModifierNode, Node, Pitch};
use std::fmt::Display;

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

fn fmt_mods(mods: &[ModifierNode]) -> String {
    mods.iter()
        .map(|m| format!("{}", m))
        .collect::<Vec<_>>()
        .join("")
}

impl Display for ChordNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (root, modifiers, tensions, on) = (
            &self.root,
            &self.modifiers,
            &self.tensions.as_ref(),
            &self.on,
        );
        let root = format!("{}", root);
        let mods = fmt_mods(modifiers);
        let tensions = tensions
            .map(|t| format!("({})", fmt_mods(t)))
            .unwrap_or("".to_string());
        let on = on.map(|p| format!("/{}", p)).unwrap_or("".to_string());
        write!(f, "{}{}{}{}", root, mods, tensions, on)
    }
}

impl Display for DegreeNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let root = format!("{}{}", self.root.0, self.root.1);
        let mods = fmt_mods(&self.modifiers);
        let tensions = self
            .tensions
            .as_ref()
            .map(|t| format!("({})", fmt_mods(t)))
            .unwrap_or("".to_string());
        let on = self
            .on
            .as_ref()
            .map(|(a, d)| format!("/{}{}", a, d))
            .unwrap_or("".to_string());
        write!(f, "{}{}{}{}", root, mods, tensions, on)
    }
}

impl Display for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Node::Chord(c) => write!(f, "{}", c),
            Node::Degree(d) => write!(f, "{}", d),
            Node::Rest => write!(f, ""),
            Node::Sustain => write!(f, "_"),
            Node::Repeat => write!(f, "%"),
        }
    }
}

impl Display for ModifierNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModifierNode::Major(d) => write!(f, "{}", d.0),
            ModifierNode::Minor(d) => write!(f, "m{}", d.0),
            ModifierNode::MinorMajaor7 => write!(f, "mM7"),
            ModifierNode::Sus2 => write!(f, "sus2"),
            ModifierNode::Sus4 => write!(f, "sus4"),
            ModifierNode::Flat5th => write!(f, "-5"),
            ModifierNode::Aug => write!(f, "aug"),
            ModifierNode::Aug7 => write!(f, "aug7"),
            ModifierNode::Dim => write!(f, "dim"),
            ModifierNode::Dim7 => write!(f, "dim7"),
            ModifierNode::Omit(d) => write!(f, "omit{}", d.0),
            ModifierNode::Add(d) => write!(f, "add{}", d.0),
            ModifierNode::Tension(a, d) => write!(f, "{}{}", a, d.0),
        }
    }
}

impl Display for Accidental {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Accidental::Natural => write!(f, ""),
            Accidental::Sharp => write!(f, "#"),
            Accidental::Flat => write!(f, "b"),
        }
    }
}

impl Display for Degree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self.0 {
            1 => "I",
            2 => "II",
            3 => "III",
            4 => "IV",
            5 => "V",
            6 => "VI",
            7 => "VII",
            _ => panic!("invalid degree: {}", self.0),
        };
        write!(f, "{}", s)
    }
}

impl Display for Pitch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Pitch::*;
        let s = match self {
            C => "C",
            Cs => "C#",
            D => "D",
            Ds => "D#",
            E => "E",
            F => "F",
            Fs => "F#",
            G => "G",
            Gs => "G#",
            A => "A",
            As => "A#",
            B => "B",
        };
        write!(f, "{}", s)
    }
}
