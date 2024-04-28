use super::{pitch::Pitch, scale::Degree};
use std::fmt::Display;

#[derive(Debug, PartialEq, Clone)]
pub enum Key {
    Absolute(Pitch),
    // semitones
    Relative(u8),
}

impl Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Key::Absolute(pitch) => write!(f, "{}", pitch),
            Key::Relative(semitone) => write!(f, "{}", Degree::from_semitone(*semitone)),
        }
    }
}

impl Key {
    pub fn into_degree(self, key: Pitch) -> Key {
        match self {
            Key::Absolute(pitch) => Key::Relative(pitch.diff(&key)),
            Key::Relative(_) => self,
        }
    }

    pub fn into_pitch(self, pitch: Pitch) -> Key {
        match self {
            Key::Absolute(_) => self,
            Key::Relative(degree) => {
                Key::Absolute(Pitch::try_from((degree + pitch as u8) % 12).unwrap())
            }
        }
    }
}
