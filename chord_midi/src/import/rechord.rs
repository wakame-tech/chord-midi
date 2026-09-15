use super::parser_util::Span;
use crate::import::chord::node_parser;
use crate::import::RechordImporter;
use crate::model::ast::{Ast, Beat, Directive, Func, Pattern};
use crate::model::pitch::Pitch;
use crate::model::scale::Scale;
use anyhow::Result;
use std::str::FromStr;
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::{line_ending, not_line_ending, space0};
use nom::combinator::{eof, map, map_res, opt, peek, value};
use nom::multi::{many0, many1};
use nom::sequence::{delimited, tuple};
use nom::IResult;
use nom_locate::LocatedSpan;
use nom_tracable::tracable_parser;
use nom_tracable::TracableInfo;

impl super::Importer for RechordImporter {
    fn import(&self, code: &str) -> Result<Ast> {
        let code = code.replace("♭", "b");
        let span = LocatedSpan::new_extra(code.as_str(), TracableInfo::new());
        let (rest, ast) = ast_parser(span).map_err(|e| anyhow::anyhow!("parse error: {:?}", e))?;
        if !rest.is_empty() {
            return Err(anyhow::anyhow!("parse error: {:?}", rest));
        }
        Ok(ast)
    }
}

#[tracable_parser]
fn ast_parser(s: Span) -> IResult<Span, Ast> {
    map(
        tuple((
            many1(alt((
                directive_parser,
                comment_parser,
                apply_parser,
                measure_parser,
            ))),
            eof,
        )),
        |(score, _)| Ast::Score(score.into_iter().map(Box::new).collect()),
    )(s)
}

#[tracable_parser]
fn directive_parser(s: Span) -> IResult<Span, Ast> {
    // a bad value (e.g. `key: H`) fails the parse gracefully (map_res) rather
    // than panicking — important for the realtime web editor while typing.
    // The terminator accepts eof so a trailing directive without a newline parses.
    alt((
        map_res(
            tuple((tag("key:"), space0, not_line_ending, alt((line_ending, eof)))),
            |(_, _, val, _): (Span, Span, Span, Span)| {
                Pitch::from_str(val.fragment().trim()).map(|p| Ast::Directive(Directive::Key(p)))
            },
        ),
        map_res(
            tuple((tag("scale:"), space0, not_line_ending, alt((line_ending, eof)))),
            |(_, _, val, _): (Span, Span, Span, Span)| {
                Scale::from_str(val.fragment().trim()).map(|s| Ast::Directive(Directive::Scale(s)))
            },
        ),
    ))(s)
}

#[tracable_parser]
fn comment_parser(s: Span) -> IResult<Span, Ast> {
    map(
        tuple((tag("#"), not_line_ending, line_ending)),
        // `# ` の後ろの空白は落とす。Display 側が `# ` を付けて出すので、
        // 残したままだと書き戻すたびに空白が1つずつ増えていく
        |(_, comment, _): (Span, Span, Span)| Ast::Comment(comment.trim().to_string()),
    )(s)
}

fn measure_sep(s: Span) -> IResult<Span, bool> {
    // eof acts as an implicit separator so the final measure need not end
    // with a trailing `|` or newline.
    alt((
        // `|` の直後が行末なら、その小節でひと行が終わる
        value(true, tuple((tag("|"), space0, peek(line_ending)))),
        value(false, tag("|")),
        value(true, line_ending),
        // 関数の閉じ括弧の直前も区切りとして扱う。`)` は消費しない
        value(false, peek(tag(")"))),
        value(false, eof),
    ))(s)
}

/// `"o.ox"` のようなリズムパターン。長さは 1/2/4/8/16 のみ
#[tracable_parser]
fn pattern_parser(s: Span) -> IResult<Span, Pattern> {
    map_res(
        delimited(
            tag("\""),
            many1(alt((
                value(Beat::Hit, tag("o")),
                value(Beat::Rest, tag(".")),
                value(Beat::Brush, tag("x")),
            ))),
            tag("\""),
        ),
        |beats: Vec<Beat>| {
            // MEASURE_LENGTH = 16 を割り切れる長さだけ許す
            if matches!(beats.len(), 1 | 2 | 4 | 8 | 16) {
                Ok(Pattern(beats))
            } else {
                Err(anyhow::anyhow!(
                    "pattern length must be 1/2/4/8/16, got {}",
                    beats.len()
                ))
            }
        },
    )(s)
}

/// `pow( <小節列> )` と `pat( "<パターン>", <小節列> )`
#[tracable_parser]
fn apply_parser(s: Span) -> IResult<Span, Ast> {
    map(
        tuple((
            alt((
                map(tuple((tag("pow("), space0)), |_| Func::Pow),
                map(tuple((tag("oct("), space0)), |_| Func::Oct),
                map(
                    tuple((
                        tag("pat("),
                        space0,
                        pattern_parser,
                        space0,
                        tag(","),
                        space0,
                    )),
                    |(_, _, pattern, _, _, _)| Func::Pat(pattern),
                ),
            )),
            many1(alt((apply_parser, measure_parser))),
            space0,
            tag(")"),
            space0,
            // 閉じ括弧の直後が行末なら、この関数でひと行が終わる
            opt(peek(line_ending)),
            space_or_line_ending_many0,
        )),
        |(func, items, _, _, _, br, _)| {
            Ast::Apply(func, items.into_iter().map(Box::new).collect(), br.is_some())
        },
    )(s)
}

fn space_or_line_ending_many0(s: Span) -> IResult<Span, ()> {
    value((), many0(alt((tag(" "), line_ending))))(s)
}

