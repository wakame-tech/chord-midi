use anyhow::Result;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Degree(pub u8);

impl Degree {
    pub fn to_semitone(&self) -> Result<u8> {
        match self.0 {
            3 => Ok(4),
            5 => Ok(7),
            7 => Ok(11),
            9 => Ok(14),
            11 => Ok(17),
            13 => Ok(21),
            _ => Err(anyhow::anyhow!("unknown degree {}", self.0)),
        }
    }

    pub fn diff(from: &Pitch, to: &Pitch) -> Self {
        let diff = (to.into_u8() as i8 - from.into_u8() as i8 + 12) % 12;
        Degree(diff as u8)
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
            "C#" => Ok(Cs),
            "D" => Ok(D),
            "D#" => Ok(Ds),
            "E" => Ok(E),
            "F" => Ok(F),
            "F#" => Ok(Fs),
            "G" => Ok(G),
            "G#" => Ok(Gs),
            "A" => Ok(A),
            "A#" => Ok(As),
            "B" => Ok(B),
            _ => Err(anyhow::anyhow!("invalid pitch: {}", s)),
        }
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
}
