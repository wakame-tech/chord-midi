use crate::score::ChordNode;
use anyhow::Result;
use rust_music_theory::{
    interval::Interval,
    note::{Note, PitchClass},
};

#[derive(Debug, Clone, PartialEq)]
pub enum Quality {
    None,
    Major,
    Minor,
    MinorM7,
    Dim,
    Aug,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Chord {
    octabe: u8,
    // root note
    key: PitchClass,
    // relative semitones
    semitones: Vec<u8>,
}

impl std::fmt::Display for Chord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {:?}", self.key, self.semitones)
    }
}

fn semitones(quality: Quality, number: u8) -> Result<Vec<u8>> {
    match quality {
        // domiant 7th
        Quality::None if number == 7 => Ok(vec![4, 3, 3]),
        Quality::None | Quality::Major => match number {
            // C
            5 => Ok(vec![4, 3]),
            // C6
            6 => Ok(vec![4, 3, 2]),
            // CM7
            7 => Ok(vec![4, 3, 4]),
            // C9
            9 => Ok(vec![4, 3, 4, 3]),
            _ => Err(anyhow::anyhow!("unknown: {:?}, {}", quality, number)),
        },
        Quality::Minor => match number {
            // Cm
            5 => Ok(vec![3, 4]),
            // Cm6
            6 => Ok(vec![3, 4, 2]),
            // Cm7
            7 => Ok(vec![3, 4, 3]),
            // Cm9
            9 => Ok(vec![3, 4, 3, 3]),
            _ => Err(anyhow::anyhow!("unknown: {:?}, {}", quality, number)),
        },
        Quality::MinorM7 => match number {
            // CmM7
            7 => Ok(vec![3, 4, 4]),
            _ => Err(anyhow::anyhow!("unknown: {:?}, {}", quality, number)),
        },
        Quality::Dim => match number {
            // Cdim
            5 => Ok(vec![3, 3]),
            // Cdim7
            7 => Ok(vec![3, 3, 3]),
            _ => Err(anyhow::anyhow!("unknown: {:?}, {}", quality, number)),
        },
        Quality::Aug => match number {
            // Caug
            5 => Ok(vec![3, 5]),
            // Caug7
            7 => Ok(vec![3, 5, 3]),
            _ => Err(anyhow::anyhow!("unknown: {:?}, {}", quality, number)),
        },
    }
}

fn to_semitone(d: u8) -> Result<u8> {
    // TODO: hontou?
    match d {
        3 => Ok(4),
        5 => Ok(7),
        7 => Ok(11),
        9 => Ok(14),
        11 => Ok(17),
        13 => Ok(21),
        _ => Err(anyhow::anyhow!("unknown degree {}", d)),
    }
}

// (degree, diff)
#[derive(Debug, PartialEq)]
pub enum Modifier {
    // ex.
    // b5 = Mod(5, -1)
    // #5 = Mod(5, 1)
    // sus2 = Mod(3, -1)
    // sus4 = Mod(3, 1)
    Mod(u8, i8),
    // ex.
    // add9 = Add(9, 0) = [11]
    // (b9) = Add(9, -1) = [10]
    Add(u8, i8),
    // ex.
    // omit5 = Omit(5)
    Omit(u8),
}

impl Chord {
    #[cfg(test)]
    pub fn from_str(s: &str) -> Result<Self> {
        use crate::parser::chord_parser;
        use nom_locate::LocatedSpan;
        use nom_tracable::TracableInfo;

        let info = TracableInfo::new();
        let span = LocatedSpan::new_extra(s, info);
        let (rest, node) =
            chord_parser(span).map_err(|e| anyhow::anyhow!("Failed to parse: {}", e))?;
        if !rest.is_empty() {
            return Err(anyhow::anyhow!("cannot parse: {} rest={}", s, rest));
        }
        Self::from(node)
    }

    pub fn new(octabe: u8, key: PitchClass, semitones: Vec<u8>) -> Self {
        Chord {
            octabe,
            key,
            semitones,
        }
    }

    pub fn from(node: ChordNode) -> Result<Self> {
        // println!("{} {:?} {:?} {:?}", pitch, quality, number, modifiers);
        let semitones = semitones(
            node.quality.unwrap_or(Quality::None),
            node.number.unwrap_or(5),
        )?;
        let mut chord = Chord {
            octabe: 3,
            key: node.root,
            semitones,
        };
        for m in node.modifiers.iter() {
            chord.apply(m)?;
        }
        if let Some(on) = node.on {
            if chord.invert(on).is_err() {
                chord.change_root(on);
            }
        }
        Ok(chord)
    }

    fn invert(&mut self, on: PitchClass) -> Result<()> {
        let s = self
            .semitones
            .iter()
            .scan(0, |state, &x| {
                *state += x;
                Some(*state)
            })
            .collect::<Vec<_>>();
        let count = s
            .iter()
            .position(|i| i == &on.into_u8())
            .ok_or(anyhow::anyhow!("on must contain chord"))?;
        for _ in 0..=count {
            // 0 [4, 3] -> 0 [3, 12 - 3 - 4] -> 0 [3, 5]
            let s: u8 = self.semitones.iter().sum();
            let root = (self.key.into_u8() + self.semitones[0]) % 12;
            self.semitones.remove(0);
            self.semitones.push((-(s as i8)).rem_euclid(12) as u8);
            self.key = PitchClass::from_u8(root);
        }
        Ok(())
    }

