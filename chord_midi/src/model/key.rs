use super::{
    pitch::Pitch,
    scale::{Degree, Scale},
};
use std::fmt::Display;

#[derive(Debug, PartialEq, Clone)]
pub enum Key {
    Absolute(Pitch),
    Relative(Degree),
}

impl Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Key::Absolute(pitch) => write!(f, "{}", pitch),
            Key::Relative(degree) => write!(f, "{}", degree),
        }
    }
}

impl Key {
    pub fn into_pitch(self, tonic: Pitch, scale: Scale) -> Key {
        match self {
            Key::Absolute(_) => self,
            Key::Relative(Degree(n, acc)) => {
                let semitone = scale.semitone(n) as i8 + i8::from(acc);
                let p = (semitone + tonic as i8).rem_euclid(12) as u8;
                Key::Absolute(Pitch::try_from(p).unwrap())
            }
        }
    }

    /// Absolute -> degree. Deliberately chromatic (major/`from_semitone`-based),
    /// so it is NOT a scale-aware inverse of `into_pitch` under non-major modes.
    /// `scale` is accepted for signature symmetry but intentionally unused.
    pub fn into_degree(self, tonic: Pitch, _scale: Scale) -> Key {
        match self {
            Key::Absolute(p) => {
                let semitone = (p as i8 - tonic as i8).rem_euclid(12) as u8;
                Key::Relative(Degree::from_semitone(semitone))
            }
            Key::Relative(_) => self,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Key;
    use crate::model::pitch::{Accidental, Pitch};
    use crate::model::scale::{Degree, Scale};

    #[test]
    fn test_into_pitch_with_scale() {
        let k = Key::Relative(Degree(3, Accidental::Natural));
        assert_eq!(k.into_pitch(Pitch::C, Scale::Major), Key::Absolute(Pitch::E));
        let k = Key::Relative(Degree(3, Accidental::Natural));
        assert_eq!(k.into_pitch(Pitch::C, Scale::Dorian), Key::Absolute(Pitch::Ds));
        let k = Key::Relative(Degree(3, Accidental::Flat));
        assert_eq!(k.into_pitch(Pitch::C, Scale::Major), Key::Absolute(Pitch::Ds));
    }

    #[test]
    fn test_into_degree_roundtrip() {
        let k = Key::Absolute(Pitch::G);
        assert_eq!(
            k.into_degree(Pitch::C, Scale::Major),
            Key::Relative(Degree(5, Accidental::Natural))
        );
    }
}
