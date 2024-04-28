use crate::model::{
    chord::{match_pitches, Chord},
    key::Key,
    modifier::Modifier,
    pitch::Pitch,
};
use anyhow::Result;
use std::{collections::BTreeSet, fmt::Display};

#[derive(Debug, PartialEq)]
pub enum Ast {
    Comment(String),
    // nodes, br?
    Measure(Vec<Node>, bool),
    Score(Vec<Box<Ast>>),
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

#[derive(Debug, PartialEq)]
pub enum Node {
    Chord(ChordNode),
    Rest,
    Sustain,
    Repeat,
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

#[derive(Debug, Clone, PartialEq)]
pub struct ChordNode {
    pub key: Key,
    pub modifiers: BTreeSet<Modifier>,
    pub on: Option<Key>,
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

impl ChordNode {
    pub fn absolute(pitch: Pitch) -> Self {
        ChordNode {
            key: Key::Absolute(pitch),
            modifiers: BTreeSet::new(),
            on: None,
        }
    }

    pub fn relative(semitone: u8) -> Self {
        ChordNode {
            key: Key::Relative(semitone),
            modifiers: BTreeSet::new(),
            on: None,
        }
    }

    pub fn to_chord(&self) -> Result<Chord> {
        let mut chord = Chord::new(5, 0, self.key.clone());
        chord.on = self.on.clone();
        for modifier in &self.modifiers {
            chord.modify(modifier)?;
        }
        let (octave, inversion) = match_pitches(12 * chord.octave, &chord)?;
        chord.octave = octave;
        chord.inversion = inversion;
        Ok(chord)
    }
}
