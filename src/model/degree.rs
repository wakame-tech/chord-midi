use anyhow::Result;
use std::{fmt::Display, str::FromStr};

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

impl Into<i8> for Accidental {
    fn into(self) -> i8 {
        match self {
            Accidental::Natural => 0,
            Accidental::Sharp => 1,
            Accidental::Flat => -1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Degree(pub u8);

impl FromStr for Degree {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "I" => Ok(Degree(1)),
            "II" => Ok(Degree(2)),
            "III" => Ok(Degree(3)),
            "IV" => Ok(Degree(4)),
            "V" => Ok(Degree(5)),
            "VI" => Ok(Degree(6)),
            "VII" => Ok(Degree(7)),
            _ => Err(anyhow::anyhow!("invalid degree: {}", s)),
        }
    }
}

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
            _ => unreachable!(),
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

impl FromStr for Pitch {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use Pitch::*;
        match s {
            "C" => Ok(C),
            "C#" | "Db" => Ok(Cs),
            "D" => Ok(D),
            "D#" | "Eb" => Ok(Ds),
            "E" => Ok(E),
            "F" => Ok(F),
            "F#" | "Gb" => Ok(Fs),
            "G" => Ok(G),
            "G#" | "Ab" => Ok(Gs),
            "A" => Ok(A),
            "A#" | "Bb" => Ok(As),
            "B" => Ok(B),
            _ => Err(anyhow::anyhow!("invalid pitch: {}", s)),
        }
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
    let p = d.to_semitone().unwrap() as i8;
    let a: i8 = a.into();
    (p + a) as u8
}

pub fn from_semitone(s: u8) -> (Accidental, Degree) {
    let d = match s % 12 {
        0 => Degree(1),
        2 => Degree(2),
        4 => Degree(3),
        5 => Degree(4),
        7 => Degree(5),
        9 => Degree(6),
        11 => Degree(7),
        _ => unreachable!(),
    };
    let a = match s % 12 {
        1 => Accidental::Sharp,
        3 => Accidental::Sharp,
        6 => Accidental::Flat,
        8 => Accidental::Flat,
        10 => Accidental::Flat,
        _ => Accidental::Natural,
    };
    (a, d)
}
