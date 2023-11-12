use crate::{
    chord::{Chord, Modifier, Quality},
    parser::measure_parser,
};
use anyhow::Result;
use nom_locate::LocatedSpan;
use nom_tracable::TracableInfo;
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
    fn to_chords(nodes_list: Vec<Vec<ScoreNode>>) -> Result<Vec<(Option<Chord>, u32)>> {
        let mut chords = vec![];
        let mut sustain = 0;
        let mut rest = 0;
        let mut pre: Option<Chord> = None;
        for (i, nodes) in nodes_list.into_iter().enumerate() {
            let dur: u32 = match nodes.len() {
                1 | 2 | 4 | 8 | 16 => MEASURE_LENGTH / nodes.len() as u32,
                _ => {
                    return Err(anyhow::anyhow!(
                        "invalid measure length: @{} {} {:?}",
                        i,
                        nodes.len(),
                        nodes
                    ));
                }
            };
            log::trace!("{:?} len={} dur={}", nodes, nodes.len(), dur);
            for node in nodes.into_iter() {
                log::trace!("{:?} sus={} rest={}", node, sustain, rest);
                if node != ScoreNode::Sustain && sustain != 0 {
                    log::trace!("push {:?} sus={}", pre, sustain);
                    chords.push((pre.clone(), sustain));
                    sustain = 0;
                }
                if node != ScoreNode::Rest && rest != 0 {
                    log::trace!("push None sus={}", rest);
                    chords.push((None, rest));
                    rest = 0;
                }
                match node {
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
        log::trace!(
            "{}",
            chords
                .iter()
                .map(|(c, d)| format!("{:?} {}", c, d))
                .collect::<Vec<_>>()
                .join("\n")
        );
        Ok(chords)
    }

    pub fn new(bpm: u8, s: &str) -> Result<Self> {
        let symbols = s
            .split("\r\n")
            .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
            .flat_map(|line| line.split('|').collect::<Vec<_>>())
            .filter(|s| !s.trim().is_empty())
            .collect::<Vec<_>>();
        let symbols = symbols
            .iter()
            .map(|m| {
                let info = TracableInfo::new().forward(true);
                let span = LocatedSpan::new_extra(*m, info);

                let (s, nodes) = measure_parser(span).map_err(|e| anyhow::anyhow!("{}", e))?;

                if !s.is_empty() {
                    return Err(anyhow::anyhow!("cannot parse {} rest={}", m, s));
                }
                if !vec![1, 2, 4, 8, 16].contains(&nodes.len()) {
                    return Err(anyhow::anyhow!("{} is invalid length: {}", m, nodes.len()));
                }

                nom_tracable::histogram();
                nom_tracable::cumulative_histogram();

                Ok(nodes)
            })
            .collect::<Result<Vec<_>>>()?;
        let chords = Self::to_chords(symbols)?;
        Ok(Self { bpm, chords })
    }
}
