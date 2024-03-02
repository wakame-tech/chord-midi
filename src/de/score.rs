use super::chord::chord_parser;
use super::{IResult, Span};
use crate::model::{chord::Modifier, degree::Pitch};
use anyhow::Result;
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::combinator::map;
use nom::multi::many1;
use nom_locate::LocatedSpan;
use nom_tracable::{tracable_parser, TracableInfo};

#[derive(Debug, PartialEq)]
pub struct ChordNode {
    pub root: Pitch,
    pub modifiers: Vec<Modifier>,
}

#[derive(Debug, PartialEq)]
pub struct DegreeNode {
    pub modifiers: Vec<Modifier>,
}

#[derive(Debug, PartialEq)]
pub enum Node {
    Chord(ChordNode),
    Degree(DegreeNode),
    Rest,
    Sustain,
    Repeat,
}

#[tracable_parser]
pub fn node_parser(s: Span) -> IResult<Node> {
    alt((
        map(tag("="), |_| Node::Sustain),
        map(tag("_"), |_| Node::Rest),
        map(tag("%"), |_| Node::Repeat),
        map(chord_parser, Node::Chord),
    ))(s)
}

#[tracable_parser]
pub fn nodes_parser(s: Span) -> IResult<Vec<Node>> {
    alt((map(tag("N.C."), |_| vec![Node::Rest]), many1(node_parser)))(s)
}

#[derive(Debug)]
pub struct Measure(pub Vec<Node>);

impl Measure {
    pub fn parse(s: &str) -> Result<Self> {
        let info = TracableInfo::new().forward(true);
        let span = LocatedSpan::new_extra(s, info);
        let (rest, nodes) = nodes_parser(span).map_err(|e| anyhow::anyhow!("{}", e))?;
        if !rest.is_empty() {
            return Err(anyhow::anyhow!("cannot parse {} rest={}", s, rest));
        }
        #[cfg(feature = "trace")]
        {
            nom_tracable::histogram();
            nom_tracable::cumulative_histogram();
        }
        Ok(Self(nodes))
    }
}

pub struct AST(pub Vec<Measure>);

impl AST {
    pub fn parse(s: &str) -> Result<Self> {
        let measures = s
            .split("\r\n")
            .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
            .flat_map(|line| line.split('|').collect::<Vec<_>>())
            .filter(|s| !s.trim().is_empty())
            .map(|m| Measure::parse(m))
            .collect::<Result<Vec<_>>>()?;
        Ok(Self(measures))
    }
}
