use super::{
    chord::{parser_roman_num, DEGREE_REGEX, PITCH_REGEX},
    SexpImporter,
};
use crate::model::{
    ast::{ChordNode, Expr, Node},
    key::Key,
    pitch::{Accidental, Pitch},
    scale::Degree,
};
use anyhow::Result;
use std::{collections::BTreeSet, str::FromStr};
use symbolic_expressions::{parser::parse_str, Sexp};

impl super::Importer for SexpImporter {
    fn import(&self, code: &str) -> Result<Expr> {
        let score = parse_str(code)?;
        parse_ast(&score)
    }
}

fn degree_from_str(s: &str) -> Result<Degree> {
    let (roman, acc) = if let Some(stripped) = s.strip_suffix('#') {
        (stripped, Accidental::Sharp)
    } else if let Some(stripped) = s.strip_suffix('b') {
        (stripped, Accidental::Flat)
    } else {
        (s, Accidental::Natural)
    };
    let d = parser_roman_num(roman)?;
    Ok(Degree(d, acc))
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
            root_string: None,
        })),
        Sexp::List(list) if starts_with(&sexp, "chord") => Ok(Node::Chord(ChordNode {
            key: parse_key(list[1].string()?)?,
            modifiers: BTreeSet::new(),
            on: None,
            root_string: None,
        })),
        _ => Err(anyhow::anyhow!("unexpected input: {:?}", sexp)),
    }
}

fn starts_with(sexp: &Sexp, tag: &str) -> bool {
    sexp.is_list() && sexp.list_name().ok() == Some(&tag.to_string())
}

fn parse_ast(sexp: &Sexp) -> Result<Expr> {
    match sexp {
        // score(...: Measure) -> Seq
        Sexp::List(list) if starts_with(&sexp, "score") => {
            let measures = list[1..]
                .iter()
                .map(parse_ast)
                .collect::<Result<Vec<_>>>()?;
            Ok(Expr::Seq(measures))
        }
        // keyed(pitch: Pitch, measure: Measure) -> Expr::Keyed
        Sexp::List(list) if starts_with(sexp, "keyed") => {
            let key = parse_key(list[1].string()?)?;
            let Key::Absolute(pitch) = key else {
                return Err(anyhow::anyhow!("pitch must be absolute: {:?}", key));
            };
            // 解決は resolve() の仕事。ここでは包むだけ
            let inner = parse_ast(&list[2])?;
            Ok(Expr::Keyed(pitch, Box::new(inner)))
        }
        // (...: Node): Measure
        Sexp::List(list) => Ok(Expr::Measure(
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
        model::ast::{ChordNode, Expr, Node},
        model::pitch::{Accidental, Pitch},
        model::scale::Degree,
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
            Expr::Seq(vec![
                Expr::Measure(vec![c, d], false),
                Expr::Measure(vec![e, f], false)
            ])
        );

        // I# parses to degree I with a sharp; IV parses to degree IV natural
        let is = Node::Chord(ChordNode::relative(Degree(1, Accidental::Sharp)));
        let iv = Node::Chord(ChordNode::relative(Degree(4, Accidental::Natural)));
        assert_eq!(
            SexpImporter.import("(score (I# IV))")?,
            Expr::Seq(vec![Expr::Measure(vec![is, iv], false)])
        );
        Ok(())
    }

    #[test]
    fn test_modifier() -> Result<()> {
        // keyed はもう parse 時に解決しない (Expr::Keyed のまま残る) ので、
        // 生の Expr は (C F)(D G) と構造が異なる。
        // 最終的な音が同じであることは resolve() 後に確認する
        let score = SexpImporter.import("(score (keyed C (I IV)) (keyed D (I IV)))")?;
        assert_eq!(
            score.resolve(),
            SexpImporter.import("(score (C F) (D G))")?.resolve()
        );
        Ok(())
    }

    #[test]
    fn test_keyed_becomes_keyed_expr() -> Result<()> {
        use crate::model::ast::Expr;
        use crate::model::pitch::Pitch;
        let ast = SexpImporter.import("(score (keyed C (I IV)))")?;
        let Expr::Seq(items) = &ast else {
            panic!("not a seq")
        };
        assert!(
            matches!(&items[0], Expr::Keyed(Pitch::C, _)),
            "keyed は Expr::Keyed になる: {:?}",
            items[0]
        );
        Ok(())
    }

    /// keyed が入れ子になった場合、内側の keyed が以降のスコープで
    /// 外側の tonic を上書きする (rechord の `key:` と同じ挙動)
    #[test]
    fn test_nested_keyed_inner_wins() -> Result<()> {
        use crate::model::ast::{ChordNode, Node};
        use crate::model::pitch::Pitch;
        let ast = SexpImporter.import("(score (keyed C (keyed D (I IV))))")?;
        let resolved = ast.resolve();
        let expected = Expr::Seq(vec![Expr::Measure(
            vec![
                Node::Chord(ChordNode::absolute(Pitch::D)),
                Node::Chord(ChordNode::absolute(Pitch::G)),
            ],
            false,
        )]);
        assert_eq!(resolved, expected);
        Ok(())
    }

    /// (score (keyed C (I IV))) を resolve() した結果、度数が実音になること。
    /// keyed のパース時解決をやめても最終的な音は変わらないことを固定する
    #[test]
    fn test_keyed_resolves_to_absolute_pitches() -> Result<()> {
        use crate::model::ast::{ChordNode, Node};
        use crate::model::pitch::Pitch;
        let ast = SexpImporter.import("(score (keyed C (I IV)))")?;
        let resolved = ast.resolve();
        let expected = Expr::Seq(vec![Expr::Measure(
            vec![
                Node::Chord(ChordNode::absolute(Pitch::C)),
                Node::Chord(ChordNode::absolute(Pitch::F)),
            ],
            false,
        )]);
        assert_eq!(resolved, expected);
        Ok(())
    }
}
