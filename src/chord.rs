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
pub struct Chord(u8, PitchClass, Vec<u8>);

impl std::fmt::Display for Chord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {:?}", self.1, self.2)
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

    pub fn from(node: ChordNode) -> Result<Self> {
        // println!("{} {:?} {:?} {:?}", pitch, quality, number, modifiers);
        let semitones = semitones(
            node.quality.unwrap_or(Quality::None),
            node.number.unwrap_or(5),
        )?;
        let mut chord = Chord(4, node.root, semitones);
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
            .2
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
            let s: u8 = self.2.iter().sum();
            let root = (self.1.into_u8() + self.2[0]) % 12;
            self.2.remove(0);
            self.2.push((-(s as i8)).rem_euclid(12) as u8);
            self.1 = PitchClass::from_u8(root);
        }
        Ok(())
    }

    fn change_root(&mut self, root: PitchClass) {
        let diff = (root.into_u8() as i8 + 12 - self.1.into_u8() as i8) % 12;
        let diff = if diff < self.2[0] as i8 {
            diff
        } else {
            (diff - 12) % 12
        };
        // println!("root={} on={} diff={}", self.1, root, diff);
        self.2[0] = (self.2[0] as i8 - diff) as u8;
        self.1 = root;
    }

    fn modify(&mut self, index: usize, diff: i8) {
        self.2[index] = (self.2[index] as i8 + diff) as u8;
        if let Some(n) = self.2.get_mut(index + 1) {
            *n = (*n as i8 - diff) as u8;
        }
    }

    fn omit(&mut self, index: usize) {
        let s = self.2.remove(index);
        if let Some(n) = self.2.get_mut(index) {
            *n += s;
        }
    }

    pub fn apply(&mut self, m: &Modifier) -> Result<()> {
        let cumsum = self
            .2
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
                if cumsum.iter().any(|s| s == &i) {
                    return Ok(());
                }
                let s = i - cumsum.last().unwrap();
                self.2.push(s);
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
            octave: self.0,
            pitch_class: self.1,
        };
        let intervals = Interval::from_semitones(&self.2).unwrap();
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
        assert_eq!(chord, Chord(4, PitchClass::D, vec![5, 2]));

        let chord = Chord::from(ChordNode {
            root: PitchClass::C,
            quality: Some(Quality::Major),
            number: Some(7),
            modifiers: vec![Modifier::Add(9, 0)],
            on: None,
        })?;
        assert_eq!(chord, Chord(4, PitchClass::C, vec![4, 3, 4, 3]));

        let chord = Chord::from(ChordNode {
            root: PitchClass::D,
            quality: Some(Quality::None),
            number: Some(9),
            modifiers: vec![],
            on: None,
        })?;
        assert_eq!(chord.1, PitchClass::D);

        let chord = Chord::from_str("Dm7(b5)")?;
        assert_eq!(chord.2, vec![3, 3, 5]);
        Ok(())
    }

    #[test]
    fn test_on_chord() -> Result<()> {
        // A: [A, C#(+4), E(+3)]
        // A/B: [B, C#(+2), E(+3)]
        let chord = Chord::from_str("A/B")?;
        assert_eq!(chord.1, PitchClass::B);
        assert_eq!(chord.2, vec![2, 3]);

        // C: [C, E(+4), G(+3)]
        // C/E: [E, G(+3), C(+5)]
        let chord = Chord::from_str("C/E")?;
        assert_eq!(chord.1, PitchClass::E);
        assert_eq!(chord.2, vec![3, 5]);
        Ok(())
    }

    #[test]
    fn test_invert() {
        let mut chord = Chord(4, PitchClass::C, vec![4, 3]);
        chord.invert(PitchClass::E).unwrap();
        assert_eq!(chord.1, PitchClass::E);
        assert_eq!(chord.2, vec![3, 5]);

        let mut chord = Chord(4, PitchClass::C, vec![4, 3]);
        chord.invert(PitchClass::G).unwrap();
        assert_eq!(chord.1, PitchClass::G);
        assert_eq!(chord.2, vec![5, 4])
    }

    #[test]
    fn test_change_root() {
        let mut chord = Chord(4, PitchClass::C, vec![4, 3]);
        chord.change_root(PitchClass::B);
        assert_eq!(chord.1, PitchClass::B);
        assert_eq!(chord.2, vec![5, 3]);

        let mut chord = Chord(4, PitchClass::C, vec![4, 3]);
        chord.change_root(PitchClass::F);
        assert_eq!(chord.1, PitchClass::F);
        assert_eq!(chord.2, vec![11, 3]);
    }
}
