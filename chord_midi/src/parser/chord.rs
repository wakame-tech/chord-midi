use super::ast::{ChordNode, Node};
use super::parser_util::{capture, Span};
use crate::model::key::Key;
use crate::model::modifier::Modifier;
use crate::model::pitch::{Accidental, Pitch};
use crate::model::scale::{Degree, Scale};
use anyhow::Result;
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::combinator::{map, opt};
use nom::multi::{many0, separated_list1};
use nom::sequence::{delimited, preceded, tuple};
use nom::IResult;
use nom_tracable::tracable_parser;
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::BTreeSet;
use std::str::FromStr;

pub static PITCH_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^([CDEFGAB][#b]?)").unwrap());

static DEGREE_NUMBER_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(3|5|6|7|9|11|13)").unwrap());

pub static DEGREE_NAME_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(IV|VII|VI|V|III|II|I)").unwrap());

pub static DEGREE_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(IV|VII|VI|V|III|II|I)[#b]?").unwrap());

pub fn parser_roman_num(s: &str) -> Result<u8> {
    match s {
        "I" => Ok(1),
        "II" => Ok(2),
        "III" => Ok(3),
        "IV" => Ok(4),
        "V" => Ok(5),
        "VI" => Ok(6),
        "VII" => Ok(7),
        _ => Err(anyhow::anyhow!("invalid degree: {}", s)),
    }
}

#[tracable_parser]
pub fn node_parser(s: Span) -> IResult<Span, Node> {
    alt((
        map(tag("="), |_| Node::Sustain),
        map(tag("_"), |_| Node::Rest),
        map(tag("%"), |_| Node::Repeat),
        map(tag("N.C."), |_| Node::Rest),
        map(chord_node_parser, Node::Chord),
    ))(s)
}

#[tracable_parser]
fn key_parser(s: Span) -> IResult<Span, Key> {
    alt((
        map(pitch_parser, |p| Key::Absolute(p)),
        map(degree_parser, |d| Key::Relative(d)),
    ))(s)
}

#[tracable_parser]
fn chord_node_parser(s: Span) -> IResult<Span, ChordNode> {
    map(
        tuple((
            key_parser,
            many0(modifier_parser),
            opt(tensions_parser),
            opt(preceded(tag("/"), key_parser)),
        )),
        |(key, modifiers, tensions, on)| ChordNode {
            key,
            modifiers: BTreeSet::from_iter(
                vec![Modifier::Major(5)]
                    .into_iter()
                    .chain(modifiers.into_iter())
                    .chain(tensions.into_iter().flatten()),
            ),
            on,
        },
    )(s)
}

#[tracable_parser]
fn degree_number_parser(s: Span) -> IResult<Span, u8> {
    map(capture(DEGREE_NUMBER_REGEX.to_owned()), |cap| {
        cap[1].parse::<u8>().unwrap()
    })(s)
}

#[tracable_parser]
fn degree_name_parser(s: Span) -> IResult<Span, u8> {
    map(capture(DEGREE_NAME_REGEX.to_owned()), |cap| {
        parser_roman_num(&cap[1]).unwrap()
    })(s)
}

#[tracable_parser]
fn accidental_parser(s: Span) -> IResult<Span, Accidental> {
    map(capture(Regex::new(r"^([b#])").unwrap()), |cap| {
        Accidental::from_str(&cap[1]).unwrap()
    })(s)
}

#[tracable_parser]
fn degree_parser(s: Span) -> IResult<Span, u8> {
    map(tuple((accidental_parser, degree_name_parser)), |(a, d)| {
        (Scale::Major.semitone(d) as i8 + a as i8) as u8
    })(s)
}

#[tracable_parser]
fn pitch_parser(s: Span) -> IResult<Span, Pitch> {
    map(capture(PITCH_REGEX.to_owned()), |cap| {
        Pitch::from_str(&cap[1]).unwrap()
    })(s)
}

#[tracable_parser]
fn modifier_parser(s: Span) -> IResult<Span, Modifier> {
    alt((
        map(alt((tag("-5"), tag("b5"))), |_| Modifier::Flat5th),
        map(tag("sus2"), |_| Modifier::Sus2),
        map(tag("sus4"), |_| Modifier::Sus4),
        map(tag("dim7"), |_| Modifier::Dim7),
        map(alt((tag("dim"), tag("o"))), |_| Modifier::Dim),
        map(tag("aug7"), |_| Modifier::Aug7),
        map(alt((tag("aug"), tag("+"))), |_| Modifier::Aug),
        map(tuple((tag("add"), degree_number_parser)), |(_, d)| {
            Modifier::Add(d)
        }),
        map(
            tuple((alt((tag("omit"), tag("no"))), degree_number_parser)),
            |(_, d)| Modifier::Omit(d),
        ),
        map(tag("mM7"), |_| Modifier::MinorMajaor7),
        map(
            tuple((alt((tag("maj"), tag("M"))), opt(degree_number_parser))),
            |(_, d)| Modifier::Major(d.unwrap_or(5)),
        ),
        map(tuple((tag("m"), opt(degree_number_parser))), |(_, d)| {
            Modifier::Minor(d.unwrap_or(5))
        }),
        map(
            tuple((accidental_parser, degree_number_parser)),
            |(a, d)| Modifier::Tension(Degree(d, a)),
        ),
        map(degree_number_parser, Modifier::Major),
    ))(s)
}

#[tracable_parser]
fn tensions_parser(s: Span) -> IResult<Span, Vec<Modifier>> {
    map(
        delimited(
            tag("("),
            separated_list1(
                tag(","),
                tuple((opt(accidental_parser), degree_number_parser)),
            ),
            tag(")"),
        ),
        |tensions| {
            tensions
                .into_iter()
                .map(|(a, d)| Modifier::Tension(Degree(d, a.unwrap_or(Accidental::Natural))))
                .collect()
        },
    )(s)
}

#[cfg(test)]
mod tests {
    use super::chord_node_parser;
    use anyhow::Result;
    use nom_locate::LocatedSpan;
    use nom_tracable::TracableInfo;

    fn span<'a>(s: &'a str) -> LocatedSpan<&'a str, TracableInfo> {
        LocatedSpan::new_extra(s, TracableInfo::new())
    }

    #[test]
    fn test_tentions_parser() -> Result<()> {
        for tension in ["(b9)", "(#9)", "(b5)", "(#5)", "(b13)", "(b9,#11)"] {
            let span = span(tension);
            let (res, _ast) = super::tensions_parser(span)?;
            assert_eq!(res.into_fragment(), "");
        }
        Ok(())
    }

    #[test]
    fn test_chord_node_parser() -> Result<()> {
        for chord in [
            "C",
            "Cm",
            "CmM7",
            "Csus2",
            "Csus4",
            "C-5",
            "Caug",
            "Caug7",
            "Cdim",
            "Cdim7",
            "C/D",
            "C7sus4(b9)",
            "C7(13)",
            "AbmM7/Eb",
        ] {
            let span = span(chord);
            let (res, _ast) = chord_node_parser(span)?;
            assert_eq!(res.into_fragment(), "");
        }
        Ok(())
    }
}
