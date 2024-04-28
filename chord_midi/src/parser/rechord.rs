use super::parser_util::Span;
use crate::parser::chord::node_parser;
use crate::parser::RechordParser;
use crate::syntax::Ast;
use anyhow::Result;
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

impl super::Parser for RechordParser {
    fn parse(&self, code: &str) -> Result<Ast> {
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
        tuple((many1(alt((comment_parser, measure_parser))), eof)),
        |(score, _)| Ast::Score(score.into_iter().map(Box::new).collect()),
    )(s)
}

#[tracable_parser]
fn comment_parser(s: Span) -> IResult<Span, Ast> {
    map(
        tuple((tag("#"), not_line_ending, line_ending)),
        |(_, comment, _): (Span, Span, Span)| Ast::Comment(comment.to_string()),
    )(s)
}

fn measure_sep(s: Span) -> IResult<Span, bool> {
    alt((value(false, tag("|")), value(true, line_ending)))(s)
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
}
