use anyhow::Result;
use regex::Regex;
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

#[derive(Debug, Clone, PartialEq)]
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

    fn parse_pitch(s: &str) -> Result<(PitchClass, &str)> {
        let pat = Regex::new(r"^([CDEFGAB])([#b]?)").unwrap();
        if let Some(cap) = pat.captures(s) {
            let (d, m) = (cap.get(1).unwrap().as_str(), cap.get(2).unwrap().as_str());
            return Ok((
                PitchClass::from_regex(format!("{}{}", d, m).as_str())?.0,
                s.trim_start_matches(cap.get(0).unwrap().as_str()),
            ));
        }
        Err(anyhow::anyhow!("cannot parse \"{}\"", s))
    }

    fn parse_quality(s: &str) -> Result<(Quality, &str)> {
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
                return Ok((q.clone(), &s[m.end()..]));
            }
        }
        Ok((Quality::None, s))
    }

    fn parse_number(s: &str) -> Result<(u8, &str)> {
        let pats = vec![
            (Regex::new(r"^6").unwrap(), 6),
            (Regex::new(r"^7").unwrap(), 7),
            (Regex::new(r"^9").unwrap(), 9),
        ];
        for (pat, n) in pats.iter() {
            if let Some(m) = pat.find(s) {
                return Ok((n.clone(), &s[m.end()..]));
            }
        }
        return Ok((5, s));
    }

    fn parse_flat5(s: &str) -> Result<(bool, &str)> {
        if let Some(m) = Regex::new(r"^(b5|-5)").unwrap().find(s) {
            Ok((true, &s[m.end()..]))
        } else {
            Ok((false, s))
        }
    }

    fn parse_add9(s: &str) -> Result<(bool, &str)> {
        if let Some(m) = Regex::new(r"^add9").unwrap().find(s) {
            Ok((true, &s[m.end()..]))
        } else {
            Ok((false, s))
        }
    }

    fn parse_tentions(s: &str) -> Result<(Option<u8>, &str)> {
        let r = Regex::new(r"^\(([b#]?)(\d+)\)").unwrap();
        if let Some(cap) = r.captures(s) {
            let (a, b) = (cap.get(1).unwrap().as_str(), cap.get(2).unwrap().as_str());
            let mut semitone = match b {
                "9" => 14,
                "11" => 17,
                "13" => 21,
                _ => return Err(anyhow::anyhow!("unsupport {}", b)),
            };
            match a {
                "b" => semitone -= 1,
                "#" => semitone += 1,
                "" => {}
                _ => return Err(anyhow::anyhow!("unknown {}", a)),
            }
            return Ok((
                Some(semitone),
                s.trim_start_matches(cap.get(0).unwrap().as_str()),
            ));
        }
        Ok((None, s))
    }

    fn parse_on_chord(s: &str) -> Result<(Option<PitchClass>, &str)> {
        if let Some(cap) = Regex::new(r"^/(.+)").unwrap().captures(s) {
            let on = cap.get(1).unwrap().as_str();
            let on = PitchClass::from_str(on).ok_or(anyhow::anyhow!("invalid on chord"))?;
            return Ok((Some(on), &s[cap.get(0).unwrap().end()..]));
        }
        Ok((None, s))
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
        if s == "N.C." || s == "_" || s == "=" {
            return Ok(None);
        }
        let (root, s) = Self::parse_pitch(s)?;
        let (quality, s) = Self::parse_quality(s)?;
        let (number, s) = Self::parse_number(s)?;
        let mut semitones = Self::semitones(quality.clone(), number.clone())?;

        let (has_flat5, s) = Self::parse_flat5(s)?;
        if has_flat5 {
            semitones[1] -= 1;
        }
        let (has_add9, s) = Self::parse_add9(s)?;
        if has_add9 {
            semitones.push(14 - semitones.iter().sum::<u8>());
        }
        let (tention, s) = Self::parse_tentions(s)?;
        if let Some(tention) = tention {
            let i = tention - semitones.iter().sum::<u8>();
            semitones.push(i);
        }
        let mut chord = Chord::new(root, semitones);

        let (on, s) = Self::parse_on_chord(s)?;
        if let Some(on) = on {
            if chord.invert(on).is_err() {
                chord.change_root(on);
            }
        }
        if !s.is_empty() {
            return Err(anyhow::anyhow!("cannot parse: \"{}\"", s));
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
            .filter(|line| !line.trim().is_empty() && !line.starts_with("#"))
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
    use anyhow::Result;
    use rust_music_theory::note::PitchClass;

    #[test]
    fn test_parse_chord() -> Result<()> {
        let chord = Chord::parse("Dsus4")?;
        assert_eq!(chord, Some(Chord(4, PitchClass::D, vec![5, 2])));
        Ok(())
    }

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