    fn change_root(&mut self, root: PitchClass) {
        let diff = (root.into_u8() as i8 + 12 - self.key.into_u8() as i8) % 12;
        let diff = if diff < self.semitones[0] as i8 {
            diff
        } else {
            (diff - 12) % 12
        };
        // println!("root={} on={} diff={}", self.1, root, diff);
        self.semitones[0] = (self.semitones[0] as i8 - diff) as u8;
        self.key = root;
    }

    fn modify(&mut self, index: usize, diff: i8) {
        self.semitones[index] = (self.semitones[index] as i8 + diff) as u8;
        if let Some(n) = self.semitones.get_mut(index + 1) {
            *n = (*n as i8 - diff) as u8;
        }
    }

    fn omit(&mut self, index: usize) {
        let s = self.semitones.remove(index);
        if let Some(n) = self.semitones.get_mut(index) {
            *n += s;
        }
    }

    pub fn apply(&mut self, m: &Modifier) -> Result<()> {
        let cumsum = self
            .semitones
            .iter()
            .scan(0, |state, &x| {
                *state += x;
                Some(*state)
            })
            .collect::<Vec<_>>();

        match m {
            Modifier::Mod(d, diff) => {
                let i = to_semitone(*d)?;
                if let Some(at) = cumsum.iter().position(|s| s == &i) {
                    self.modify(at, *diff);
                }
            }
            Modifier::Add(d, diff) => {
                let i = (to_semitone(*d)? as i8 + diff) as u8;
                log::debug!("{} {:?} i={} cumsum={:?}", self, m, i, cumsum);
                let s = i - cumsum.last().unwrap();
                self.semitones.push(s);
            }
            Modifier::Omit(d) => {
                let i = to_semitone(*d)?;
                if let Some(at) = cumsum.iter().position(|s| s == &i) {
                    self.omit(at);
                }
            }
        }
        Ok(())
    }

    pub fn notes(&self) -> Vec<Note> {
        let root_note = Note {
            octave: self.octabe,
            pitch_class: self.key,
        };
        let intervals = Interval::from_semitones(&self.semitones).unwrap();
        Interval::to_notes(root_note, intervals)
    }
}

#[cfg(test)]
mod tests {
    use super::{Chord, Modifier};
    use crate::{chord::Quality, score::ChordNode};
    use anyhow::Result;
    use rust_music_theory::note::PitchClass;

    #[test]
    fn test_chord_from() -> Result<()> {
        let chord = Chord::from(ChordNode {
            root: PitchClass::D,
            quality: None,
            number: None,
            modifiers: vec![Modifier::Mod(3, 1)],
            on: None,
        })?;
        assert_eq!(chord, Chord::new(4, PitchClass::D, vec![5, 2]));

        let chord = Chord::from(ChordNode {
            root: PitchClass::C,
            quality: Some(Quality::Major),
            number: Some(7),
            modifiers: vec![Modifier::Add(9, 0)],
            on: None,
        })?;
        assert_eq!(chord, Chord::new(4, PitchClass::C, vec![4, 3, 4, 3]));

        let chord = Chord::from(ChordNode {
            root: PitchClass::D,
            quality: Some(Quality::None),
            number: Some(9),
            modifiers: vec![],
            on: None,
        })?;
        assert_eq!(chord.key, PitchClass::D);

        let chord = Chord::from_str("Dm7(b5)")?;
        assert_eq!(chord.semitones, vec![3, 3, 4]);
        Ok(())
    }

    #[test]
    fn test_on_chord() -> Result<()> {
        // A: [A, C#(+4), E(+3)]
        // A/B: [B, C#(+2), E(+3)]
        let chord = Chord::from_str("A/B")?;
        assert_eq!(chord.key, PitchClass::B);
        assert_eq!(chord.semitones, vec![2, 3]);

        // C: [C, E(+4), G(+3)]
        // C/E: [E, G(+3), C(+5)]
        let chord = Chord::from_str("C/E")?;
        assert_eq!(chord.key, PitchClass::E);
        assert_eq!(chord.semitones, vec![3, 5]);
        Ok(())
    }

    #[test]
    fn test_invert() {
        let mut chord = Chord::new(4, PitchClass::C, vec![4, 3]);
        chord.invert(PitchClass::E).unwrap();
        assert_eq!(chord.key, PitchClass::E);
        assert_eq!(chord.semitones, vec![3, 5]);

        let mut chord = Chord::new(4, PitchClass::C, vec![4, 3]);
        chord.invert(PitchClass::G).unwrap();
        assert_eq!(chord.key, PitchClass::G);
        assert_eq!(chord.semitones, vec![5, 4])
    }

    #[test]
    fn test_change_root() {
        let mut chord = Chord::new(4, PitchClass::C, vec![4, 3]);
        chord.change_root(PitchClass::B);
        assert_eq!(chord.key, PitchClass::B);
        assert_eq!(chord.semitones, vec![5, 3]);

        let mut chord = Chord::new(4, PitchClass::C, vec![4, 3]);
        chord.change_root(PitchClass::F);
        assert_eq!(chord.key, PitchClass::F);
        assert_eq!(chord.semitones, vec![11, 3]);
    }
}
