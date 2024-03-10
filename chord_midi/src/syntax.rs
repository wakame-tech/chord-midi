use crate::chord::{Chord, Modifier};
use anyhow::Result;
use std::fmt::Display;

#[derive(Debug)]
pub enum Ast {
    Comment(String),
    Measure(Vec<Node>, bool),
    Score(Vec<Box<Ast>>),
}

#[derive(Debug, PartialEq)]
pub enum Node {
    Chord(ChordNode),
    Degree(DegreeNode),
    Rest,
    Sustain,
    Repeat,
}

#[derive(Debug, PartialEq, Clone)]
pub struct ChordNode {
    pub root: Pitch,
    pub modifiers: Vec<ModifierNode>,
    pub tensions: Option<Vec<ModifierNode>>,
    pub on: Option<Pitch>,
}

#[derive(Debug, PartialEq)]
pub struct DegreeNode {
    pub root: (Accidental, Degree),
    pub modifiers: Vec<ModifierNode>,
    pub tensions: Option<Vec<ModifierNode>>,
    pub on: Option<(Accidental, Degree)>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ModifierNode {
    Major(Degree),
    Minor(Degree),
    MinorMajaor7,
    Sus2,
    Sus4,
    Flat5th,
    Aug,
    Aug7,
    Dim,
    Dim7,
    Omit(Degree),
    Add(Degree),
    Tension(Accidental, Degree),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Accidental {
    Natural,
    Sharp,
    Flat,
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
pub struct Degree(pub u8);

impl Degree {
    pub fn to_semitone(&self) -> Result<i8> {
        match self.0 {
            1 => Ok(0),
            2 => Ok(2),
            3 => Ok(4),
            4 => Ok(5),
            5 => Ok(7),
            6 => Ok(9),
            7 => Ok(11),
            9 => Ok(14),
            11 => Ok(17),
            13 => Ok(21),
            _ => Err(anyhow::anyhow!("unknown degree {}", self.0)),
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

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
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

impl Pitch {
    pub fn from_u8(n: u8) -> Self {
        use Pitch::*;
        match n {
            0 => C,
            1 => Cs,
            2 => D,
            3 => Ds,
            4 => E,
            5 => F,
            6 => Fs,
            7 => G,
            8 => Gs,
            9 => A,
            10 => As,
            11 => B,
            _ => unreachable!(),
        }
    }

    pub fn into_u8(self) -> u8 {
        use Pitch::*;
        match self {
            C => 0,
            Cs => 1,
            D => 2,
            Ds => 3,
            E => 4,
            F => 5,
            Fs => 6,
            G => 7,
            Gs => 8,
            A => 9,
            As => 10,
            B => 11,
        }
    }

    pub fn diff(from: &Pitch, to: &Pitch) -> u8 {
        let diff = (to.into_u8() as i8 - from.into_u8() as i8 + 12) % 12;
        diff as u8
    }
}

pub fn into_semitone(a: Accidental, d: Degree) -> u8 {
    let p = d.to_semitone().unwrap();
    let a: i8 = a.into();
    (p + a) as u8
}

pub fn from_semitone(s: u8) -> (Accidental, Degree) {
    match s {
        0 => (Accidental::Natural, Degree(1)),
        1 => (Accidental::Sharp, Degree(1)),
        2 => (Accidental::Natural, Degree(2)),
        3 => (Accidental::Sharp, Degree(2)),
        4 => (Accidental::Natural, Degree(3)),
        5 => (Accidental::Natural, Degree(4)),
        6 => (Accidental::Sharp, Degree(4)),
        7 => (Accidental::Natural, Degree(5)),
        8 => (Accidental::Sharp, Degree(5)),
        9 => (Accidental::Natural, Degree(6)),
        10 => (Accidental::Sharp, Degree(6)),
        11 => (Accidental::Natural, Degree(7)),
        _ => panic!("invalid semitone: {}", s),
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

impl ModifierNode {
    pub fn degree_to_mods(is_minor: bool, d: Degree) -> Result<Vec<Modifier>> {
        let third = Modifier::Mod(
            Degree(3),
            if is_minor {
                Accidental::Flat
            } else {
                Accidental::Natural
            },
        );
        let seventh = Modifier::Add(
            Degree(7),
            if is_minor {
                Accidental::Flat
            } else {
                Accidental::Natural
            },
        );
        match d {
            Degree(5) => Ok(vec![third]),
            Degree(6) => Ok(vec![third, Modifier::Add(Degree(6), Accidental::Natural)]),
            Degree(7) => Ok(vec![third, seventh]),
            Degree(9) => Ok(vec![
                third,
                seventh,
                Modifier::Add(Degree(9), Accidental::Natural),
            ]),
            _ => Err(anyhow::anyhow!("invalid degree: {:?}", d)),
        }
    }

    pub fn into_modifiers(self) -> Result<Vec<Modifier>> {
        match self {
            ModifierNode::Major(d) => Self::degree_to_mods(false, d),
            ModifierNode::Minor(d) => Self::degree_to_mods(true, d),
            ModifierNode::MinorMajaor7 => Ok(vec![
                Modifier::Mod(Degree(3), Accidental::Flat),
                Modifier::Add(Degree(7), Accidental::Natural),
            ]),
            ModifierNode::Sus2 => Ok(vec![Modifier::Mod(Degree(3), Accidental::Flat)]),
            ModifierNode::Sus4 => Ok(vec![Modifier::Mod(Degree(3), Accidental::Sharp)]),
            ModifierNode::Flat5th => Ok(vec![Modifier::Mod(Degree(5), Accidental::Flat)]),
            ModifierNode::Aug => Ok(vec![Modifier::Mod(Degree(5), Accidental::Sharp)]),
            ModifierNode::Aug7 => Ok(vec![
                Modifier::Mod(Degree(5), Accidental::Sharp),
                Modifier::Mod(Degree(7), Accidental::Sharp),
            ]),
            ModifierNode::Dim => Ok(vec![Modifier::Mod(Degree(5), Accidental::Flat)]),
            ModifierNode::Dim7 => Ok(vec![
                Modifier::Mod(Degree(5), Accidental::Flat),
                Modifier::Mod(Degree(7), Accidental::Flat),
            ]),
            ModifierNode::Omit(d) => Ok(vec![Modifier::Omit(d)]),
            ModifierNode::Add(d) => Ok(vec![Modifier::Add(d, Accidental::Natural)]),
            ModifierNode::Tension(a, d) => Ok(vec![Modifier::Add(d, a)]),
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

impl ChordNode {
    pub fn into_degree_node(self, key: Pitch) -> DegreeNode {
        let s = Pitch::diff(&key, &self.root);
        DegreeNode {
            root: from_semitone(s),
            modifiers: self.modifiers,
            tensions: self.tensions,
            on: self.on.map(|p| from_semitone(Pitch::diff(&key, &p))),
        }
    }

    pub fn into_chord(self, octave: u8) -> Result<Chord> {
        let mods = into_modifiers(self.modifiers)?;
        let tensions = into_modifiers(self.tensions.unwrap_or_default())?;
        let on = self
            .on
            .as_ref()
            .map(|on| vec![Modifier::OnChord(Pitch::diff(&self.root, on))])
            .unwrap_or_default();
        Ok(Chord::new(
            octave,
            self.root,
            Chord::degrees(&[mods, tensions, on].concat()),
        ))
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

impl DegreeNode {
    pub fn into_chord(self, key: Pitch, octave: u8) -> Result<Chord> {
        let (root, modifiers, tensions, on) = (self.root, self.modifiers, self.tensions, self.on);
        let s = into_semitone(root.0, root.1);
        let key = Pitch::from_u8(key.into_u8() + s);
        let on = on
            .map(|(a, d)| vec![Modifier::OnChord(into_semitone(a, d))])
            .unwrap_or(vec![]);
        let mods = [
            into_modifiers(modifiers)?,
            if let Some(tensions) = tensions {
                into_modifiers(tensions)?
            } else {
                vec![]
            },
            on,
        ]
        .concat();
        let degrees = Chord::degrees(&mods);
        Ok(Chord::new(octave, key, degrees))
    }
}

fn into_modifiers(mods: Vec<ModifierNode>) -> Result<Vec<Modifier>> {
    Ok([mods
        .into_iter()
        .map(|m| m.into_modifiers())
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()]
    .concat())
}
