use crate::chord::{Modifier, Quality};
use crate::error::DebugError;
use crate::score::{ChordNode, ScoreNode};
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::one_of;
use nom::combinator::{map, opt};
use nom::multi::{many0, many1};
use nom::sequence::{delimited, tuple};
use nom_regex::str::re_capture;
use once_cell::sync::Lazy;
use regex::Regex;
use rust_music_theory::note::PitchClass;

static PITCH_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^([CDEFGAB][#b]?)").unwrap());

static MOD_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"([b#+-]?)(\d+)").unwrap());

type IResult<'a, T> = nom::IResult<&'a str, T, DebugError>;

fn decimal(s: &str) -> IResult<u8> {
    map(many1(one_of("0123456789")), |v| {
        v.iter().collect::<String>().parse::<u8>().unwrap()
    })(s)
}

fn pitch_parser(s: &str) -> IResult<PitchClass> {
    map(re_capture(PITCH_REGEX.to_owned()), |cap| {
        PitchClass::from_str(cap[1]).unwrap()
    })(s)
}

fn quality_parser(s: &str) -> IResult<Quality> {
    alt((
        map(tag("mM"), |_| Quality::MinorM7),
        map(tag("M"), |_| Quality::Major),
        map(tag("m"), |_| Quality::Minor),
        map(alt((tag("dim"), tag("o"))), |_| Quality::Dim),
        map(alt((tag("aug"), tag("+"))), |_| Quality::Aug),
    ))(s)
}

fn on_chord_parser(s: &str) -> IResult<PitchClass> {
    map(tuple((tag("/"), pitch_parser)), |(_, p)| p)(s)
}

fn degree_parser(s: &str) -> IResult<(u8, i8)> {
    map(re_capture(MOD_REGEX.to_owned()), |cap| {
        let diff = match cap[1] {
            "#" | "+" => 1,
            "b" | "-" => -1,
            "" => 0,
            _ => unreachable!(),
        };
        (cap[2].parse().unwrap(), diff)
    })(s)
}

fn modifiers_parser(s: &str) -> IResult<Modifier> {
    alt((
        map(degree_parser, |(d, diff)| Modifier::Mod(d, diff)),
        map(tag("sus2"), |_| Modifier::Mod(3, -1)),
        map(tag("sus4"), |_| Modifier::Mod(3, 1)),
        map(tuple((tag("add"), decimal)), |(_, d)| Modifier::Add(d, 0)),
        map(tuple((alt((tag("omit"), tag("no"))), decimal)), |(_, d)| {
            Modifier::Omit(d)
        }),
        // C(b9)
        map(
            delimited(
                tag("("),
                // separated_list1(tag(","), re_capture(MOD_REGEX.to_owned())),
                degree_parser,
                tag(")"),
            ),
            |(d, diff)| Modifier::Add(d, diff),
        ),
    ))(s)
}

pub fn chord_parser(s: &str) -> IResult<ChordNode> {
    map(
        tuple((
            pitch_parser,
            opt(quality_parser),
            opt(decimal),
            many0(modifiers_parser),
            opt(on_chord_parser),
        )),
        |(root, quality, number, modifiers, on)| ChordNode {
            root,
            quality,
            number,
            modifiers,
            on,
        },
    )(s)
}

pub fn score_node_parser(s: &str) -> IResult<ScoreNode> {
    alt((
        map(tag("="), |_| ScoreNode::Sustain),
        map(tag("_"), |_| ScoreNode::Rest),
        map(tag("%"), |_| ScoreNode::Repeat),
        map(chord_parser, ScoreNode::Chord),
    ))(s)
}

pub fn measure_parser(s: &str) -> IResult<Vec<ScoreNode>> {
    alt((
        map(tag("N.C."), |_| vec![ScoreNode::Rest]),
        many1(score_node_parser),
    ))(s)
}

#[cfg(test)]
mod tests {
    use super::chord_parser;
    use anyhow::Result;

    #[test]
    fn test_chord_parser() -> Result<()> {
        let chords = vec!["Ab6no5", "Dm7b5", "G7#5/B", "AbM7sus2/C"];
        for chord in chords.iter() {
            let (s, _chord) = chord_parser(chord)?;
            assert_eq!(s, "");
        }
        Ok(())
    }
}
