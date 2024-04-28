use std::fmt::Display;

use super::pitch::{Accidental, Pitch};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Scale {
    Major,
    Minor,
}

impl Scale {
    pub fn degrees(&self) -> Vec<u8> {
        match self {
            Scale::Major => vec![2, 2, 1, 2, 2, 2, 1],
            Scale::Minor => vec![2, 1, 2, 2, 1, 2, 2],
        }
    }

    pub fn semitone(&self, degree: u8) -> u8 {
        let s = self.degrees();
        let mut semitone = 0;
        for i in 0..degree as usize - 1 {
            semitone += s[i % 7];
        }
        semitone
    }

    pub fn semitones(&self, degrees: &[u8]) -> Vec<u8> {
        degrees.iter().map(|d| self.semitone(*d)).collect()
    }
}

fn to_roman_str(semitone: u8) -> &'static str {
    match semitone {
        0 => "I",
        1 => "I#",
        2 => "II",
        3 => "II#",
        4 => "III",
        5 => "IV",
        6 => "IV#",
        7 => "V",
        8 => "V#",
        9 => "VI",
        10 => "VI#",
        11 => "VII",
        _ => panic!("invalid semitone: {}", semitone),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Degree(pub u8, pub Accidental);

impl Display for Degree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", to_roman_str(self.0), self.1)
    }
}

impl Degree {
    pub fn from_semitone(semitone: u8) -> Self {
        match semitone {
            0 => Degree(1, Accidental::Natural),
            1 => Degree(1, Accidental::Sharp),
            2 => Degree(2, Accidental::Natural),
            3 => Degree(2, Accidental::Sharp),
            4 => Degree(3, Accidental::Natural),
            5 => Degree(4, Accidental::Natural),
            6 => Degree(4, Accidental::Sharp),
            7 => Degree(5, Accidental::Natural),
            8 => Degree(5, Accidental::Sharp),
            9 => Degree(6, Accidental::Natural),
            10 => Degree(6, Accidental::Sharp),
            11 => Degree(7, Accidental::Natural),
            _ => panic!("invalid semitone: {}", semitone),
        }
    }

    pub fn with_pitch(&self, pitch: Pitch) -> Pitch {
        let i: i8 = self.1.clone().into();
        Pitch::try_from(((pitch as u8 as i8 + self.0 as i8 + i) % 12) as u8).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::Scale;

    #[test]
    fn test_semitone() {
        assert_eq!(Scale::Major.semitone(1), 0);
        assert_eq!(Scale::Major.semitone(2), 2);
        assert_eq!(Scale::Major.semitone(3), 4);
        assert_eq!(Scale::Major.semitone(4), 5);
    }
}
