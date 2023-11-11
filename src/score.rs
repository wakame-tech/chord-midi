use core::num;

use anyhow::Result;
use regex::{Match, Regex};
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
    Sus2,
    Sus4,
    Aug,
    AugM7,
    Dim,
}

#[derive(Debug)]
pub struct Chord(u8, PitchClass, Vec<u8>);

impl Chord {
    fn semitones(quality: Quality, number: u8) -> Result<Vec<u8>> {
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
            Quality::Sus2 => match number {
                5 => Ok(vec![2, 5]),
                _ => Err(anyhow::anyhow!("unknown: {:?}, {}", quality, number)),
            },
            Quality::Sus4 => match number {
                5 => Ok(vec![5, 7]),
                _ => Err(anyhow::anyhow!("unknown: {:?}, {}", quality, number)),
            },
            Quality::Aug => match number {
                5 => Ok(vec![4, 4]),
                7 => Ok(vec![4, 4, 2]),
                _ => Err(anyhow::anyhow!("unknown: {:?}, {}", quality, number)),
            },
            Quality::AugM7 => Ok(vec![4, 4, 3]),
            Quality::Dim => match number {
                5 => Ok(vec![3, 3]),
                7 => Ok(vec![3, 3, 3]),
                _ => Err(anyhow::anyhow!("unknown: {:?}, {}", quality, number)),
            },
        }
    }

    fn parse_quality(s: &str) -> (Quality, Option<Match>) {
        let pats = vec![
            (Regex::new(r"^mM").unwrap(), Quality::MinorM7),
            (Regex::new(r"^M").unwrap(), Quality::Major),
            (Regex::new(r"^m").unwrap(), Quality::Minor),
            (Regex::new(r"^dim").unwrap(), Quality::Dim),
            (Regex::new(r"^(aug|\+)").unwrap(), Quality::Aug),
            (Regex::new(r"^sus2").unwrap(), Quality::Sus2),
            (Regex::new(r"^sus4").unwrap(), Quality::Sus4),
        ];
        for (pat, q) in pats.iter() {
            if let Some(m) = pat.find(s) {
                return (q.clone(), Some(m));
            }
        }
        return (Quality::None, None);
    }

    fn parse_number(s: &str) -> (u8, Option<Match>) {
        let pats = vec![
            (Regex::new(r"^6").unwrap(), 6),
            (Regex::new(r"^7").unwrap(), 7),
            (Regex::new(r"^9").unwrap(), 9),
        ];
        for (pat, n) in pats.iter() {
            if let Some(m) = pat.find(s) {
                return (n.clone(), Some(m));
            }
        }
        return (5, None);
    }

    pub fn new(root: PitchClass, semitones: Vec<u8>) -> Self {
        Self(4, root, semitones)
    }

    fn invert(&mut self, on: PitchClass) -> Result<()> {
        let s = self
            .2
            .iter()
            .scan(0, |state, &x| {
                *state = *state + x;
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

    pub fn parse(s: &str) -> Result<Option<Self>> {
        if s == "N.C." || s == "_" {
            return Ok(None);
        }
        let (root, m) = PitchClass::from_regex(s)?;
        let s = &s[m.end()..];
        let (quality, m) = Self::parse_quality(s);
        let s = m.map_or(s, |m| &s[m.end()..]);
        let (number, m) = Self::parse_number(s);
        let mut s = m.map_or(s, |m| &s[m.end()..]);
        let mut semitones = Self::semitones(quality.clone(), number.clone())?;

        if s.contains("b5") {
            semitones[1] -= 1;
            s = s.trim_start_matches("b5");
        }
        if s.contains("add9") {
            semitones.push(14 - semitones.last().unwrap());
            s = s.trim_start_matches("add9");
        }

        let r = Regex::new(r"^\(([b#]?)(\d+)\)").unwrap();
        if let Some(cap) = r.captures(s) {
            let (a, b) = (cap.get(1).unwrap().as_str(), cap.get(2).unwrap().as_str());
            let d = match b {
                "9" => 14,
                "11" => 17,
                "13" => 21,
                _ => return Err(anyhow::anyhow!("unsupport {}", b)),
            };
            let mut i = d - semitones.last().unwrap();
            match a {
                "b" => i -= 1,
                "#" => i += 1,
                "" => {}
                _ => return Err(anyhow::anyhow!("unknown {}", a)),
            }
            semitones.push(i);
            s = s.trim_start_matches(cap.get(0).unwrap().as_str());
        }
        let mut chord = Chord::new(root, semitones);
        // on chord
        if s.starts_with("/") {
            let on = s.trim_start_matches("/");
            let on = PitchClass::from_str(on).ok_or(anyhow::anyhow!("invalid on chord"))?;
            // inversion
            if chord.invert(on).is_err() {
                chord.change_root(on);
            }
            s = "";
        }
        if !s.is_empty() {
            return Err(anyhow::anyhow!("cannot parse :{}", s));
        }
        println!(
            "{} {:?} {} s={}, semitones={:?}",
            root, quality, number, s, chord.2
        );
        Ok(Some(chord))
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

#[derive(Debug)]
pub struct Score {
    pub bpm: u8,
    pub chords: Vec<Vec<Option<Chord>>>,
}

impl Score {
    pub fn parse(code: &str) -> Result<Self> {
        let chords = code
            .split(|c| c == '|' || c == '\n')
            .map(|s| {
                s.trim()
                    .split(" ")
                    .map(|c| Chord::parse(c))
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { bpm: 180, chords })
    }
}

#[cfg(test)]
mod tests {
    use super::Chord;
    use rust_music_theory::note::PitchClass;

    #[test]
    fn test_on_chord() {
        // A: [A, C#(+4), E(+3)]
        // A/B: [B, C#(+2), E(+3)]
        let chord = Chord::parse("A/B").unwrap().unwrap();
        assert_eq!(chord.1, PitchClass::B);
        assert_eq!(chord.2, vec![2, 3]);

        // C: [C, E(+4), G(+3)]
        // C/E: [E, G(+3), C(+5)]
        let chord = Chord::parse("C/E").unwrap().unwrap();
        assert_eq!(chord.1, PitchClass::E);
        assert_eq!(chord.2, vec![3, 5]);
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
