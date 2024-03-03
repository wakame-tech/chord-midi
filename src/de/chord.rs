use super::ast::{ChordNode, DegreeNode, ModifierNode};
use super::{IResult, Span};
use crate::model::degree::{Accidental, Degree, Pitch};
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::combinator::{map, opt};
use nom::error::ErrorKind;
use nom::multi::{many0, many1};
use nom::sequence::{delimited, preceded, tuple};
use nom::Slice;
use nom_regex::lib::nom::Err;
use nom_tracable::tracable_parser;
use once_cell::sync::Lazy;
use regex::Regex;
use std::str::FromStr;

static PITCH_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^([CDEFGAB][#b]?)").unwrap());

static DEGREE_NUMBER_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(3|5|6|7|9|11|13)").unwrap());

static DEGREE_NAME_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(IV|VII|VI|V|III|II|I)").unwrap());

fn capture(re: Regex) -> impl Fn(Span) -> IResult<Vec<Span>> {
    move |s| {
        if let Some(c) = re.captures(*s) {
            let v: Vec<_> = c
                .iter()
                .filter(|el| el.is_some())
                .map(|el| el.unwrap())
                .map(|m| s.slice(m.start()..m.end()))
                .collect();
            let offset = {
                let end = v.last().unwrap();
                end.as_ptr() as usize + end.len() - s.as_ptr() as usize
            };
            Ok((s.slice(offset..), v))
        } else {
            Err(Err::Error(nom::error::Error::new(
                s,
                ErrorKind::RegexpCapture,
            )))
        }
    }
}

#[tracable_parser]
fn degree_number_parser(s: Span) -> IResult<Degree> {
    map(capture(DEGREE_NUMBER_REGEX.to_owned()), |cap| {
        Degree(cap[1].parse::<u8>().unwrap())
    })(s)
}

#[tracable_parser]
fn accidental_parser(s: Span) -> IResult<Accidental> {
    map(capture(Regex::new(r"([b#+-])").unwrap()), |cap| {
        Accidental::from_str(&cap[1]).unwrap()
    })(s)
}

#[tracable_parser]
fn degree_name_parser(s: Span) -> IResult<Degree> {
    map(capture(DEGREE_NAME_REGEX.to_owned()), |cap| {
        Degree::from_str(&cap[1]).unwrap()
    })(s)
}

#[tracable_parser]
fn degree_parser(s: Span) -> IResult<(Accidental, Degree)> {
    tuple((accidental_parser, degree_name_parser))(s)
}

#[tracable_parser]
fn pitch_parser(s: Span) -> IResult<Pitch> {
    map(capture(PITCH_REGEX.to_owned()), |cap| {
        Pitch::from_str(&cap[1]).unwrap()
    })(s)
}

#[tracable_parser]
fn chord_on_chord_parser(s: Span) -> IResult<Pitch> {
    preceded(tag("/"), pitch_parser)(s)
}

#[tracable_parser]
fn degree_on_chord_parser(s: Span) -> IResult<(Accidental, Degree)> {
    preceded(tag("/"), degree_parser)(s)
}

#[tracable_parser]
fn modifier_node_parser(s: Span) -> IResult<ModifierNode> {
    alt((
        map(tag("-5"), |_| ModifierNode::Flat5th),
        map(tag("sus2"), |_| ModifierNode::Sus2),
        map(tag("sus4"), |_| ModifierNode::Sus4),
        map(tag("dim7"), |_| ModifierNode::Dim7),
        map(alt((tag("dim"), tag("o"))), |_| ModifierNode::Dim),
        map(tag("aug7"), |_| ModifierNode::Aug7),
        map(alt((tag("aug"), tag("+"))), |_| ModifierNode::Aug),
        map(tuple((tag("add"), degree_number_parser)), |(_, d)| {
            ModifierNode::Add(d)
        }),
        map(
            tuple((alt((tag("omit"), tag("no"))), degree_number_parser)),
            |(_, d)| ModifierNode::Omit(d),
        ),
        map(tag("mM7"), |_| ModifierNode::MinorMajaor7),
        map(tuple((tag("m"), opt(degree_number_parser))), |(_, d)| {
            ModifierNode::Minor(d.unwrap_or(Degree(5)))
        }),
        map(
            tuple((alt((tag("maj"), tag("M"))), opt(degree_number_parser))),
            |(_, d)| ModifierNode::Major(d.unwrap_or(Degree(5))),
        ),
    ))(s)
}

#[tracable_parser]
fn tensions_parser(s: Span) -> IResult<Vec<ModifierNode>> {
    map(
        delimited(tag("("), many1(degree_parser), tag(")")),
        |tensions| {
            tensions
                .into_iter()
                .map(|(a, d)| ModifierNode::Tension(a, d))
                .collect()
        },
    )(s)
}

#[tracable_parser]
pub fn chord_node_parser(s: Span) -> IResult<ChordNode> {
    map(
        tuple((
            pitch_parser,
            many0(modifier_node_parser),
            opt(tensions_parser),
            opt(chord_on_chord_parser),
        )),
        |(root, modifiers, tensions, on)| ChordNode {
            root,
            modifiers,
            tensions,
            on,
        },
    )(s)
}

#[tracable_parser]
pub fn degree_node_parser(s: Span) -> IResult<DegreeNode> {
    map(
        tuple((
            degree_parser,
            many0(modifier_node_parser),
            opt(tensions_parser),
            opt(degree_on_chord_parser),
        )),
        |(root, modifiers, tensions, on)| DegreeNode {
            root,
            modifiers,
            tensions,
            on,
        },
    )(s)
}
