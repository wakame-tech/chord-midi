use super::parser_util::Span;
use crate::import::chord::node_parser;
use crate::import::RechordImporter;
use crate::model::ast::{Beat, Expr, Func, Pattern};
use crate::model::pitch::Pitch;
use crate::model::scale::Scale;
use anyhow::Result;
use nom::branch::alt;
use nom::bytes::complete::{tag, take_while1};
use nom::character::complete::{digit1, line_ending, not_line_ending, space0};
use nom::combinator::{eof, map, map_res, opt, peek, value};
use nom::multi::{many0, many1};
use nom::sequence::{delimited, tuple};
use nom::IResult;
use nom_locate::LocatedSpan;
use nom_tracable::tracable_parser;
use nom_tracable::TracableInfo;
use std::str::FromStr;

impl super::Importer for RechordImporter {
    fn import(&self, code: &str) -> Result<Expr> {
        let code = code.replace("♭", "b");
        let span = LocatedSpan::new_extra(code.as_str(), TracableInfo::new());
        let (rest, ast) = ast_parser(span).map_err(|e| anyhow::anyhow!("parse error: {:?}", e))?;
        if !rest.is_empty() {
            return Err(anyhow::anyhow!("parse error: {:?}", rest));
        }
        Ok(ast)
    }
}

/// `key:` / `scale:` の1行。値だけを返す。以降を包むのは seq_parser の仕事
enum DirectiveHead {
    Key(Pitch),
    Scale(Scale),
}

#[tracable_parser]
fn directive_head(s: Span) -> IResult<Span, DirectiveHead> {
    // a bad value (e.g. `key: H`) fails the parse gracefully (map_res) rather
    // than panicking — important for the realtime web editor while typing.
    // The terminator accepts eof so a trailing directive without a newline parses.
    alt((
        map_res(
            tuple((
                tag("key:"),
                space0,
                not_line_ending,
                alt((line_ending, eof)),
            )),
            |(_, _, val, _): (Span, Span, Span, Span)| {
                Pitch::from_str(val.fragment().trim()).map(DirectiveHead::Key)
            },
        ),
        map_res(
            tuple((
                tag("scale:"),
                space0,
                not_line_ending,
                alt((line_ending, eof)),
            )),
            |(_, _, val, _): (Span, Span, Span, Span)| {
                Scale::from_str(val.fragment().trim()).map(DirectiveHead::Scale)
            },
        ),
    ))(s)
}

/// 式の列。`key:` / `scale:` に当たったら、そこから後ろを再帰で読んで包む
fn seq_parser(s: Span) -> IResult<Span, Expr> {
    let mut items: Vec<Expr> = Vec::new();
    let mut rest = s;
    loop {
        if let Ok((r, head)) = directive_head(rest) {
            let (r2, inner) = seq_parser(r)?;
            items.push(match head {
                DirectiveHead::Key(p) => Expr::Keyed(p, Box::new(inner)),
                DirectiveHead::Scale(sc) => Expr::Scaled(sc, Box::new(inner)),
            });
            rest = r2;
            // 以降は inner が飲み切っている
            break;
        }
        // `$name = ...` の定義。`key:` と違ってスコープの包みではなく、
        // 0小節を返す単位元なので、後ろを包まずそのまま平らに並べる。
        // alt に混ぜずにここで先に試すことで、`$a = ...` の `$a` だけを
        // ref_parser に食われないようにしている
        if let Ok((r, def)) = define_parser(rest) {
            items.push(def);
            rest = r;
            continue;
        }
        // `apply_parser` は `group_parser` より先。逆だと `pow(C)` の `(C)` が
        // グループとして食われる
        match alt((
            comment_parser,
            apply_parser,
            group_parser,
            ref_parser,
            measure_parser,
        ))(rest)
        {
            Ok((r, item)) => {
                // 入力が減らないまま成功するパーサがあると無限ループになる。
                // 念のため「進んでいない」ときは打ち切る
                if r.fragment().len() >= rest.fragment().len() {
                    break;
                }
                items.push(item);
                rest = r;
            }
            Err(_) => break,
        }
    }
    Ok((rest, Expr::Seq(items)))
}

