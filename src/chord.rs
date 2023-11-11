use anyhow::Result;
use rust_music_theory::{
    interval::Interval,
    note::{Note, PitchClass},
};

#[derive(Debug, Clone)]
pub enum Quality {
    None,
    Major,
    Minor,
    MinorM7,
    Dim,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Chord(u8, PitchClass, Vec<u8>);

pub fn semitones(quality: Quality, number: u8) -> Result<Vec<u8>> {
    match quality {
        // domiant 7th
        Quality::None if number == 7 => Ok(vec![4, 3, 3]),
        Quality::None | Quality::Major => match number {
            5 => Ok(vec![4, 3]),
            6 => Ok(vec![4, 3, 2]),
            7 => Ok(vec![4, 3, 4]),
            9 => Ok(vec![4, 3, 4, 3]),
            _ => Err(anyhow::anyhow!("unknown: {:?}, {}", quality, number)),
        },
        Quality::Minor => match number {
            5 => Ok(vec![3, 4]),
            6 => Ok(vec![3, 4, 2]),
            7 => Ok(vec![3, 4, 3]),
            9 => Ok(vec![3, 4, 3, 3]),
            _ => Err(anyhow::anyhow!("unknown: {:?}, {}", quality, number)),
        },
        Quality::MinorM7 => match number {
            7 => Ok(vec![3, 4, 4]),
            _ => Err(anyhow::anyhow!("unknown: {:?}, {}", quality, number)),
        },
        Quality::Dim => match number {
            5 => Ok(vec![3, 3]),
            7 => Ok(vec![3, 3, 3]),
            _ => Err(anyhow::anyhow!("unknown: {:?}, {}", quality, number)),
        },
    }
}

#[derive(Debug)]
pub enum Modifiers {
    Flat5,
    Sharp5,
    Tention(u8),
    Omit(u8),
    Sus2,
    Sus4,
    Aug,
}

impl Chord {
    pub fn new(root: PitchClass, semitones: Vec<u8>) -> Self {
        Self(4, root, semitones)
    }

    pub fn invert(&mut self, on: PitchClass) -> Result<()> {
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

    pub fn change_root(&mut self, root: PitchClass) {
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

    pub fn apply(&mut self, m: &Modifiers) {
        match m {
            Modifiers::Flat5 => {
                self.2[1] -= 1;
                if let Some(n) = self.2.get_mut(2) {
                    *n += 1;
                }
            }
            Modifiers::Aug | Modifiers::Sharp5 => {
                self.2[1] += 1;
                if let Some(n) = self.2.get_mut(2) {
                    *n -= 1;
                }
            }
            Modifiers::Sus2 => {
                self.2[0] -= 1;
                if let Some(n) = self.2.get_mut(1) {
                    *n += 1;
                }
            }
            Modifiers::Sus4 => {
                self.2[0] += 1;
                if let Some(n) = self.2.get_mut(1) {
                    *n -= 1;
                }
            }
            Modifiers::Tention(d) => {
                let i = d - self.2.iter().sum::<u8>();
                self.2.push(i);
            }
            Modifiers::Omit(d) => {
                let s = self
                    .2
                    .iter()
                    .scan(0, |state, &x| {
                        *state += x;
                        Some(*state)
                    })
                    .collect::<Vec<_>>();
                if let Some(at) = s.iter().position(|i| i == d) {
                    self.2.remove(at);
                }
            }
        }
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
    use super::Chord;
    use crate::parser::chord_parser;
    use anyhow::Result;
    use rust_music_theory::note::PitchClass;

    #[test]
    fn test_parse_chord() -> Result<()> {
        let chord = chord_parser("Dsus4")?.1;
        assert_eq!(chord, Chord(4, PitchClass::D, vec![5, 2]));

        let chord = chord_parser("CM7(9)")?.1;
        assert_eq!(chord, Chord(4, PitchClass::C, vec![4, 3, 4, 3]));

        let chord = chord_parser("D9")?.1;
        assert_eq!(chord.1, PitchClass::D);
        Ok(())
    }

    #[test]
    fn test_on_chord() -> Result<()> {
        // A: [A, C#(+4), E(+3)]
        // A/B: [B, C#(+2), E(+3)]
        let chord = chord_parser("A/B")?.1;
        assert_eq!(chord.1, PitchClass::B);
        assert_eq!(chord.2, vec![2, 3]);

        // C: [C, E(+4), G(+3)]
        // C/E: [E, G(+3), C(+5)]
        let chord = chord_parser("C/E")?.1;
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
