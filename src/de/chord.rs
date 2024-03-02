use super::ast::{ChordNode, DegreeNode};
use super::{IResult, Span};
use crate::model::chord::{Chord, Modifier};
use crate::model::degree::{Degree, Pitch};
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::combinator::{map, opt};
use nom::error::ErrorKind;
use nom::multi::many0;
use nom::sequence::{delimited, tuple};
use nom::Slice;
use nom_regex::lib::nom::Err;
use nom_tracable::tracable_parser;
use once_cell::sync::Lazy;
use regex::Regex;
use std::str::FromStr;

static PITCH_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^([CDEFGAB][#b]?)").unwrap());

static TENSION_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^([b#+-]?)(\d+)").unwrap());

static DEGREE_NUMBER_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(3|5|6|7|9|11|13)").unwrap());

static DEGREE_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(IV|VII|VI|V|III|II|I)").unwrap());

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
fn degree_parser(s: Span) -> IResult<Degree> {
    map(capture(DEGREE_REGEX.to_owned()), |cap| {
        Degree::from_str(&cap[1]).unwrap()
    })(s)
}

#[tracable_parser]
fn pitch_parser(s: Span) -> IResult<Pitch> {
    map(capture(PITCH_REGEX.to_owned()), |cap| {
        Pitch::from_str(&cap[1]).unwrap()
    })(s)
}

#[tracable_parser]
fn on_chord_parser(s: Span) -> IResult<Pitch> {
    map(tuple((tag("/"), pitch_parser)), |(_, p)| p)(s)
}

#[tracable_parser]
fn on_chord_degree_parser(s: Span) -> IResult<Degree> {
    map(tuple((tag("/"), degree_parser)), |(_, d)| d)(s)
}

#[tracable_parser]
fn tension_parser(s: Span) -> IResult<(Degree, i8)> {
    map(capture(TENSION_REGEX.to_owned()), |cap| {
        let (d, i) = (cap[2].parse().unwrap(), cap[1].into_fragment());
        let diff = match i {
            "#" | "+" => 1,
            "b" | "-" => -1,
            "" => 0,
            _ => unreachable!(),
        };
        (Degree(d), diff)
    })(s)
}

#[tracable_parser]
fn modifiers_parser(s: Span) -> IResult<Vec<Modifier>> {
    alt((
        map(alt((tag("-5"), tag("(b5)"))), |_| {
            vec![Modifier::Mod(Degree(5), -1)]
        }),
        map(tag("sus2"), |_| vec![Modifier::Mod(Degree(3), -1)]),
        map(tag("sus4"), |_| vec![Modifier::Mod(Degree(3), 1)]),
        map(tag("dim7"), |_| {
            vec![Modifier::Mod(Degree(5), -1), Modifier::Add(Degree(7), -1)]
        }),
        map(alt((tag("dim"), tag("o"))), |_| {
            vec![Modifier::Mod(Degree(5), -1)]
        }),
        map(tag("aug7"), |_| {
            vec![Modifier::Mod(Degree(5), 1), Modifier::Add(Degree(7), 1)]
        }),
        map(alt((tag("aug"), tag("+"))), |_| {
            vec![Modifier::Mod(Degree(5), 1)]
        }),
        map(tuple((tag("add"), degree_number_parser)), |(_, d)| {
            vec![Modifier::Add(d, 0)]
        }),
        map(
            tuple((alt((tag("omit"), tag("no"))), degree_number_parser)),
            |(_, d)| vec![Modifier::Omit(d)],
        ),
        map(delimited(tag("("), tension_parser, tag(")")), |(d, i)| {
            vec![Modifier::Add(d, i)]
        }),
        map(tension_parser, |(d, i)| vec![Modifier::Add(d, i)]),
        map(tag("mM7"), |_| {
            vec![Modifier::Mod(Degree(3), -1), Modifier::Add(Degree(7), 0)]
        }),
        map(
            tuple((
                alt((tag("maj"), tag("m"), tag("M"))),
                opt(degree_number_parser),
            )),
            |(m, d)| {
                let is_minor = m.into_fragment() == "m";
                Chord::degree_to_mods(is_minor, d.unwrap_or(Degree(5)))
            },
        ),
    ))(s)
}

#[tracable_parser]
pub fn chord_node_parser(s: Span) -> IResult<ChordNode> {
    map(
        tuple((pitch_parser, many0(modifiers_parser), opt(on_chord_parser))),
        |(root, modifiers, on)| ChordNode {
            root,
            modifiers: vec![
                modifiers.into_iter().flatten().collect(),
                if let Some(p) = on {
                    let s = Pitch::diff(&root, &p);
                    let (d, _) = Degree::from_semitone(s);
                    vec![Modifier::OnChord(d)]
                } else {
                    vec![]
                },
            ]
            .concat(),
        },
    )(s)
}

#[tracable_parser]
pub fn degree_node_parser(s: Span) -> IResult<DegreeNode> {
    map(
        tuple((
            degree_parser,
            many0(modifiers_parser),
            opt(on_chord_degree_parser),
            opt(tag(" ")),
        )),
        |(root, modifiers, on, _)| DegreeNode {
            root,
            modifiers: vec![
                modifiers.into_iter().flatten().collect(),
                if let Some(d) = on {
                    vec![Modifier::OnChord(d)]
                } else {
                    vec![]
                },
            ]
            .concat(),
        },
    )(s)
}

#[cfg(test)]
mod tests {
    use crate::de::chord::modifiers_parser;
    use anyhow::Result;
    use nom::multi::many0;
    use nom_locate::LocatedSpan;
    use nom_tracable::TracableInfo;

    #[test]
    fn test_modifiers_parser() -> Result<()> {
        for lit in [
            "", "6", "7", "maj7", "M7", "m", "m6", "m7", "mM7", "m7-5", "dim", "sus4", "7sus4",
            "add9", "9", "m9", "7(b9)", "7(#9)", "maj9", "7(#11)", "7(13)", "7(b13)",
        ] {
            let info = TracableInfo::new();
            let (s, mods) = many0(modifiers_parser)(LocatedSpan::new_extra(lit, info))?;
            assert_eq!(*s, "");
            let mods = mods.into_iter().flatten().collect::<Vec<_>>();
            println!("\"{}\" -> {:?}", lit, mods);
        }
        Ok(())
    }
}