#[tracable_parser]
fn ast_parser(s: Span) -> IResult<Span, Expr> {
    // 旧実装は many1 だったので空入力を拒んでいた。その挙動を保つ
    map_res(tuple((seq_parser, eof)), |(seq, _)| {
        if matches!(&seq, Expr::Seq(items) if items.is_empty()) {
            Err(anyhow::anyhow!("empty input"))
        } else {
            Ok(seq)
        }
    })(s)
}

#[tracable_parser]
fn comment_parser(s: Span) -> IResult<Span, Expr> {
    map(
        tuple((tag("#"), not_line_ending, line_ending)),
        // `# ` の後ろの空白は落とす。Display 側が `# ` を付けて出すので、
        // 残したままだと書き戻すたびに空白が1つずつ増えていく
        |(_, comment, _): (Span, Span, Span)| Expr::Comment(comment.trim().to_string()),
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

/// `"o.ox"` のリズムパターンを式として読む
fn pattern_atom(s: Span) -> IResult<Span, Expr> {
    map(tuple((pattern_parser, space0)), |(p, _)| Expr::Pattern(p))(s)
}

/// `$` に続く名前。英数字 + `_` + 日本語。記号は取らない。
/// `char::is_alphanumeric` は漢字・かな・カタカナに true を返すので
/// `$Aメロ` がそのまま通る
fn name_parser(s: Span) -> IResult<Span, String> {
    map(
        tuple((
            tag("$"),
            take_while1(|c: char| c.is_alphanumeric() || c == '_'),
        )),
        |(_, name): (Span, Span)| name.fragment().to_string(),
    )(s)
}

/// `$name` の参照。後ろの区切りは飲まない。関数の引数のように式の内側で使う
fn ref_atom(s: Span) -> IResult<Span, Expr> {
    map(name_parser, Expr::Ref)(s)
}

/// `$name` の参照。小節の位置に置けるよう、後ろの区切り (`|` / 改行) まで飲む
#[tracable_parser]
fn ref_parser(s: Span) -> IResult<Span, Expr> {
    map(
        tuple((
            ref_atom,
            space0,
            opt(repeat_suffix),
            opt(measure_sep),
            space_or_line_ending_many0,
        )),
        |(r, _, n, _, _)| repeated(r, n),
    )(s)
}

/// `$name = <本体>`。本体は1行。
///
/// 本体を読む前に `not_line_ending` でその行の範囲を切り出し、切り出した
/// Span の上で本体のパーサを走らせる。`measure_parser` は `measure_sep` で
/// 改行を消費してしまうので、行の中で止まるよう入力の側を先に区切っておく
#[tracable_parser]
fn define_parser(s: Span) -> IResult<Span, Expr> {
    let (s, (name, _, _, _)) = tuple((name_parser, space0, tag("="), space0))(s)?;
    let (s, line) = not_line_ending(s)?;
    let (leftover, body) = define_body_parser(line)?;
    // 行の中に読めない残りがあるなら、その定義ごと失敗させる。
    // 黙って捨てると `$a = C ???` のような行が通ってしまう
    if !leftover.fragment().trim().is_empty() {
        return Err(nom::Err::Error(nom::error::Error::new(
            leftover,
            nom::error::ErrorKind::Eof,
        )));
    }
    let (s, _) = alt((line_ending, eof))(s)?;
    let (s, _) = space_or_line_ending_many0(s)?;
    Ok((s, Expr::Define(name, Box::new(body))))
}

/// 定義の本体。入力は既に1行ぶんに切り出された Span なので、
/// 行末はそのまま `eof` として見える
fn define_body_parser(s: Span) -> IResult<Span, Expr> {
    let mut items: Vec<Expr> = Vec::new();
    let mut rest = s;
    while !rest.fragment().trim_start().is_empty() {
        match alt((
            apply_parser,
            group_parser,
            ref_parser,
            pattern_atom,
            measure_parser,
        ))(rest)
        {
            Ok((r, item)) => {
                // 入力が減らないまま成功するパーサがあると無限ループになる
                if r.fragment().len() >= rest.fragment().len() {
                    break;
                }
                items.push(item);
                rest = r;
            }
            Err(_) => break,
        }
    }
    if items.is_empty() {
        return Err(nom::Err::Error(nom::error::Error::new(
            s,
            nom::error::ErrorKind::Many1,
        )));
    }
    // 要素が1つなら包まない。`$eight = "o.o.o."` の本体が `Expr::Pattern`
    // そのものになり、`pat` の引数としてそのまま使える
    let body = if items.len() == 1 {
        items.pop().expect("len == 1")
    } else {
        Expr::Seq(items)
    };
    Ok((rest, body))
}

/// `xN` の接尾辞。2〜32 だけ受け付ける。
/// `x0` / `x1` は「繰り返し」として意味がなく、大きすぎる N は
/// 打ち間違いの可能性が高いので、どちらもここで弾く
fn repeat_suffix(s: Span) -> IResult<Span, u8> {
    map_res(
        tuple((tag("x"), digit1, space0)),
        |(_, n, _): (Span, Span, Span)| -> Result<u8> {
            let n: u32 = n.fragment().parse()?;
            if (2..=32).contains(&n) {
                Ok(n as u8)
            } else {
                Err(anyhow::anyhow!("x{} は 2〜32 の範囲外", n))
            }
        },
    )(s)
}

/// `xN` が付いていたら包む。付いていなければそのまま
fn repeated(expr: Expr, n: Option<u8>) -> Expr {
    match n {
        Some(n) => Expr::Repeat(Box::new(expr), n),
        None => expr,
    }
}

/// `( <小節列> )` のグループ。リピートの範囲を明示するための括弧で、
/// `Group` という型は持たない。`( )` は Display 側で導出する。
///
/// `pow(` などの関数は `apply_parser` が先に食う必要があるので、
/// 呼ぶ側の `alt` では必ず `apply_parser` を先に置くこと
#[tracable_parser]
fn group_parser(s: Span) -> IResult<Span, Expr> {
    map(
        tuple((
            tag("("),
            space0,
            many1(alt((apply_parser, group_parser, ref_parser, measure_parser))),
            space0,
            tag(")"),
            space0,
            opt(repeat_suffix),
            opt(measure_sep),
            space_or_line_ending_many0,
        )),
        |(_, _, items, _, _, _, n, _, _)| repeated(Expr::Seq(items), n),
    )(s)
}

/// `pow( <小節列> )` と `pat( "<パターン>", <小節列> )`
#[tracable_parser]
fn apply_parser(s: Span) -> IResult<Span, Expr> {
    map(
        tuple((
            alt((
                map(tuple((tag("pow("), space0)), |_| Func::Pow),
                map(tuple((tag("oct("), space0)), |_| Func::Oct),
                map(
                    tuple((
                        tag("pat("),
                        space0,
                        // 第1引数はリズムパターンそのものか、その定義への参照
                        alt((map(pattern_parser, Expr::Pattern), ref_atom)),
                        space0,
                        tag(","),
                        space0,
                    )),
                    |(_, _, arg, _, _, _)| Func::Pat(Box::new(arg)),
                ),
            )),
            // group_parser も並べる。`pow((C | D) x2)` のように関数の中で
            // グループを使えるようにする。`apply_parser` を先に試すので
            // `pow(` のタグが `(` のグループに食われることはない
            many1(alt((apply_parser, group_parser, ref_parser, measure_parser))),
            space0,
            tag(")"),
            space0,
            // `pow(C) x2` の `x2` は閉じ括弧の後ろ、行末の判定より前
            opt(repeat_suffix),
            // 閉じ括弧の直後が行末なら、この関数でひと行が終わる
            opt(peek(line_ending)),
            space_or_line_ending_many0,
        )),
        |(func, items, _, _, _, n, br, _)| {
            repeated(
                Expr::Apply(func, Box::new(Expr::Seq(items)), br.is_some()),
                n,
            )
        },
    )(s)
}

fn space_or_line_ending_many0(s: Span) -> IResult<Span, ()> {
    value((), many0(alt((tag(" "), line_ending))))(s)
}

#[tracable_parser]
fn measure_parser(s: Span) -> IResult<Span, Expr> {
    map(
        tuple((
            many1(delimited(space0, node_parser, space0)),
            // `Am x3 |` の `x3` は小節の中身と区切りの間にある。`node_parser`
            // は `x` で始まるものを受け付けないので、many1 はここで止まり、
            // `x3` がそのまま残る
            opt(repeat_suffix),
            measure_sep,
            space_or_line_ending_many0,
        )),
        |(nodes, n, br, _)| repeated(Expr::Measure(nodes, br), n),
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
        use crate::model::ast::Expr;
        use crate::model::pitch::Pitch;
        use crate::model::scale::Scale;
        // `key:` / `scale:` の値がそれぞれ正しく読まれること。
        // 平らな Directive 列ではなく、以降を包む Keyed / Scaled になった
        let ast = RechordImporter
            .import("key: C\nscale: dorian\nC |\n")
            .unwrap();
        let Expr::Seq(items) = &ast else { panic!() };
        let Expr::Keyed(Pitch::C, inner) = &items[0] else {
            panic!("key: は Keyed になる: {:?}", items[0])
        };
        let Expr::Seq(inner_items) = inner.as_ref() else {
            panic!("中身は Seq")
        };
        assert!(matches!(&inner_items[0], Expr::Scaled(Scale::Dorian, _)));
    }

    #[test]
    fn test_bad_directive_errors_not_panics() {
        use crate::import::{Importer, RechordImporter};
        // invalid pitch / scale must yield an Err (graceful), never panic
        assert!(RechordImporter.import("key: H\nC |\n").is_err());
        assert!(RechordImporter.import("scale: bogus\nC |\n").is_err());
        // 途中行の不正なディレクティブも握り潰さない。seq_parser は
        // directive_head の失敗を `if let Ok` で捨てるので、その行が
        // 小節として食われてしまわないことを固定しておく
        assert!(RechordImporter.import("C |\nkey: H\n").is_err());
        assert!(RechordImporter.import("C |\nscale: bogus\n").is_err());
        // 値なしも通さない
        assert!(RechordImporter.import("key:\nC |\n").is_err());
    }

    #[test]
    fn test_scale_directive_changes_pitch() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::{Expr, Node};
        use crate::model::key::Key;
        use crate::model::pitch::Pitch;
        // III resolves differently per scale: Major -> E, Dorian -> D#/Ds
        let major = RechordImporter.import("key: C\nIII |\n").unwrap().resolve();
        let dorian = RechordImporter
            .import("key: C\nscale: dorian\nIII |\n")
            .unwrap()
            .resolve();
        let first_key = |ast: &Expr| -> Key {
            let Expr::Seq(items) = ast else { panic!() };
            for it in items {
                if let Expr::Measure(nodes, _) = it {
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
    fn test_key_directive_wraps_the_rest() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::Expr;
        use crate::model::pitch::Pitch;
        let ast = RechordImporter.import("key: C\nC | Am |\n").unwrap();
        let Expr::Seq(items) = &ast else {
            panic!("not a seq")
        };
        // key: 以降は Keyed に包まれるので、トップの Seq は要素1つ
        assert_eq!(items.len(), 1);
        let Expr::Keyed(Pitch::C, inner) = &items[0] else {
            panic!("key: は Keyed になる: {:?}", items[0])
        };
        let Expr::Seq(measures) = inner.as_ref() else {
            panic!("中身は Seq")
        };
        assert_eq!(measures.len(), 2);
    }

    #[test]
    fn test_comment_before_key_stays_outside() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::Expr;
        let ast = RechordImporter.import("# memo\nkey: C\nC |\n").unwrap();
        let Expr::Seq(items) = &ast else {
            panic!("not a seq")
        };
        assert_eq!(items.len(), 2);
        assert!(matches!(&items[0], Expr::Comment(_)));
        assert!(matches!(&items[1], Expr::Keyed(_, _)));
    }

    #[test]
    fn test_scale_nests_inside_key() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::Expr;
        let ast = RechordImporter
            .import("key: C\nscale: dorian\nI |\n")
            .unwrap();
        let Expr::Seq(items) = &ast else {
            panic!("not a seq")
        };
        let Expr::Keyed(_, inner) = &items[0] else {
            panic!("not keyed")
        };
        let Expr::Seq(inner_items) = inner.as_ref() else {
            panic!("中身は Seq")
        };
        assert!(matches!(&inner_items[0], Expr::Scaled(_, _)));
    }

    #[test]
    fn test_pow_parser() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::{Expr, Func};
        let ast = RechordImporter.import("pow(C | E D)\n").unwrap();
        let Expr::Seq(items) = &ast else {
            panic!("not a seq")
        };
        assert_eq!(items.len(), 1);
        let Expr::Apply(Func::Pow, inner, _) = &items[0] else {
            panic!("not a pow apply: {:?}", items[0])
        };
        // 中身は Seq 1つ
        let Expr::Seq(measures) = inner.as_ref() else {
            panic!("apply の中身は Seq")
        };
        // C の小節と E D の小節
        assert_eq!(measures.len(), 2);
        let Expr::Measure(nodes, _) = &measures[0] else {
            panic!("not a measure")
        };
        assert_eq!(nodes.len(), 1);
        let Expr::Measure(nodes, _) = &measures[1] else {
            panic!("not a measure")
        };
        assert_eq!(nodes.len(), 2);
    }

    #[test]
    fn test_pat_parser() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::{Beat, Expr, Func};
        let ast = RechordImporter.import("pat(\"o.ox\", C | E D)\n").unwrap();
        let Expr::Seq(items) = &ast else {
            panic!("not a score")
        };
        let Expr::Apply(Func::Pat(arg), inner, _) = &items[0] else {
            panic!("not a pat apply: {:?}", items[0])
        };
        let Expr::Pattern(pattern) = arg.as_ref() else {
            panic!("pat の第1引数は Expr::Pattern: {:?}", arg)
        };
        assert_eq!(
            pattern.0,
            vec![Beat::Hit, Beat::Rest, Beat::Hit, Beat::Brush]
        );
        let Expr::Seq(inner) = inner.as_ref() else {
            panic!("apply の中身は Seq")
        };
        assert_eq!(inner.len(), 2);
    }

    #[test]
    fn test_pat_nested_in_pow() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::{Expr, Func};
        let ast = RechordImporter.import("pat(\"oxox\", pow(C))\n").unwrap();
        let Expr::Seq(items) = &ast else {
            panic!("not a score")
        };
        let Expr::Apply(Func::Pat(_), inner, _) = &items[0] else {
            panic!("not a pat apply")
        };
        let Expr::Seq(inner) = inner.as_ref() else {
            panic!("apply の中身は Seq")
        };
        assert!(matches!(&inner[0], Expr::Apply(Func::Pow, _, _)));
    }

    #[test]
    fn test_apply_followed_by_measure() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::Expr;
        let ast = RechordImporter.import("pow(C) F |\n").unwrap();
        let Expr::Seq(items) = &ast else {
            panic!("not a score")
        };
        assert_eq!(items.len(), 2);
        assert!(matches!(&items[0], Expr::Apply(_, _, _)));
        assert!(matches!(&items[1], Expr::Measure(_, _)));
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

    #[test]
    fn test_define_and_ref_parse() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::Expr;
        let ast = RechordImporter
            .import("$riff = pow(C | E D)\n$riff | C |\n")
            .unwrap();
        let Expr::Seq(items) = &ast else { panic!("not a seq") };
        // 定義は 0 小節を返す単位元。スコープ包みではないので平らに並ぶ
        let Expr::Define(name, body) = &items[0] else {
            panic!("1つ目は Define: {:?}", items[0])
        };
        assert_eq!(name, "riff");
        assert!(matches!(body.as_ref(), Expr::Apply(_, _, _)));
        // 定義の後ろが入れ子 Seq になっていないこと
        assert!(
            !matches!(&items[1], Expr::Seq(_)),
            "定義の後ろは入れ子 Seq にしない: {:?}", items[1]
        );
    }

    #[test]
    fn test_multiple_defines_stay_flat() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::Expr;
        let ast = RechordImporter.import("$a = C\n$b = D\nC |\n").unwrap();
        let Expr::Seq(items) = &ast else { panic!("not a seq") };
        // 定義2つと小節。ネストしない
        let defines = items.iter().filter(|e| matches!(e, Expr::Define(_, _))).count();
        assert_eq!(defines, 2, "定義が平らに並ばない: {:?}", items);
        assert!(
            items.iter().all(|e| !matches!(e, Expr::Seq(_))),
            "入れ子 Seq が残っている: {:?}", items
        );
    }

    #[test]
    fn test_dollar_is_not_a_chord_node() {
        use crate::import::{Importer, RechordImporter};
        // `$` は既存のコード表記のどれとも衝突しない。コードの一部として
        // 吸われるなら、この入力が通ってしまう
        assert!(RechordImporter.import("C$ |\n").is_err());
        // `$` に続く名前は1文字以上
        assert!(RechordImporter.import("$ |\n").is_err());
    }

    #[test]
    fn test_ref_in_measure_position() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::Expr;
        let ast = RechordImporter.import("$riff = C\n$riff | Am |\n").unwrap();
        // Ref が木のどこかに現れること
        fn has_ref(e: &Expr, want: &str) -> bool {
            match e {
                Expr::Ref(n) => n == want,
                Expr::Seq(items) => items.iter().any(|i| has_ref(i, want)),
                Expr::Apply(_, inner, _) => has_ref(inner, want),
                Expr::Define(_, body) => has_ref(body, want),
                Expr::Keyed(_, inner) | Expr::Scaled(_, inner) => has_ref(inner, want),
                _ => false,
            }
        }
        assert!(has_ref(&ast, "riff"), "{:?}", ast);
    }

    #[test]
    fn test_pattern_define_and_ref_in_pat() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::{Expr, Func};
        let ast = RechordImporter
            .import("$eight = \"o.o.o.o.\"\npat($eight, C)\n")
            .unwrap();
        let Expr::Seq(items) = &ast else { panic!("not a seq") };
        let Expr::Define(name, body) = &items[0] else { panic!("not a define") };
        assert_eq!(name, "eight");
        assert!(matches!(body.as_ref(), Expr::Pattern(_)));
        fn find_pat_arg(e: &Expr) -> Option<&Expr> {
            match e {
                Expr::Apply(Func::Pat(arg), _, _) => Some(arg.as_ref()),
                Expr::Seq(items) => items.iter().find_map(find_pat_arg),
                Expr::Define(_, body) => find_pat_arg(body),
                _ => None,
            }
        }
        let arg = find_pat_arg(&ast).expect("pat が見つからない");
        assert!(matches!(arg, Expr::Ref(n) if n == "eight"), "{:?}", arg);
    }

    #[test]
    fn test_japanese_define_name() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::Expr;
        let ast = RechordImporter.import("$Aメロ = C | Am |\n$Aメロ\n").unwrap();
        let Expr::Seq(items) = &ast else { panic!("not a seq") };
        let Expr::Define(name, _) = &items[0] else { panic!("not a define") };
        assert_eq!(name, "Aメロ");
    }

    #[test]
    fn test_define_body_stops_at_line_end() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::Expr;
        // 定義の本体は1行。次の行を飲み込まないこと
        let ast = RechordImporter.import("$a = C | E |\nAm | F |\n").unwrap();
        let Expr::Seq(items) = &ast else { panic!("not a seq") };
        // 定義は平らに並ぶので、後ろの Am / F はトップに2小節として残る
        assert_eq!(items.len(), 3, "定義と、その後ろの2小節: {:?}", items);
        let Expr::Define(_, body) = &items[0] else { panic!("not a define") };
        fn count_measures(e: &Expr) -> usize {
            match e {
                Expr::Measure(_, _) => 1,
                Expr::Seq(items) => items.iter().map(count_measures).sum(),
                Expr::Apply(_, inner, _) => count_measures(inner),
                _ => 0,
            }
        }
        // 本体は C と E の2小節だけ。Am / F を飲み込んでいない
        assert_eq!(count_measures(body), 2, "本体が次の行まで伸びている: {:?}", body);
    }

    #[test]
    fn test_repeat_on_measure() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::Expr;
        let ast = RechordImporter.import("C | Am x3 |\n").unwrap();
        fn find_repeat(e: &Expr) -> Option<u8> {
            match e {
                Expr::Repeat(_, n) => Some(*n),
                Expr::Seq(items) => items.iter().find_map(find_repeat),
                _ => None,
            }
        }
        assert_eq!(find_repeat(&ast), Some(3));
    }

    #[test]
    fn test_repeat_on_group() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::Expr;
        let ast = RechordImporter.import("(C | Am | F | G) x4\n").unwrap();
        fn find_repeat(e: &Expr) -> Option<(usize, u8)> {
            match e {
                Expr::Repeat(inner, n) => match inner.as_ref() {
                    Expr::Seq(items) => Some((items.len(), *n)),
                    _ => Some((1, *n)),
                },
                Expr::Seq(items) => items.iter().find_map(find_repeat),
                _ => None,
            }
        }
        assert_eq!(find_repeat(&ast), Some((4, 4)));
    }

    #[test]
    fn test_repeat_on_ref() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::Expr;
        let ast = RechordImporter.import("$a = C\n$a x2\n").unwrap();
        fn has_repeat_of_ref(e: &Expr) -> bool {
            match e {
                Expr::Repeat(inner, 2) => matches!(inner.as_ref(), Expr::Ref(n) if n == "a"),
                Expr::Seq(items) => items.iter().any(has_repeat_of_ref),
                _ => false,
            }
        }
        assert!(has_repeat_of_ref(&ast), "{:?}", ast);
    }

    #[test]
    fn test_repeat_rejects_out_of_range() {
        use crate::import::{Importer, RechordImporter};
        // x0 / x1 は意味がないのでエラー
        assert!(RechordImporter.import("C x0 |\n").is_err());
        assert!(RechordImporter.import("C x1 |\n").is_err());
        // 33 以上もエラー
        assert!(RechordImporter.import("C x33 |\n").is_err());
    }

    #[test]
    fn test_group_without_repeat() {
        use crate::import::{Importer, RechordImporter};
        assert!(RechordImporter.import("(C | Am)\n").is_ok());
    }

    #[test]
    fn test_paren_is_not_confused_with_func() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::{Expr, Func};
        // pow( は関数、( はグループ
        let ast = RechordImporter.import("pow(C) (D | E) x2\n").unwrap();
        fn has_pow(e: &Expr) -> bool {
            match e {
                Expr::Apply(Func::Pow, _, _) => true,
                Expr::Seq(items) => items.iter().any(has_pow),
                Expr::Repeat(inner, _) => has_pow(inner),
                _ => false,
            }
        }
        assert!(has_pow(&ast), "{:?}", ast);
    }

    #[test]
    fn test_repeat_in_define_body() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::Expr;
        // 定義の本体にもリピートが書ける
        let ast = RechordImporter.import("$a = C x2 | D |\n$a\n").unwrap();
        let Expr::Seq(items) = &ast else { panic!("not a seq") };
        let Expr::Define(_, body) = &items[0] else { panic!("not a define") };
        fn has_repeat(e: &Expr) -> bool {
            match e {
                Expr::Repeat(_, _) => true,
                Expr::Seq(items) => items.iter().any(has_repeat),
                _ => false,
            }
        }
        assert!(has_repeat(body), "定義の本体にリピートが無い: {:?}", body);
    }

    #[test]
    fn test_group_inside_func() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::{Expr, Func};
        // 関数の中にグループとリピートを書ける
        let ast = RechordImporter.import("pow((C | D) x2)\n").unwrap();
        let Expr::Seq(items) = &ast else { panic!("not a seq") };
        let Expr::Apply(Func::Pow, inner, _) = &items[0] else {
            panic!("not a pow: {:?}", items[0])
        };
        let Expr::Seq(inner) = inner.as_ref() else { panic!("apply の中身は Seq") };
        assert!(
            matches!(&inner[0], Expr::Repeat(_, 2)),
            "関数の中のグループがリピートされていない: {:?}", inner[0]
        );
    }

    #[test]
    fn test_nested_group() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::Expr;
        // グループは入れ子にできる
        let ast = RechordImporter.import("((C | Am) x2 | D) x3\n").unwrap();
        fn outer(e: &Expr) -> Option<&Expr> {
            match e {
                Expr::Repeat(_, _) => Some(e),
                Expr::Seq(items) => items.iter().find_map(outer),
                _ => None,
            }
        }
        let Some(Expr::Repeat(inner, 3)) = outer(&ast) else {
            panic!("外側の x3 が無い: {:?}", ast)
        };
        let Expr::Seq(items) = inner.as_ref() else {
            panic!("外側の中身は Seq: {:?}", inner)
        };
        // 内側の (C | Am) x2 と D の2項目
        assert_eq!(items.len(), 2, "{:?}", items);
        assert!(matches!(&items[0], Expr::Repeat(_, 2)), "{:?}", items[0]);
        assert!(matches!(&items[1], Expr::Measure(_, _)), "{:?}", items[1]);
    }

    #[test]
    fn test_pat_argument_is_a_pattern_expr() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::{Expr, Func};
        let ast = RechordImporter.import("pat(\"oxox\", C)\n").unwrap();
        let Expr::Seq(items) = &ast else { panic!("not a seq") };
        let Expr::Apply(Func::Pat(arg), _, _) = &items[0] else {
            panic!("not a pat apply: {:?}", items[0])
        };
        let Expr::Pattern(p) = arg.as_ref() else {
            panic!("pat の第1引数は Expr::Pattern: {:?}", arg)
        };
        assert_eq!(p.to_string(), "oxox");
    }
}
