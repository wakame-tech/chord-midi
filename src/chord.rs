use crate::score::ChordNode;
use anyhow::Result;
use rust_music_theory::note::PitchClass;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Degree(pub u8);

impl Degree {
    fn to_semitone(&self) -> Result<u8> {
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

    fn diff(from: &PitchClass, to: &PitchClass) -> Self {
        let diff = (to.into_u8() as i8 - from.into_u8() as i8 + 12) % 12;
        Degree(diff as u8)
    }
}

// (degree, diff)
#[derive(Debug, PartialEq, Clone)]
pub enum Modifier {
    // ex.
    // b5 = Mod(5, -1)
    // #5 = Mod(5, 1)
    // sus2 = Mod(3, -1)
    // sus4 = Mod(3, 1)
    Mod(Degree, i8),
    // ex.
    // add9 = Add(9, 0) = [11]
    // (b9) = Add(9, -1) = [10]
    Add(Degree, i8),
    // ex.
    // omit5 = Omit(5)
    Omit(Degree),
    // root, on
    OnChord(PitchClass, PitchClass),
}

#[derive(Debug, Clone)]
pub struct Note(pub u8, pub PitchClass);

#[derive(Debug, Clone, PartialEq)]
pub struct Chord {
    octabe: u8,
    invert: u8,
    /// root note
    key: PitchClass,
    /// absolute degree from root note
    degrees: BTreeMap<Degree, i8>,
}

impl std::fmt::Display for Chord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let degrees = self
            .degrees
            .iter()
            .map(|(d, diff)| format!("{}{}", d.0, diff))
            .collect::<Vec<_>>()
            .join(",");
        write!(f, "{} {:?}", self.key, degrees)
    }
}

impl Chord {
    pub fn new(octabe: u8, key: PitchClass, degrees: BTreeMap<Degree, i8>) -> Self {
        Chord {
            octabe,
            invert: 0,
            key,
            degrees,
        }
    }

    pub fn degree_to_mods(is_minor: bool, d: Degree) -> Vec<Modifier> {
        let third = Modifier::Mod(Degree(3), if is_minor { -1 } else { 0 });
        let seventh = Modifier::Add(Degree(7), if is_minor { -1 } else { 0 });
        match d {
            Degree(5) => Ok(vec![third]),
            Degree(6) => Ok(vec![third, Modifier::Add(Degree(6), 0)]),
            Degree(7) => Ok(vec![third, seventh]),
            Degree(9) => Ok(vec![third, seventh, Modifier::Add(Degree(9), 0)]),
            _ => Err(anyhow::anyhow!("invalid degree: {:?}", d)),
        }
        .unwrap()
    }

    fn degrees(modifiers: &[Modifier]) -> BTreeMap<Degree, i8> {
        // triad
        let mut degrees = BTreeMap::from_iter(vec![(Degree(0), 0), (Degree(3), 0), (Degree(5), 0)]);
        for m in modifiers {
            match m {
                Modifier::Mod(d, i) => {
                    degrees.get_mut(d).map(|v| *v += i);
                }
                Modifier::Add(d, i) => {
                    degrees.insert(d.clone(), *i);
                }
                Modifier::Omit(d) => {
                    degrees.remove(d);
                }
                Modifier::OnChord(root, on) => {
                    let degree = Degree::diff(root, &on);
                    if let Some(i) = degrees.get_mut(&degree) {
                        *i -= 12;
                    } else {
                        degrees.insert(degree, -12);
                    };
                }
            }
        }
        degrees
    }

    pub fn from(node: ChordNode) -> Self {
        let degrees = Self::degrees(&node.modifiers);
        Chord::new(3, node.root, degrees)
    }

    pub fn notes(&self) -> Result<Vec<Note>> {
        self.degrees
            .iter()
            .map(|(d, diff)| {
                let n = ((self.octabe * 12 + self.key.into_u8() + d.to_semitone()?) as i8 + *diff)
                    as u8;
                let (octave, pitch) = (n / 12, n % 12);
                Ok(Note(octave, PitchClass::from_u8(pitch)))
            })
            .collect::<Result<Vec<_>>>()
    }
}

#[cfg(test)]
mod tests {}
