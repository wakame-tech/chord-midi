use super::{chord::PITCH_REGEX, SexpParser};
use crate::{
    parser::chord::DEGREE_NAME_REGEX,
    syntax::{Ast, ChordNode, Key, Node, Pitch},
};
use anyhow::Result;
use std::{collections::BTreeSet, str::FromStr};
use symbolic_expressions::{parser::parse_str, Sexp};

impl super::Parser for SexpParser {
    fn parse(&self, code: &str) -> Result<Ast> {
        let score = parse_str(code)?;
        parse_ast(&score)
    }
}

fn parse_key(key: &str) -> Result<Key> {
    match key {
        pitch if PITCH_REGEX.is_match(&pitch) => Ok(Key::Absolute(Pitch::from_str(pitch)?)),
        degree if DEGREE_NAME_REGEX.is_match(&degree) => Ok(Key::Relative(degree.parse()?)),
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
    use crate::parser::{Parser, SexpParser};
    use crate::syntax::{Ast, ChordNode, Key, Node, Pitch};
    use anyhow::Result;
    use std::collections::BTreeSet;
    use std::str::FromStr;

    #[test]
    fn test_parse_score() -> Result<()> {
        let score = "(score (C D) (E F))";

        fn pitch(key: &str) -> Node {
            Node::Chord(ChordNode {
                key: Key::Absolute(Pitch::from_str(key).unwrap()),
                modifiers: BTreeSet::new(),
                on: None,
            })
        }

        let ast = SexpParser.parse(score)?;
        println!("{}", ast);
        assert_eq!(
            ast,
            Ast::Score(vec![
                Box::new(Ast::Measure(vec![pitch("C"), pitch("D")], false)),
                Box::new(Ast::Measure(vec![pitch("E"), pitch("F")], false))
            ])
        );
        Ok(())
    }
}
