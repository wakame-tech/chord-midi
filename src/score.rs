use crate::{chord::Chord, parser::score_parser};
use anyhow::Result;

#[derive(Debug)]
pub struct Score {
    pub bpm: u8,
    pub chords: Vec<Vec<Option<Chord>>>,
}

impl Score {
    pub fn parse(code: &str) -> Result<Self> {
        let code = code
            .split("\n")
            .filter(|line| !line.trim().is_empty() && !line.starts_with("#"))
            .collect::<Vec<_>>()
            .join("\n");
        let code = code.replace("\n", "|");
        let (s, chords) = score_parser(&code)?;
        if !s.is_empty() {
            return Err(anyhow::anyhow!("cannot parse: \"{}\"", s));
        }
        Ok(Self { bpm: 180, chords })
    }
}