#[tracable_parser]
fn measure_parser(s: Span) -> IResult<Span, Ast> {
    map(
        tuple((
            many1(delimited(space0, node_parser, space0)),
            measure_sep,
            space_or_line_ending_many0,
        )),
        |(nodes, br, _)| Ast::Measure(nodes, br),
    )(s)
}

#[cfg(test)]
mod tests {
    use super::{ast_parser, measure_parser};
    use anyhow::Result;
    use nom_locate::LocatedSpan;
    use nom_tracable::TracableInfo;

    fn span(s: &str) -> LocatedSpan<&str, TracableInfo> {
        LocatedSpan::new_extra(s, TracableInfo::new())
    }

    #[test]
    fn test_measure_parser() -> Result<()> {
        for measure in ["C\n"] {
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

    #[test]
    fn test_directive_parser() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::{Ast, Directive};
        use crate::model::pitch::Pitch;
        use crate::model::scale::Scale;
        let ast = RechordImporter.import("key: C\nscale: dorian\nC |\n").unwrap();
        let Ast::Score(items) = &ast else { panic!() };
        assert!(matches!(items[0].as_ref(), Ast::Directive(Directive::Key(Pitch::C))));
        assert!(matches!(items[1].as_ref(), Ast::Directive(Directive::Scale(Scale::Dorian))));
    }

    #[test]
    fn test_bad_directive_errors_not_panics() {
        use crate::import::{Importer, RechordImporter};
        // invalid pitch / scale must yield an Err (graceful), never panic
        assert!(RechordImporter.import("key: H\nC |\n").is_err());
        assert!(RechordImporter.import("scale: bogus\nC |\n").is_err());
    }

    #[test]
    fn test_scale_directive_changes_pitch() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::{Ast, Node};
        use crate::model::key::Key;
        use crate::model::pitch::Pitch;
        // III resolves differently per scale: Major -> E, Dorian -> D#/Ds
        let major = RechordImporter.import("key: C\nIII |\n").unwrap().resolve();
        let dorian = RechordImporter
            .import("key: C\nscale: dorian\nIII |\n")
            .unwrap()
            .resolve();
        let first_key = |ast: &Ast| -> Key {
            let Ast::Score(items) = ast else { panic!() };
            for it in items {
                if let Ast::Measure(nodes, _) = it.as_ref() {
                    if let Some(Node::Chord(c)) = nodes.first() {
                        return c.key.clone();
                    }
                }
            }
            panic!("no chord")
        };
        assert_eq!(first_key(&major), Key::Absolute(Pitch::E));
        assert_eq!(first_key(&dorian), Key::Absolute(Pitch::Ds));
    }

    #[test]
    fn test_pow_parser() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::{Ast, Func};
        let ast = RechordImporter.import("pow(C | E D)\n").unwrap();
        let Ast::Score(items) = &ast else { panic!("not a score") };
        assert_eq!(items.len(), 1);
        let Ast::Apply(Func::Pow, inner, _) = items[0].as_ref() else {
            panic!("not a pow apply: {:?}", items[0])
        };
        // C の小節と E D の小節
        assert_eq!(inner.len(), 2);
        let Ast::Measure(nodes, _) = inner[0].as_ref() else { panic!("not a measure") };
        assert_eq!(nodes.len(), 1);
        let Ast::Measure(nodes, _) = inner[1].as_ref() else { panic!("not a measure") };
        assert_eq!(nodes.len(), 2);
    }

    #[test]
    fn test_pat_parser() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::{Ast, Beat, Func};
        let ast = RechordImporter.import("pat(\"o.ox\", C | E D)\n").unwrap();
        let Ast::Score(items) = &ast else { panic!("not a score") };
        let Ast::Apply(Func::Pat(pattern), inner, _) = items[0].as_ref() else {
            panic!("not a pat apply: {:?}", items[0])
        };
        assert_eq!(
            pattern.0,
            vec![Beat::Hit, Beat::Rest, Beat::Hit, Beat::Brush]
        );
        assert_eq!(inner.len(), 2);
    }

    #[test]
    fn test_pat_nested_in_pow() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::{Ast, Func};
        let ast = RechordImporter.import("pat(\"oxox\", pow(C))\n").unwrap();
        let Ast::Score(items) = &ast else { panic!("not a score") };
        let Ast::Apply(Func::Pat(_), inner, _) = items[0].as_ref() else {
            panic!("not a pat apply")
        };
        assert!(matches!(inner[0].as_ref(), Ast::Apply(Func::Pow, _, _)));
    }

    #[test]
    fn test_apply_followed_by_measure() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::Ast;
        let ast = RechordImporter.import("pow(C) F |\n").unwrap();
        let Ast::Score(items) = &ast else { panic!("not a score") };
        assert_eq!(items.len(), 2);
        assert!(matches!(items[0].as_ref(), Ast::Apply(_, _, _)));
        assert!(matches!(items[1].as_ref(), Ast::Measure(_, _)));
    }

    #[test]
    fn test_pattern_length_must_be_power_of_two() {
        use crate::import::{Importer, RechordImporter};
        // 3 文字は 16 を割り切れないので受け付けない
        assert!(RechordImporter.import("pat(\"o.o\", C)\n").is_err());
        assert!(RechordImporter.import("pat(\"o.o.o\", C)\n").is_err());
    }

    #[test]
    fn test_pattern_rejects_unknown_char() {
        use crate::import::{Importer, RechordImporter};
        assert!(RechordImporter.import("pat(\"o.oz\", C)\n").is_err());
    }
}
