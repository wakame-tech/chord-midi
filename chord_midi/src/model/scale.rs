use std::fmt::Display;

use super::pitch::{Accidental, Pitch};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Scale {
    Major,
    Minor,
    Dorian,
    Phrygian,
    Lydian,
    Mixolydian,
    Locrian,
}

impl std::str::FromStr for Scale {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> anyhow::Result<Self> {
        match s.to_lowercase().as_str() {
            "major" | "ionian" => Ok(Scale::Major),
            "minor" | "aeolian" => Ok(Scale::Minor),
            "dorian" => Ok(Scale::Dorian),
            "phrygian" => Ok(Scale::Phrygian),
            "lydian" => Ok(Scale::Lydian),
            "mixolydian" => Ok(Scale::Mixolydian),
            "locrian" => Ok(Scale::Locrian),
            _ => Err(anyhow::anyhow!("invalid scale: {}", s)),
        }
    }
}

impl Display for Scale {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Scale::Major => "major",
            Scale::Minor => "minor",
            Scale::Dorian => "dorian",
            Scale::Phrygian => "phrygian",
            Scale::Lydian => "lydian",
            Scale::Mixolydian => "mixolydian",
            Scale::Locrian => "locrian",
        };
        write!(f, "{}", s)
    }
}

impl Scale {
    pub fn degrees(&self) -> Vec<u8> {
        match self {
            Scale::Major => vec![2, 2, 1, 2, 2, 2, 1],
            Scale::Minor => vec![2, 1, 2, 2, 1, 2, 2],
            Scale::Dorian => vec![2, 1, 2, 2, 2, 1, 2],
            Scale::Phrygian => vec![1, 2, 2, 2, 1, 2, 2],
            Scale::Lydian => vec![2, 2, 2, 1, 2, 2, 1],
            Scale::Mixolydian => vec![2, 2, 1, 2, 2, 1, 2],
            Scale::Locrian => vec![1, 2, 2, 1, 2, 2, 2],
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

fn degree_to_roman(n: u8) -> &'static str {
    match n {
        1 => "I",
        2 => "II",
        3 => "III",
        4 => "IV",
        5 => "V",
        6 => "VI",
        7 => "VII",
        _ => "?",
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Degree(pub u8, pub Accidental);

impl Display for Degree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.1, degree_to_roman(self.0))
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

    #[test]
    fn test_modes() {
        assert_eq!(Scale::Dorian.semitone(3), 3);
        assert_eq!(Scale::Dorian.semitone(7), 10);
        assert_eq!(Scale::Lydian.semitone(4), 6);
        assert_eq!(Scale::Phrygian.semitone(2), 1);
    }

    #[test]
    fn test_degree_display() {
        use crate::model::pitch::Accidental;
        use crate::model::scale::Degree;
        assert_eq!(format!("{}", Degree(1, Accidental::Natural)), "I");
        assert_eq!(format!("{}", Degree(3, Accidental::Natural)), "III");
        assert_eq!(format!("{}", Degree(2, Accidental::Sharp)), "#II");
        assert_eq!(format!("{}", Degree(6, Accidental::Flat)), "bVI");
    }

    #[test]
    fn test_scale_fromstr_display() {
        use std::str::FromStr;
        assert_eq!(Scale::from_str("dorian").unwrap(), Scale::Dorian);
        assert_eq!(Scale::from_str("MAJOR").unwrap(), Scale::Major);
        assert_eq!(Scale::from_str("aeolian").unwrap(), Scale::Minor);
        assert_eq!(format!("{}", Scale::Dorian), "dorian");
        assert_eq!(format!("{}", Scale::Major), "major");
    }
}
