use super::{
    chord::{parser_roman_num, DEGREE_REGEX, PITCH_REGEX},
    SexpImporter,
};
use crate::model::{
    ast::{Ast, ChordNode, Node},
    key::Key,
    pitch::{Accidental, Pitch},
    scale::Scale,
};
use anyhow::Result;
use std::{collections::BTreeSet, str::FromStr};
use symbolic_expressions::{parser::parse_str, Sexp};

impl super::Importer for SexpImporter {
    fn import(&self, code: &str) -> Result<Ast> {
        let score = parse_str(code)?;
        parse_ast(&score)
    }
}

fn degree_from_str(s: &str) -> Result<u8> {
    match s {
        s if s.ends_with("b") | s.ends_with("#") => {
            let d = parser_roman_num(&s[..s.len() - 1])?;
            let a = Accidental::from_str(&s[s.len() - 1..])?;
            Ok((Scale::Major.semitone(d) as i8 + a as i8) as u8)
        }
        s => {
            let d = parser_roman_num(s)?;
            Ok(Scale::Major.semitone(d) as u8)
        }
    }
}

fn parse_key(key: &str) -> Result<Key> {
    match key {
        pitch if PITCH_REGEX.is_match(&pitch) => Ok(Key::Absolute(Pitch::from_str(pitch)?)),
        degree if DEGREE_REGEX.is_match(&degree) => Ok(Key::Relative(degree_from_str(degree)?)),
        _ => Err(anyhow::anyhow!("invalid key: {}", key)),
    }
}

fn parse_node(sexp: &Sexp) -> Result<Node> {
    match sexp {
        Sexp::String(s) if s == "N.C." => Ok(Node::Rest),
        Sexp::String(s) if s == "=" => Ok(Node::Sustain),
        Sexp::String(s) if s == "_" => Ok(Node::Rest),
        Sexp::String(s) if s == "%" => Ok(Node::Repeat),
        Sexp::String(key) => Ok(Node::Chord(ChordNode {
            key: parse_key(&key)?,
            modifiers: BTreeSet::new(),
            on: None,
        })),
        Sexp::List(list) if starts_with(&sexp, "chord") => Ok(Node::Chord(ChordNode {
            key: parse_key(list[1].string()?)?,
            modifiers: BTreeSet::new(),
            on: None,
        })),
        _ => Err(anyhow::anyhow!("unexpected input: {:?}", sexp)),
    }
}

fn starts_with(sexp: &Sexp, tag: &str) -> bool {
    sexp.is_list() && sexp.list_name().ok() == Some(&tag.to_string())
}

fn parse_ast(sexp: &Sexp) -> Result<Ast> {
    match sexp {
        // score(...: Measure) -> Score
        Sexp::List(list) if starts_with(&sexp, "score") => {
            let measures = list[1..]
                .iter()
                .map(|ast| parse_ast(ast).map(Box::new))
                .collect::<Result<Vec<_>>>()?;
            Ok(Ast::Score(measures))
        }
        // keyed(pitch: Pitch, measure: Measure) -> Measure
        Sexp::List(list) if starts_with(sexp, "keyed") => {
            let key = parse_key(list[1].string()?)?;
            let Key::Absolute(pitch) = key else {
                return Err(anyhow::anyhow!("pitch must be absolute: {:?}", key));
            };
            let measure = parse_ast(&list[2])?.into_pitch(pitch);
            Ok(measure)
        }
        // (...: Node): Measure
        Sexp::List(list) => Ok(Ast::Measure(
            list.into_iter()
                .map(parse_node)
                .collect::<Result<Vec<_>>>()?,
            false,
        )),
        _ => Err(anyhow::anyhow!("unexpected input: {:?}", sexp)),
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        import::{Importer, SexpImporter},
        model::ast::{Ast, ChordNode, Node},
        model::pitch::Pitch,
    };
    use anyhow::Result;

    #[test]
    fn test_parse_score() -> Result<()> {
        let c = Node::Chord(ChordNode::absolute(Pitch::C));
        let d = Node::Chord(ChordNode::absolute(Pitch::D));
        let e = Node::Chord(ChordNode::absolute(Pitch::E));
        let f = Node::Chord(ChordNode::absolute(Pitch::F));
        assert_eq!(
            SexpImporter.import("(score (C D) (E F))")?,
            Ast::Score(vec![
                Box::new(Ast::Measure(vec![c, d], false)),
                Box::new(Ast::Measure(vec![e, f], false))
            ])
        );

        let is = Node::Chord(ChordNode::relative(1));
        let iv = Node::Chord(ChordNode::relative(5));
        assert_eq!(
            SexpImporter.import("(score (I# IV))")?,
            Ast::Score(vec![Box::new(Ast::Measure(vec![is, iv], false))])
        );
        Ok(())
    }

    #[test]
    fn test_modifier() -> Result<()> {
        let score = SexpImporter.import("(score (keyed C (I IV)) (keyed D (I IV)))")?;
        assert_eq!(score, SexpImporter.import("(score (C F) (D G))")?);
        Ok(())
    }
}
