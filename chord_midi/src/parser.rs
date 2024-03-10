use crate::parser_util::{capture, Span};
use crate::scale::Scale;
use crate::syntax::{Accidental, Ast, ChordNode, Degree, Key, Modifier, Node, Pitch};
use anyhow::Result;
use nom::branch::alt;
use nom::bytes::complete::{tag, take_until};
use nom::character::complete::{line_ending, space0};
use nom::combinator::{eof, map, opt, value};
use nom::multi::{many0, many1, separated_list1};
use nom::sequence::{delimited, preceded, tuple};
use nom::IResult;
use nom_locate::LocatedSpan;
use nom_tracable::tracable_parser;
use nom_tracable::TracableInfo;
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashSet;
use std::str::FromStr;

static PITCH_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^([CDEFGAB][#b]?)").unwrap());

static DEGREE_NUMBER_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(3|5|6|7|9|11|13)").unwrap());

static DEGREE_NAME_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(IV|VII|VI|V|III|II|I)").unwrap());

impl FromStr for Accidental {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "b" => Ok(Accidental::Flat),
            "#" => Ok(Accidental::Sharp),
            _ => Err(anyhow::anyhow!("invalid accidental: {}", s)),
        }
    }
}

fn parser_roman_num(s: &str) -> Result<u8> {
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

impl FromStr for Pitch {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use Pitch::*;
        match s {
            "C" => Ok(C),
            "C#" | "Db" => Ok(Cs),
            "D" => Ok(D),
            "D#" | "Eb" => Ok(Ds),
            "E" => Ok(E),
            "F" => Ok(F),
            "F#" | "Gb" => Ok(Fs),
            "G" => Ok(G),
            "G#" | "Ab" => Ok(Gs),
            "A" => Ok(A),
            "A#" | "Bb" => Ok(As),
            "B" => Ok(B),
            _ => Err(anyhow::anyhow!("invalid pitch: {}", s)),
        }
    }
}

pub fn parse(code: &str) -> Result<Ast> {
    let span = LocatedSpan::new_extra(code, TracableInfo::new());
    let (rest, ast) = ast_parser(span).map_err(|e| anyhow::anyhow!("parse error: {:?}", e))?;
    if !rest.is_empty() {
        return Err(anyhow::anyhow!("parse error: {:?}", rest));
    }
    Ok(ast)
}

#[tracable_parser]
fn ast_parser(s: Span) -> IResult<Span, Ast> {
    map(
        tuple((many1(alt((comment_parser, measure_parser))), eof)),
        |(score, _)| Ast::Score(score.into_iter().map(Box::new).collect()),
    )(s)
}

#[tracable_parser]
fn comment_parser(s: Span) -> IResult<Span, Ast> {
    map(
        delimited(tag("#"), take_until("\n"), tag("\n")),
        |comment: Span| Ast::Comment(comment.fragment().to_string()),
    )(s)
}

fn measure_end_parser(s: Span) -> IResult<Span, bool> {
    alt((
        value(false, tag("|")),
        value(true, eof),
        value(true, line_ending),
    ))(s)
}

#[tracable_parser]
fn measure_parser(s: Span) -> IResult<Span, Ast> {
    map(
        tuple((
            many1(delimited(space0, node_parser, space0)),
            measure_end_parser,
        )),
        |(nodes, br)| Ast::Measure(nodes, br),
    )(s)
}

#[tracable_parser]
fn node_parser(s: Span) -> IResult<Span, Node> {
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
            many0(modifier_node_parser),
            opt(tensions_parser),
            opt(preceded(tag("/"), key_parser)),
        )),
        |(key, modifiers, tensions, on)| ChordNode {
            key,
            modifiers: HashSet::from_iter(
                modifiers.into_iter().chain(tensions.into_iter().flatten()),
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
    map(capture(Regex::new(r"([b#])").unwrap()), |cap| {
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
fn modifier_node_parser(s: Span) -> IResult<Span, Modifier> {
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
        map(degree_number_parser, Modifier::Major),
    ))(s)
}

#[tracable_parser]
fn tensions_parser(s: Span) -> IResult<Span, Vec<Modifier>> {
    map(
        delimited(
            tag("("),
            separated_list1(tag(","), tuple((accidental_parser, degree_number_parser))),
            tag(")"),
        ),
        |tensions| {
            tensions
                .into_iter()
                .map(|(a, d)| Modifier::Tension(Degree(d, a)))
                .collect()
        },
    )(s)
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use nom_locate::LocatedSpan;
    use nom_tracable::TracableInfo;

    use crate::parser::{ast_parser, chord_node_parser, measure_parser};

    fn span(s: &str) -> LocatedSpan<&str, TracableInfo> {
        LocatedSpan::new_extra(s, TracableInfo::new())
    }

    #[test]
    fn test_chord_node_parser() -> Result<()> {
        for chord in [
            "C", "Cm", "CmM7", "Csus2", "Csus4", "C-5", "Caug", "Caug7", "Cdim", "Cdim7", "C/D",
        ] {
            let span = span(chord);
            let (res, _ast) = chord_node_parser(span)?;
            assert_eq!(res.into_fragment(), "");
        }
        Ok(())
    }

    #[test]
    fn test_measure_parser() -> Result<()> {
        for measure in ["C", "CC", "C C |", "C|", "C\n"] {
            let span = span(measure);
            let (res, _ast) = measure_parser(span)?;
            assert_eq!(res.into_fragment(), "");
        }
        Ok(())
    }

    #[test]
    fn test_ast_parser() -> Result<()> {
        for score in ["# comment\nCCC", "CCC|", "CCC\n"] {
            let span = span(score);
            let (res, _ast) = ast_parser(span)?;
            assert_eq!(res.into_fragment(), "");
        }
        Ok(())
    }
}
