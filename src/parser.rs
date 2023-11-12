use crate::chord::{Modifier, Quality};
use crate::score::{ChordNode, ScoreNode};
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::one_of;
use nom::combinator::{map, opt};
use nom::error::ErrorKind;
use nom::multi::{many0, many1};
use nom::sequence::{delimited, tuple};
use nom::Slice;
use nom_locate::LocatedSpan;
use nom_regex::lib::nom::Err;
use nom_tracable::{tracable_parser, TracableInfo};
use once_cell::sync::Lazy;
use regex::Regex;
use rust_music_theory::note::PitchClass;

static PITCH_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^([CDEFGAB][#b]?)").unwrap());

static MOD_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^([b#+-]?)(\d+)").unwrap());

type Span<'a> = LocatedSpan<&'a str, TracableInfo>;
type IResult<'a, T> = nom::IResult<Span<'a>, T>;

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
fn decimal(s: Span) -> IResult<u8> {
    map(many1(one_of("0123456789")), |v| {
        v.iter().collect::<String>().parse::<u8>().unwrap()
    })(s)
}

#[tracable_parser]
fn pitch_parser(s: Span) -> IResult<PitchClass> {
    map(capture(PITCH_REGEX.to_owned()), |cap| {
        PitchClass::from_str(&cap[1]).unwrap()
    })(s)
}

#[tracable_parser]
fn quality_parser(s: Span) -> IResult<Quality> {
    alt((
        map(tag("mM"), |_| Quality::MinorM7),
        map(tag("M"), |_| Quality::Major),
        map(tag("m"), |_| Quality::Minor),
        map(alt((tag("dim"), tag("o"))), |_| Quality::Dim),
        map(alt((tag("aug"), tag("+"))), |_| Quality::Aug),
    ))(s)
}

#[tracable_parser]
fn on_chord_parser(s: Span) -> IResult<PitchClass> {
    map(tuple((tag("/"), pitch_parser)), |(_, p)| p)(s)
}

#[tracable_parser]
fn degree_parser(s: Span) -> IResult<(u8, i8)> {
    map(capture(MOD_REGEX.to_owned()), |cap| {
        dbg!(&cap);
        let diff = match *cap[1] {
            "#" | "+" => 1,
            "b" | "-" => -1,
            "" => 0,
            _ => unreachable!(),
        };
        (cap[2].parse().unwrap(), diff)
    })(s)
}

#[tracable_parser]
fn modifiers_parser(s: Span) -> IResult<Modifier> {
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

#[tracable_parser]
pub fn chord_parser(s: Span) -> IResult<ChordNode> {
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

#[tracable_parser]
pub fn score_node_parser(s: Span) -> IResult<ScoreNode> {
    alt((
        map(tag("="), |_| ScoreNode::Sustain),
        map(tag("_"), |_| ScoreNode::Rest),
        map(tag("%"), |_| ScoreNode::Repeat),
        map(chord_parser, ScoreNode::Chord),
    ))(s)
}

#[tracable_parser]
pub fn measure_parser(s: Span) -> IResult<Vec<ScoreNode>> {
    alt((
        map(tag("N.C."), |_| vec![ScoreNode::Rest]),
        many1(score_node_parser),
    ))(s)
}

#[cfg(test)]
mod tests {
    use super::chord_parser;
    use anyhow::Result;
    use nom_locate::LocatedSpan;
    use nom_tracable::TracableInfo;

    #[test]
    fn test_chord_parser() -> Result<()> {
        let info = TracableInfo::new();

        // let chords = vec!["Ab6no5", "Dm7b5", "G7#5/B", "AbM7sus2/C", "AbM7G7"];
        let chords = vec!["AbM7G7"];
        for chord in chords.iter() {
            let (s, _chord) = chord_parser(LocatedSpan::new_extra(chord, info))?;
            assert_eq!(*s, "");
        }
        Ok(())
    }
}
