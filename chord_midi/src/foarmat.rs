use crate::syntax::{Accidental, Ast, ChordNode, Degree, Key, Modifier, Node, Pitch};
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

impl Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Key::Absolute(pitch) => write!(f, "{}", pitch),
            Key::Relative(degree) => write!(f, "{}", degree),
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

impl Display for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Node::Chord(c) => write!(f, "{}", c),
            Node::Rest => write!(f, ""),
            Node::Sustain => write!(f, "_"),
            Node::Repeat => write!(f, "%"),
        }
    }
}

impl Display for Modifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Modifier::Major(d) => write!(f, "{}", d),
            Modifier::Minor(d) => write!(f, "m{}", d),
            Modifier::MinorMajaor7 => write!(f, "mM7"),
            Modifier::Sus2 => write!(f, "sus2"),
            Modifier::Sus4 => write!(f, "sus4"),
            Modifier::Flat5th => write!(f, "-5"),
            Modifier::Aug => write!(f, "aug"),
            Modifier::Aug7 => write!(f, "aug7"),
            Modifier::Dim => write!(f, "dim"),
            Modifier::Dim7 => write!(f, "dim7"),
            Modifier::Omit(d) => write!(f, "omit{}", d),
            Modifier::Add(d) => write!(f, "add{}", d),
            Modifier::Tension(Degree(a, d)) => write!(f, "{}{}", a, d),
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

fn to_roman_str(v: u8) -> &'static str {
    match v {
        1 => "I",
        2 => "II",
        3 => "III",
        4 => "IV",
        5 => "V",
        6 => "VI",
        7 => "VII",
        _ => panic!("invalid degree: {}", v),
    }
}

impl Display for Degree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", to_roman_str(self.0), self.1)
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
