use anyhow::Result;
use std::collections::BTreeMap;

use crate::syntax::{Accidental, Degree, Pitch};

// (degree, diff)
#[derive(Debug, PartialEq, Clone)]
pub enum Modifier {
    // ex.
    // b5 = Mod(5, -1)
    // #5 = Mod(5, 1)
    // sus2 = Mod(3, -1)
    // sus4 = Mod(3, 1)
    Mod(Degree, Accidental),
    // ex.
    // add9 = Add(9, 0) = [11]
    // (b9) = Add(9, -1) = [10]
    Add(Degree, Accidental),
    // ex.
    // omit5 = Omit(5)
    Omit(Degree),
    // semitones from root
    OnChord(u8),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Chord {
    octave: u8,
    invert: u8,
    /// root note
    key: Pitch,
    /// absolute degree from root note
    degrees: BTreeMap<Degree, i8>,
}

impl std::fmt::Display for Chord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let degrees = self
            .degrees
            .iter()
            .map(|(d, i)| format!("{}({})", d.0, i))
            .collect::<Vec<_>>()
            .join(",");
        write!(f, "{:?} {}", self.key, degrees)
    }
}

impl Chord {
    pub fn set_nearest_octave(&mut self, pre: &Chord) {
        self.octave = [-1, 0, 1]
            .into_iter()
            .map(|o| Chord {
                octave: (pre.octave as i8 + o) as u8,
                ..self.clone()
            })
            .min_by_key(|c| c.distance(pre).unwrap())
            .unwrap()
            .octave;
    }

    pub fn new(octave: u8, key: Pitch, degrees: BTreeMap<Degree, i8>) -> Self {
        Chord {
            octave,
            invert: 0,
            key,
            degrees,
        }
    }

    pub fn degrees(modifiers: &[Modifier]) -> BTreeMap<Degree, i8> {
        // triad
        let mut degrees = BTreeMap::from_iter(vec![(Degree(1), 0), (Degree(3), 0), (Degree(5), 0)]);
        for m in modifiers {
            match m {
                Modifier::Mod(d, i) => {
                    let s: i8 = i.clone().into();
                    if let Some(v) = degrees.get_mut(d) {
                        *v += s;
                    }
                }
                Modifier::Add(d, i) => {
                    degrees.insert(d.clone(), i.clone().into());
                }
                Modifier::Omit(d) => {
                    degrees.remove(d);
                }
                // TODO
                Modifier::OnChord(s) => {
                    log::warn!("OnChord is not implemented yet: {}", s);
                }
            }
        }
        degrees
    }

    pub fn semitones(&self) -> Result<Vec<u8>> {
        let s = self
            .degrees
            .iter()
            .map(|(d, diff)| {
                let semitone =
                    self.octave as i8 * 12 + self.key.into_u8() as i8 + d.to_semitone()? + *diff;
                Ok(semitone as u8)
            })
            .collect::<Result<Vec<_>>>()?;
        log::debug!("{:?}", s);
        Ok(s)
    }

    /// returns edit distance of each semitone
    pub fn distance(&self, other: &Self) -> Result<usize> {
        let s1 = self.semitones()?;
        let s2 = other.semitones()?;
        let d = s1
            .into_iter()
            .zip(s2)
            .map(|(a, b)| (a as i8 - b as i8).unsigned_abs() as usize)
            .sum();
        Ok(d)
    }
}

#[cfg(test)]
mod tests {}