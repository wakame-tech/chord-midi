use super::parser_util::Span;
use crate::import::chord::node_parser;
use crate::import::RechordImporter;
use crate::model::ast::Ast;
use crate::model::ast::Directive;
use crate::model::pitch::Pitch;
use crate::model::scale::Scale;
use anyhow::Result;
use std::str::FromStr;
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::{line_ending, not_line_ending, space0};
use nom::combinator::{eof, map, value};
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
        tuple((many1(alt((directive_parser, comment_parser, measure_parser))), eof)),
        |(score, _)| Ast::Score(score.into_iter().map(Box::new).collect()),
    )(s)
}

#[tracable_parser]
fn directive_parser(s: Span) -> IResult<Span, Ast> {
    alt((
        map(
            tuple((tag("key:"), space0, not_line_ending, line_ending)),
            |(_, _, val, _): (Span, Span, Span, Span)| {
                Ast::Directive(Directive::Key(Pitch::from_str(val.fragment().trim()).unwrap()))
            },
        ),
        map(
            tuple((tag("scale:"), space0, not_line_ending, line_ending)),
            |(_, _, val, _): (Span, Span, Span, Span)| {
                Ast::Directive(Directive::Scale(Scale::from_str(val.fragment().trim()).unwrap()))
            },
        ),
    ))(s)
}

#[tracable_parser]
fn comment_parser(s: Span) -> IResult<Span, Ast> {
    map(
        tuple((tag("#"), not_line_ending, line_ending)),
        |(_, comment, _): (Span, Span, Span)| Ast::Comment(comment.to_string()),
    )(s)
}

fn measure_sep(s: Span) -> IResult<Span, bool> {
    // eof acts as an implicit separator so the final measure need not end
    // with a trailing `|` or newline.
    alt((
        value(false, tag("|")),
        value(true, line_ending),
        value(false, eof),
    ))(s)
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
}
