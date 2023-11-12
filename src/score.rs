use crate::{
    chord::{Chord, Modifier, Quality},
    parser::measure_parser,
};
use anyhow::Result;
use rust_music_theory::note::PitchClass;

#[derive(Debug)]
pub struct Score {
    pub bpm: u8,
    pub chords: Vec<(Option<Chord>, u32)>,
}

#[derive(Debug, PartialEq)]
pub struct ChordNode {
    pub root: PitchClass,
    pub quality: Option<Quality>,
    pub number: Option<u8>,
    pub modifiers: Vec<Modifier>,
    pub on: Option<PitchClass>,
}

#[derive(Debug, PartialEq)]
pub enum ScoreNode {
    Chord(ChordNode),
    Rest,
    Sustain,
    Repeat,
}

const MEASURE_LENGTH: u32 = 16;

impl Score {
    fn to_chords(symbols: Vec<Vec<ScoreNode>>) -> Result<Vec<(Option<Chord>, u32)>> {
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
                if symbol != ScoreNode::Sustain && sustain != 0 {
                    // println!("push {:?} sus={}", pre, sustain);
                    chords.push((pre.clone(), sustain + dur));
                    sustain = 0;
                }
                if symbol != ScoreNode::Rest && rest != 0 {
                    // println!("push None sus={}", rest);
                    chords.push((None, rest));
                    rest = 0;
                }
                match symbol {
                    ScoreNode::Chord(node) => {
                        let chord = Chord::from(node)?;
                        pre = Some(chord.clone());
                        sustain = dur;
                    }
                    ScoreNode::Repeat => {
                        sustain = dur;
                    }
                    ScoreNode::Sustain => {
                        sustain += dur;
                    }
                    ScoreNode::Rest => {
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

    pub fn new(bpm: u8, s: &str) -> Result<Self> {
        let symbols = s
            .split("\r\n")
            .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
            .flat_map(|line| line.split('|').collect::<Vec<_>>())
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
        Ok(Self { bpm, chords })
    }
}
