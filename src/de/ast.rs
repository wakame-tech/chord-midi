use super::chord::{chord_node_parser, degree_node_parser};
use super::Span;
use crate::model::chord::Chord;
use crate::model::degree::{from_semitone, into_semitone, Accidental, Degree};
use crate::model::{chord::Modifier, degree::Pitch};
use anyhow::Result;
use nom::branch::alt;
use nom::bytes::complete::{tag, take_until};
use nom::combinator::map;
use nom::multi::{many0, many1, separated_list1};
use nom::sequence::tuple;
use nom::IResult;
use nom_locate::LocatedSpan;
use nom_tracable::{tracable_parser, TracableInfo};
use std::fmt::Display;

#[derive(Debug, Clone, PartialEq)]
pub enum ModifierNode {
    Major(Degree),
    Minor(Degree),
    MinorMajaor7,
    Sus2,
    Sus4,
    Flat5th,
    Aug,
    Aug7,
    Dim,
    Dim7,
    Omit(Degree),
    Add(Degree),
    Tension(Accidental, Degree),
}

impl Display for ModifierNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModifierNode::Major(d) => write!(f, "{}", d.0),
            ModifierNode::Minor(d) => write!(f, "m{}", d.0),
            ModifierNode::MinorMajaor7 => write!(f, "mM7"),
            ModifierNode::Sus2 => write!(f, "sus2"),
            ModifierNode::Sus4 => write!(f, "sus4"),
            ModifierNode::Flat5th => write!(f, "-5"),
            ModifierNode::Aug => write!(f, "aug"),
            ModifierNode::Aug7 => write!(f, "aug7"),
            ModifierNode::Dim => write!(f, "dim"),
            ModifierNode::Dim7 => write!(f, "dim7"),
            ModifierNode::Omit(d) => write!(f, "omit{}", d.0),
            ModifierNode::Add(d) => write!(f, "add{}", d.0),
            ModifierNode::Tension(a, d) => write!(f, "{}{}", a, d.0),
        }
    }
}

impl ModifierNode {
    pub fn degree_to_mods(is_minor: bool, d: Degree) -> Result<Vec<Modifier>> {
        let third = Modifier::Mod(
            Degree(3),
            if is_minor {
                Accidental::Flat
            } else {
                Accidental::Natural
            },
        );
        let seventh = Modifier::Add(
            Degree(7),
            if is_minor {
                Accidental::Flat
            } else {
                Accidental::Natural
            },
        );
        match d {
            Degree(5) => Ok(vec![third]),
            Degree(6) => Ok(vec![third, Modifier::Add(Degree(6), Accidental::Natural)]),
            Degree(7) => Ok(vec![third, seventh]),
            Degree(9) => Ok(vec![
                third,
                seventh,
                Modifier::Add(Degree(9), Accidental::Natural),
            ]),
            _ => Err(anyhow::anyhow!("invalid degree: {:?}", d)),
        }
    }

    pub fn into_modifiers(self) -> Result<Vec<Modifier>> {
        match self {
            ModifierNode::Major(d) => Self::degree_to_mods(false, d),
            ModifierNode::Minor(d) => Self::degree_to_mods(true, d),
            ModifierNode::MinorMajaor7 => Ok(vec![
                Modifier::Mod(Degree(3), Accidental::Flat),
                Modifier::Add(Degree(7), Accidental::Natural),
            ]),
            ModifierNode::Sus2 => Ok(vec![Modifier::Mod(Degree(3), Accidental::Flat)]),
            ModifierNode::Sus4 => Ok(vec![Modifier::Mod(Degree(3), Accidental::Sharp)]),
            ModifierNode::Flat5th => Ok(vec![Modifier::Mod(Degree(5), Accidental::Flat)]),
            ModifierNode::Aug => Ok(vec![Modifier::Mod(Degree(5), Accidental::Sharp)]),
            ModifierNode::Aug7 => Ok(vec![
                Modifier::Mod(Degree(5), Accidental::Sharp),
                Modifier::Mod(Degree(7), Accidental::Sharp),
            ]),
            ModifierNode::Dim => Ok(vec![Modifier::Mod(Degree(5), Accidental::Flat)]),
            ModifierNode::Dim7 => Ok(vec![
                Modifier::Mod(Degree(5), Accidental::Flat),
                Modifier::Mod(Degree(7), Accidental::Flat),
            ]),
            ModifierNode::Omit(d) => Ok(vec![Modifier::Omit(d)]),
            ModifierNode::Add(d) => Ok(vec![Modifier::Add(d, Accidental::Natural)]),
            ModifierNode::Tension(a, d) => Ok(vec![Modifier::Add(d, a)]),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct ChordNode {
    pub root: Pitch,
    pub modifiers: Vec<ModifierNode>,
    pub tensions: Option<Vec<ModifierNode>>,
    pub on: Option<Pitch>,
}

fn fmt_mods(mods: &[ModifierNode]) -> String {
    mods.iter()
        .map(|m| format!("{}", m))
        .collect::<Vec<_>>()
        .join("")
}

impl Display for ChordNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (root, modifiers, tensions, on) = (
            &self.root,
            &self.modifiers,
            &self.tensions.as_ref(),
            &self.on,
        );
        let root = format!("{}", root);
        let mods = fmt_mods(modifiers);
        let tensions = tensions
            .map(|t| format!("({})", fmt_mods(&t)))
            .unwrap_or("".to_string());
        let on = on.map(|p| format!("/{}", p)).unwrap_or("".to_string());
        write!(f, "{}{}{}{}", root, mods, tensions, on)
    }
}

impl ChordNode {
    pub fn into_degree_node(self, key: Pitch) -> DegreeNode {
        let s = Pitch::diff(&key, &self.root);
        DegreeNode {
            root: from_semitone(s),
            modifiers: self.modifiers,
            tensions: self.tensions,
            on: self.on.map(|p| from_semitone(Pitch::diff(&key, &p))),
        }
    }

