use crate::{chord::Chord, parser::measure_parser};
use anyhow::Result;

#[derive(Debug)]
pub struct Score {
    pub bpm: u8,
    pub chords: Vec<(Option<Chord>, u32)>,
}

#[derive(Debug, PartialEq)]
pub enum ScoreSymbol {
    Chord(Chord),
    Rest,
    Sustain,
    Repeat,
}

const MEASURE_LENGTH: u32 = 16;

impl Score {
    fn to_chords(symbols: Vec<Vec<ScoreSymbol>>) -> Result<Vec<(Option<Chord>, u32)>> {
        let mut chords = vec![];
        let mut sustain = 0;
        let mut rest = 0;
        let mut pre: Option<Chord> = None;
        for (i, measure) in symbols.into_iter().enumerate() {
            let dur: u32 = match measure.len() {
                1 | 2 | 4 | 8 | 16 => MEASURE_LENGTH / measure.len() as u32,
                _ => {
                    return Err(anyhow::anyhow!(
                        "invalid measure length: @{} {} {:?}",
                        i,
                        measure.len(),
                        measure
                    ));
                }
            };
            for symbol in measure.into_iter() {
                // println!("{:?} sus={} rest={}", symbol, sustain, rest);
                if symbol != ScoreSymbol::Sustain && sustain != 0 {
                    // println!("push {:?} sus={}", pre, sustain);
                    chords.push((pre.clone(), sustain + dur));
                    sustain = 0;
                }
                if symbol != ScoreSymbol::Rest && rest != 0 {
                    // println!("push None sus={}", rest);
                    chords.push((None, rest));
                    rest = 0;
                }
                match symbol {
                    ScoreSymbol::Chord(ref chord) => {
                        pre = Some(chord.clone());
                        sustain = dur;
                    }
                    ScoreSymbol::Repeat => {
                        sustain = dur;
                    }
                    ScoreSymbol::Sustain => {
                        sustain += dur;
                    }
                    ScoreSymbol::Rest => {
                        rest += dur;
                    }
                }
            }
        }
        // println!(
        //     "{}",
        //     chords
        //         .iter()
        //         .map(|(c, d)| format!("{:?} {}", c, d))
        //         .collect::<Vec<_>>()
        //         .join("\n")
        // );
        Ok(chords)
    }

    pub fn parse(code: &str) -> Result<Self> {
        let symbols = code
            .split("\r\n")
            .filter(|line| !line.trim().is_empty() && !line.starts_with("#"))
            .map(|line| line.split("|").collect::<Vec<_>>())
            .flatten()
            .filter(|s| !s.trim().is_empty())
            .collect::<Vec<_>>();
        // dbg!(&symbols);
        let symbols = symbols
            .iter()
            .map(|m| {
                measure_parser(m)
                    .map_err(|e| anyhow::anyhow!("{}", e))
                    .and_then(|t| {
                        if !vec![1, 2, 4, 8, 16].contains(&t.1.len()) {
                            Err(anyhow::anyhow!("{} is invalid length", m))
                        } else {
                            Ok(t.1)
                        }
                    })
            })
            .collect::<Result<Vec<_>>>()?;
        let chords = Self::to_chords(symbols)?;
        Ok(Self { bpm: 175, chords })
    }
}
