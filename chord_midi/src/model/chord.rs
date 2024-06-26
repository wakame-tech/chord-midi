use crate::model::{
    key::Key,
    modifier::Modifier,
    scale::{Degree, Scale},
};
use anyhow::Result;
use std::{collections::BTreeSet, fmt::Debug};

#[derive(Clone, PartialEq)]
pub struct Chord {
    pub octave: u8,
    pub inversion: u8,
    pub key: Key,
    pub semitones: BTreeSet<u8>,
    pub on: Option<Key>,
}

/// returns best octave and inversion to base pitch
pub fn match_pitches(base: u8, chord: &Chord) -> Result<(u8, u8)> {
    let (mut diff, mut best_octave, mut best_inversion) = (u8::MAX, 0, 0);
    let mut chord = chord.clone();
    for octave in 0..8 {
        for inversion in 0..chord.semitones.len() as u8 {
            chord.octave = octave;
            chord.inversion = inversion;
            let chord_root = chord.root_pitch().unwrap();
            let d = base.abs_diff(chord_root);
            if d < diff {
                diff = d;
                best_octave = octave;
                best_inversion = inversion;
            }
        }
    }
    Ok((best_octave, best_inversion))
}

impl Debug for Chord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?}{}",
            self.key,
            self.on
                .as_ref()
                .map(|on| format!("/{}", on))
                .unwrap_or_default()
        )
    }
}

impl Chord {
    pub fn new(octave: u8, inversion: u8, key: Key) -> Self {
        Self {
            octave,
            inversion,
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

    pub fn modify(&mut self, modifier: &Modifier) -> Result<()> {
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
                self.semitones.remove(&self.scale().semitone(*d));
                Ok(())
            }
            Modifier::Add(d) => {
                self.semitones.insert(self.scale().semitone(*d));
                Ok(())
            }
            Modifier::Tension(Degree(d, a)) => {
                self.semitones
                    .insert(self.scale().semitone(*d) + a.clone() as u8);
                Ok(())
            }
            _ => Err(anyhow::anyhow!("unknown mod: {:?}", modifier)),
        }
    }

    pub fn root_pitch(&self) -> Result<u8> {
        let s = *self.semitones.iter().nth(self.inversion as usize).unwrap();
        match &self.key {
            Key::Absolute(p) => Ok(12 + 12 * self.octave + p.clone() as u8 + s - 1),
            Key::Relative(d) => Err(anyhow::anyhow!("relative key: {}", d)),
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
            (Key::Relative(sa), Key::Relative(sb)) => {
                let a = self.octave * 12 + *sa;
                let b = other.octave * 12 + *sb;
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

#[cfg(test)]
mod tests {
    use crate::model::{key::Key, modifier::Modifier, pitch::Pitch};

    use super::Chord;
    use anyhow::Result;
    use std::collections::BTreeSet;

    #[test]
    fn test_chord_modify() -> Result<()> {
        let mut chord = Chord::new(4, 0, Key::Absolute(Pitch::C));
        chord.modify(&Modifier::Major(5))?;
        assert_eq!(chord.semitones, BTreeSet::from_iter(vec![0, 4, 7]));
        chord.modify(&Modifier::Minor(5))?;
        assert_eq!(chord.semitones, BTreeSet::from_iter(vec![0, 3, 7]));
        chord.modify(&Modifier::Major(7))?;
        assert_eq!(chord.semitones, BTreeSet::from_iter(vec![0, 4, 7, 11]));
        Ok(())
    }

    #[test]
    fn test_modifier_multi() -> Result<()> {
        let mods = BTreeSet::from_iter(vec![Modifier::Major(5), Modifier::Aug]);
        let mut chord = Chord::new(4, 0, Key::Absolute(Pitch::F));
        for m in mods {
            chord.modify(&m)?;
        }
        assert_eq!(chord.semitones, BTreeSet::from_iter(vec![0, 4, 8]));
        Ok(())
    }
}