    pub fn into_chord(self, octave: u8) -> Result<Chord> {
        let mods = into_modifiers(self.modifiers)?;
        let tensions = into_modifiers(self.tensions.unwrap_or(vec![]))?;
        let on = self
            .on
            .as_ref()
            .map(|on| vec![Modifier::OnChord(Pitch::diff(&self.root, on))])
            .unwrap_or(vec![]);
        Ok(Chord::new(
            octave,
            self.root,
            Chord::degrees(&vec![mods, tensions, on].concat()),
        ))
    }
}

#[derive(Debug, PartialEq)]
pub struct DegreeNode {
    pub root: (Accidental, Degree),
    pub modifiers: Vec<ModifierNode>,
    pub tensions: Option<Vec<ModifierNode>>,
    pub on: Option<(Accidental, Degree)>,
}

impl Display for DegreeNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let root = format!("{}{}", self.root.0, self.root.1);
        let mods = fmt_mods(&self.modifiers);
        let tensions = self
            .tensions
            .as_ref()
            .map(|t| format!("({})", fmt_mods(&t)))
            .unwrap_or("".to_string());
        let on = self
            .on
            .as_ref()
            .map(|(a, d)| format!("/{}{}", a, d))
            .unwrap_or("".to_string());
        write!(f, "{}{}{}{}", root, mods, tensions, on)
    }
}

impl DegreeNode {
    pub fn into_chord(self, key: Pitch, octave: u8) -> Result<Chord> {
        let (root, modifiers, tensions, on) = (self.root, self.modifiers, self.tensions, self.on);
        let s = into_semitone(root.0, root.1);
        let key = Pitch::from_u8(key.into_u8() + s);
        let on = on
            .map(|(a, d)| vec![Modifier::OnChord(into_semitone(a, d))])
            .unwrap_or(vec![]);
        let mods = vec![
            into_modifiers(modifiers)?,
            if let Some(tensions) = tensions {
                into_modifiers(tensions)?
            } else {
                vec![]
            },
            on,
        ]
        .concat();
        let degrees = Chord::degrees(&mods);
        Ok(Chord::new(octave, key, degrees))
    }
}

fn into_modifiers(mods: Vec<ModifierNode>) -> Result<Vec<Modifier>> {
    Ok(vec![mods
        .into_iter()
        .map(|m| m.into_modifiers())
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()]
    .concat())
}

#[derive(Debug, PartialEq)]
pub enum Node {
    Chord(ChordNode),
    Degree(DegreeNode),
    Rest,
    Sustain,
    Repeat,
}

impl Display for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Node::Chord(c) => write!(f, "{}", c),
            Node::Degree(d) => write!(f, "{}", d),
            Node::Rest => write!(f, ""),
            Node::Sustain => write!(f, "_"),
            Node::Repeat => write!(f, "%"),
        }
    }
}

#[tracable_parser]
fn node_parser(s: Span) -> IResult<Span, Node> {
    alt((
        map(tag("="), |_| Node::Sustain),
        map(tag("_"), |_| Node::Rest),
        map(tag("%"), |_| Node::Repeat),
        map(tag("N.C."), |_| Node::Rest),
        map(chord_node_parser, Node::Chord),
        map(degree_node_parser, Node::Degree),
    ))(s)
}

#[tracable_parser]
fn measure_parser(s: Span) -> IResult<Span, Measure> {
    map(many1(node_parser), Measure)(s)
}

#[tracable_parser]
fn breaks_parser(s: Span) -> IResult<Span, Vec<Measure>> {
    map(alt((tag("\r\n"), tag("\n"), tag("|"))), |_| vec![])(s)
}

#[tracable_parser]
fn comment_parser(s: Span) -> IResult<Span, Vec<Measure>> {
    map(tuple((tag("#"), take_until("\n"))), |_| vec![])(s)
}

#[tracable_parser]
fn line_parser(s: Span) -> IResult<Span, Vec<Measure>> {
    alt((
        breaks_parser,
        comment_parser,
        separated_list1(breaks_parser, measure_parser),
    ))(s)
}

#[tracable_parser]
fn ast_parser(s: Span) -> IResult<Span, AST> {
    map(many0(line_parser), |lines| {
        AST(lines.into_iter().flatten().collect())
    })(s)
}

#[derive(Debug)]
pub struct Measure(pub Vec<Node>);

#[derive(Debug)]
pub struct AST(pub Vec<Measure>);

pub fn parse(code: &str) -> Result<AST> {
    let span = LocatedSpan::new_extra(code, TracableInfo::new());
    let (rest, ast) = ast_parser(span).map_err(|e| anyhow::anyhow!("parse error: {:?}", e))?;
    if !rest.is_empty() {
        return Err(anyhow::anyhow!("parse error: {:?}", rest));
    }
    Ok(ast)
}
