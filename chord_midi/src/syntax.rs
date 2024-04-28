use anyhow::Result;
use std::collections::BTreeSet;

#[derive(Debug, PartialEq)]
pub enum Ast {
    Comment(String),
    // nodes, br?
    Measure(Vec<Node>, bool),
    Score(Vec<Box<Ast>>),
}

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
                        c.key.as_degree(key);
                    }
                }
            }
            Ast::Comment(_) => {}
        }
    }

    pub fn as_pitch(&mut self, pitch: Pitch) {
        match self {
            Ast::Score(nodes) => {
                for node in nodes {
                    node.as_pitch(pitch);
                }
            }
            Ast::Measure(nodes, _) => {
                for node in nodes {
                    if let Node::Chord(c) = node {
                        c.key.as_pitch(pitch);
                    }
                }
            }
            Ast::Comment(_) => {}
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

#[derive(Debug, PartialEq, Clone)]
pub enum Key {
    Absolute(Pitch),
    // semitones
    Relative(u8),
}

impl Key {
    pub fn as_degree(&mut self, key: Pitch) {
        match self {
            Key::Absolute(pitch) => *self = Key::Relative(pitch.diff(&key)),
            Key::Relative(_) => {}
        }
    }

    pub fn as_pitch(&mut self, pitch: Pitch) {
        match self {
            Key::Absolute(_) => {}
            Key::Relative(degree) => {
                *self = Key::Absolute(Pitch::try_from((*degree + pitch as u8) % 12).unwrap())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChordNode {
    pub key: Key,
    pub modifiers: BTreeSet<Modifier>,
    pub on: Option<Key>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Modifier {
    Major(u8),
    Minor(u8),
    MinorMajaor7,
    Sus2,
    Sus4,
    Flat5th,
    Aug,
    Aug7,
    Dim,
    Dim7,
    Omit(u8),
    Add(u8),
    Tension(Degree),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Accidental {
    Natural,
    Sharp,
    Flat,
}

impl From<Accidental> for i8 {
    fn from(val: Accidental) -> Self {
        match val {
            Accidental::Natural => 0,
            Accidental::Sharp => 1,
            Accidental::Flat => -1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Degree(pub u8, pub Accidental);

impl Degree {
    pub fn with_pitch(&self, pitch: Pitch) -> Pitch {
        let i: i8 = self.1.clone().into();
        Pitch::try_from(((pitch as u8 as i8 + self.0 as i8 + i) % 12) as u8).unwrap()
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum Pitch {
    C,
    Cs,
    D,
    Ds,
    E,
    F,
    Fs,
    G,
    Gs,
    A,
    As,
    B,
}

impl TryFrom<u8> for Pitch {
    type Error = anyhow::Error;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Pitch::C),
            1 => Ok(Pitch::Cs),
            2 => Ok(Pitch::D),
            3 => Ok(Pitch::Ds),
            4 => Ok(Pitch::E),
            5 => Ok(Pitch::F),
            6 => Ok(Pitch::Fs),
            7 => Ok(Pitch::G),
            8 => Ok(Pitch::Gs),
            9 => Ok(Pitch::A),
            10 => Ok(Pitch::As),
            11 => Ok(Pitch::B),
            _ => Err(anyhow::anyhow!("invalid pitch: {}", value)),
        }
    }
}

impl Pitch {
    fn diff(&self, other: &Self) -> u8 {
        let a = *self as i8;
        let b = *other as i8;
        (a - b + 12) as u8 % 12
    }
}
