use crate::chord::{semitones, Chord, Modifiers, Quality};
use crate::score::ScoreSymbol;
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::combinator::{map, opt};
use nom::error::{ContextError, ErrorKind, ParseError};
use nom::multi::{many0, many1};
use nom::sequence::{delimited, tuple};
use nom_regex::str::{re_capture, re_find};
use regex::Regex;
use rust_music_theory::note::PitchClass;

#[derive(Debug)]
pub struct DebugError {
    message: String,
}

impl ParseError<&str> for DebugError {
    // on one line, we show the error code and the input that caused it
    fn from_error_kind(input: &str, kind: ErrorKind) -> Self {
        let message = format!("{:?}:\t{:?}\n", kind, input);
        // println!("{}", message);
        DebugError { message }
    }

    // if combining multiple errors, we show them one after the other
    fn append(input: &str, kind: ErrorKind, other: Self) -> Self {
        let message = format!("{}{:?}:\t{:?}\n", other.message, kind, input);
        // println!("{}", message);
        DebugError { message }
    }

    fn from_char(input: &str, c: char) -> Self {
        let message = format!("'{}':\t{:?}\n", c, input);
        // println!("{}", message);
        DebugError { message }
    }

    fn or(self, other: Self) -> Self {
        let message = format!("1: {}\n2: {}", self.message, other.message);
        // println!("{}", message);
        DebugError { message }
    }
}

impl ContextError<&str> for DebugError {
    fn add_context(input: &str, ctx: &'static str, other: Self) -> Self {
        let message = format!("{}\"{}\":\t{:?}\n", other.message, ctx, input);
        // println!("{}", message);
        DebugError { message }
    }
}

type IResult<'a, T> = nom::IResult<&'a str, T, DebugError>;

fn pitch_parser(s: &str) -> IResult<PitchClass> {
    let pat = Regex::new(r"^([CDEFGAB])([#b]?)").unwrap();
    let (s, p) = re_capture(pat)(s)?;
    Ok((s, PitchClass::from_str(p[1..].join("").as_str()).unwrap()))
}

fn tention_parser(s: &str) -> IResult<u8> {
    let pat = Regex::new(r"^([b#]?)(\d+)").unwrap();
    let (s, p) = re_capture(pat)(s)?;
    let mut semitone = match p[2] {
        "5" => 7,
        "7" => 11,
        "9" => 14,
        "11" => 17,
        "13" => 21,
        _ => {
            return Err(nom::Err::Failure(DebugError {
                message: format!("unsupport {}", p[2]),
            }))
        }
    };
    match p[1] {
        "b" => semitone -= 1,
        "#" => semitone += 1,
        "" => {}
        _ => {
            return Err(nom::Err::Failure(DebugError {
                message: format!("unsupport {}", p[1]),
            }))
        }
    };
    Ok((s, semitone))
}

fn quality_parser(s: &str) -> IResult<Quality> {
    alt((
        map(tag("mM"), |_| Quality::MinorM7),
        map(tag("M"), |_| Quality::Major),
        map(tag("m"), |_| Quality::Minor),
        map(tag("dim"), |_| Quality::Dim),
        map(tag("aug"), |_| Quality::Aug),
        map(tag("+"), |_| Quality::Aug),
        map(tag("sus2"), |_| Quality::Sus2),
        map(tag("sus4"), |_| Quality::Sus4),
    ))(s)
}

fn number_parser(s: &str) -> IResult<u8> {
    alt((
        map(tag("6"), |_| 6),
        map(tag("7"), |_| 7),
        map(tag("9"), |_| 9),
    ))(s)
}

fn on_chord_parser(s: &str) -> IResult<PitchClass> {
    map(tuple((tag("/"), pitch_parser)), |(_, p)| p)(s)
}

fn flat5_parser(s: &str) -> IResult<Modifiers> {
    let pat = Regex::new(r"^(b5|-5)").unwrap();
    let (s, _p) = re_find(pat)(s)?;
    Ok((s, Modifiers::Flat5))
}

fn add_parser(s: &str) -> IResult<Modifiers> {
    map(tuple((tag("add"), tention_parser)), |(_, t)| {
        Modifiers::Tention(t)
    })(s)
}

fn modifiers_parser(s: &str) -> IResult<Modifiers> {
    alt((
        map(tention_parser, |t| Modifiers::Tention(t)),
        map(delimited(tag("("), tention_parser, tag(")")), |t| {
            Modifiers::Tention(t)
        }),
        flat5_parser,
        add_parser,
    ))(s)
}

pub fn chord_parser(s: &str) -> IResult<Chord> {
    print!("{} ", s);
    let (s, (pitch, quality, number, modifiers, on_chord)) = tuple((
        pitch_parser,
        opt(quality_parser),
        opt(number_parser),
        many0(modifiers_parser),
        opt(on_chord_parser),
    ))(s)?;
    println!("{} {:?} {:?} {:?}", pitch, quality, number, modifiers);
    let semitones =
        semitones(quality.unwrap_or(Quality::None), number.unwrap_or(5)).map_err(|e| {
            nom::Err::Failure(DebugError {
                message: e.to_string(),
            })
        })?;
    let mut chord = Chord::new(pitch, semitones);
    for m in modifiers.iter() {
        chord.apply(m);
    }
    if let Some(on) = on_chord {
        if chord.invert(on).is_err() {
            chord.change_root(on);
        }
    }
    Ok((s, chord))
}

pub fn opt_chord_parser(s: &str) -> IResult<ScoreSymbol> {
    alt((
        map(tag("="), |_| ScoreSymbol::Sustain),
        map(tag("_"), |_| ScoreSymbol::Rest),
        map(tag("%"), |_| ScoreSymbol::Repeat),
        map(chord_parser, |chord| ScoreSymbol::Chord(chord)),
    ))(s)
}

pub fn measure_parser(s: &str) -> IResult<Vec<ScoreSymbol>> {
    alt((
        map(tag("N.C."), |_| vec![ScoreSymbol::Rest]),
        many1(opt_chord_parser),
    ))(s)
}
