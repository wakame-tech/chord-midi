use anyhow::Result;
use std::{fmt::Display, str::FromStr};

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

impl FromStr for Pitch {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use Pitch::*;
        match s {
            "C" => Ok(C),
            "C#" | "Db" => Ok(Cs),
            "D" => Ok(D),
            "D#" | "Eb" => Ok(Ds),
            "E" | "Fb" => Ok(E),
            "F" | "E#" => Ok(F),
            "F#" | "Gb" => Ok(Fs),
            "G" => Ok(G),
            "G#" | "Ab" => Ok(Gs),
            "A" => Ok(A),
            "A#" | "Bb" => Ok(As),
            "B" => Ok(B),
            "B#" | "Cb" => Ok(Cs),
            _ => Err(anyhow::anyhow!("invalid pitch: {}", s)),
        }
    }
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
    pub fn diff(&self, other: &Self) -> u8 {
        let a = *self as i8;
        let b = *other as i8;
        (a - b + 12) as u8 % 12
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
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

impl FromStr for Accidental {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "b" => Ok(Accidental::Flat),
            "#" => Ok(Accidental::Sharp),
            _ => Err(anyhow::anyhow!("invalid accidental: {}", s)),
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
