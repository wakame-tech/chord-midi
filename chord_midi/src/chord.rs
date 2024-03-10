use crate::{
    scale::Scale,
    syntax::{Degree, Key, Modifier},
};
use anyhow::Result;
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq)]
pub struct Chord {
    pub octave: u8,
    pub key: Key,
    pub semitones: BTreeSet<u8>,
    pub on: Option<Key>,
}

impl Chord {
    pub fn new(octave: u8, key: Key) -> Self {
        Self {
            octave,
            key,
            semitones: BTreeSet::new(),
            on: None,
        }
    }

    pub fn scale(&self) -> Scale {
        if self.semitones.contains(&Scale::Minor.semitone(3)) {
            Scale::Minor
        } else {
            Scale::Major
        }
    }

    pub fn remove_degree(&mut self, degree: u8) {
        self.semitones.remove(&Scale::Minor.semitone(degree));
        self.semitones.remove(&Scale::Major.semitone(degree));
    }

    pub fn modify(&mut self, modifier: Modifier) -> Result<()> {
        fn modify_degree(chord: &mut Chord, scale: Scale, degrees: &[u8]) -> Result<()> {
            for d in degrees {
                chord.remove_degree(*d);
                chord.semitones.insert(scale.semitone(*d));
            }
            Ok(())
        }

        match modifier {
            Modifier::Major(5) => modify_degree(self, Scale::Major, &[1, 3, 5]),
            Modifier::Major(6) => modify_degree(self, Scale::Major, &[1, 3, 5, 6]),
            Modifier::Major(7) => modify_degree(self, Scale::Major, &[1, 3, 5, 7]),
            Modifier::Major(9) => modify_degree(self, Scale::Major, &[1, 3, 5, 7, 9]),
            Modifier::Minor(5) => modify_degree(self, Scale::Minor, &[1, 3, 5]),
            Modifier::Minor(6) => modify_degree(self, Scale::Minor, &[1, 3, 5, 6]),
            Modifier::Minor(7) => modify_degree(self, Scale::Minor, &[1, 3, 5, 7]),
            Modifier::Minor(9) => modify_degree(self, Scale::Minor, &[1, 3, 5, 7, 9]),
            Modifier::MinorMajaor7 => {
                for (s, d) in [
                    (Scale::Minor, 1),
                    (Scale::Minor, 3),
                    (Scale::Minor, 5),
                    (Scale::Major, 7),
                ] {
                    self.semitones.remove(&self.scale().semitone(d));
                    self.semitones.insert(s.semitone(d));
                }
                Ok(())
            }
            Modifier::Sus2 => {
                self.semitones.remove(&self.scale().semitone(3));
                self.semitones.insert(Scale::Major.semitone(3) - 1);
                Ok(())
            }
            Modifier::Sus4 => {
                self.semitones.remove(&self.scale().semitone(3));
                self.semitones.insert(Scale::Major.semitone(3) + 1);
                Ok(())
            }
            Modifier::Flat5th => {
                self.semitones.remove(&self.scale().semitone(5));
                self.semitones.insert(Scale::Major.semitone(5) - 1);
                Ok(())
            }
            Modifier::Aug => {
                self.semitones.remove(&self.scale().semitone(5));
                self.semitones.insert(Scale::Major.semitone(5) + 1);
                Ok(())
            }
            Modifier::Aug7 => {
                self.semitones.remove(&self.scale().semitone(5));
                self.semitones.insert(Scale::Major.semitone(5) + 1);
                self.semitones.remove(&self.scale().semitone(7));
                self.semitones.insert(Scale::Major.semitone(7) + 1);
                Ok(())
            }
            Modifier::Dim => {
                self.semitones.remove(&self.scale().semitone(3));
                self.semitones.insert(Scale::Major.semitone(3) - 1);
                self.semitones.remove(&self.scale().semitone(5));
                self.semitones.insert(Scale::Major.semitone(5) - 1);
                Ok(())
            }
            Modifier::Dim7 => {
                self.semitones.remove(&self.scale().semitone(3));
                self.semitones.insert(Scale::Major.semitone(3) - 1);
                self.semitones.remove(&self.scale().semitone(5));
                self.semitones.insert(Scale::Major.semitone(5) - 1);
                self.semitones.remove(&self.scale().semitone(7));
                self.semitones.insert(Scale::Major.semitone(7) - 1);
                Ok(())
            }
            Modifier::Omit(d) => {
                self.semitones.remove(&self.scale().semitone(d));
                Ok(())
            }
            Modifier::Add(d) => {
                self.semitones.insert(self.scale().semitone(d));
                Ok(())
            }
            Modifier::Tension(Degree(d, a)) => {
                self.semitones.insert(self.scale().semitone(d) + a as u8);
                Ok(())
            }
            _ => Err(anyhow::anyhow!("unknown mod: {:?}", modifier)),
        }
    }

    /// returns edit distance of each semitone
    pub fn distance(&self, other: &Self) -> Result<usize> {
        let key_dist = match (&self.key, &other.key) {
            (Key::Absolute(ap), Key::Absolute(bp)) => {
                let a = self.octave * 12 + ap.clone() as u8;
                let b = other.octave * 12 + bp.clone() as u8;
                Ok::<_, anyhow::Error>(a.abs_diff(b))
            }
            (Key::Relative(ad), Key::Relative(bd)) => {
                let a =
                    self.octave as i8 * 12 + Scale::Major.semitone(ad.0) as i8 + ad.1.clone() as i8;
                let b = other.octave as i8 * 12
                    + Scale::Major.semitone(bd.0) as i8
                    + bd.1.clone() as i8;
                Ok(a.abs_diff(b))
            }
            _ => return Err(anyhow::anyhow!("key type mismatch")),
        }?;
        let semitones_dist = self
            .semitones
            .iter()
            .zip(other.semitones.iter())
            .map(|(a, b)| a.abs_diff(*b))
            .sum::<u8>();
        Ok((key_dist + semitones_dist) as usize)
    }
}
