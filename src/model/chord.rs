use super::degree::{Degree, Pitch};
use anyhow::Result;
use std::collections::BTreeMap;

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
    OnChord(Degree),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Chord {
    octabe: u8,
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
    pub fn new(octabe: u8, key: Pitch, degrees: BTreeMap<Degree, i8>) -> Self {
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

    pub fn degrees(modifiers: &[Modifier]) -> BTreeMap<Degree, i8> {
        // triad
        let mut degrees = BTreeMap::from_iter(vec![(Degree(1), 0), (Degree(3), 0), (Degree(5), 0)]);
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
                Modifier::OnChord(d) => {
                    if let Some(i) = degrees.get_mut(&d) {
                        *i -= 12;
                    } else {
                        degrees.insert(d.clone(), -12);
                    };
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
                let semitone = self.octabe as i8 * 12
                    + self.key.into_u8() as i8
                    + d.to_semitone()? as i8
                    + *diff;
                Ok(semitone as u8)
            })
            .collect::<Result<Vec<_>>>()?;
        log::debug!("{:?}", s);
        Ok(s)
    }
}

#[cfg(test)]
mod tests {}
