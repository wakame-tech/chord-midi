use super::parser_util::{capture, Span};
use crate::model::ast::{ChordExpr, ChordMap, ChordNode, Node};
use crate::model::key::Key;
use crate::model::modifier::Modifier;
use crate::model::pitch::{Accidental, Pitch};
use crate::model::scale::Degree;
use anyhow::Result;
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::{digit1, one_of, space0, space1};
use nom::combinator::{map, map_res, opt, value};
use nom::multi::{many0, separated_list1};
use nom::sequence::{delimited, preceded, tuple};
use nom::IResult;
use nom_tracable::tracable_parser;
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::BTreeSet;
use std::str::FromStr;

pub static PITCH_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^([CDEFGAB][#b]?)").unwrap());

static DEGREE_NUMBER_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(3|5|6|7|9|11|13)").unwrap());

pub static DEGREE_NAME_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(IV|VII|VI|V|III|II|I)").unwrap());

pub static DEGREE_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(IV|VII|VI|V|III|II|I)[#b]?").unwrap());

pub fn parser_roman_num(s: &str) -> Result<u8> {
    match s {
        "I" => Ok(1),
        "II" => Ok(2),
        "III" => Ok(3),
        "IV" => Ok(4),
        "V" => Ok(5),
        "VI" => Ok(6),
        "VII" => Ok(7),
        _ => Err(anyhow::anyhow!("invalid degree: {}", s)),
    }
}

/// `blk(` `sub(` `dom(` の入れ子。中身も `ChordExpr` なので再帰する。
///
/// 再帰の前に必ず `blk(` などのタグを消費するので、入力は毎回短くなる
/// (= 無限再帰にならない)
#[tracable_parser]
fn chord_expr_parser(s: Span) -> IResult<Span, ChordExpr> {
    alt((
        map(delimited(tag("blk("), chord_expr_parser, tag(")")), |e| {
            ChordExpr::Map(ChordMap::Blk, Box::new(e))
        }),
        map(delimited(tag("sub("), chord_expr_parser, tag(")")), |e| {
            ChordExpr::Map(ChordMap::Sub, Box::new(e))
        }),
        map(delimited(tag("dom("), chord_expr_parser, tag(")")), |e| {
            ChordExpr::Map(ChordMap::Dom, Box::new(e))
        }),
        map(chord_node_parser, ChordExpr::Chord),
    ))(s)
}

/// `tab()` に書けるフレットの上限。ギターの実用上限に合わせる
const MAX_TAB_FRET: u8 = 24;

/// `tab(...)` の中の1弦ぶん。`x` はミュート、それ以外は 0〜24 の10進
fn fret_parser(s: Span) -> IResult<Span, Option<u8>> {
    alt((
        value(None, tag("x")),
        map_res(digit1, |d: Span| -> Result<Option<u8>> {
            let n: u32 = d.fragment().parse()?;
            if n <= MAX_TAB_FRET as u32 {
                Ok(Some(n as u8))
            } else {
                Err(anyhow::anyhow!("フレットは 0〜{}: {}", MAX_TAB_FRET, n))
            }
        }),
    ))(s)
}

/// `tab(8 6 6 x x x)`。6弦→1弦の順でちょうど6本書く
#[tracable_parser]
fn tab_parser(s: Span) -> IResult<Span, Node> {
    map_res(
        delimited(
            tuple((tag("tab("), space0)),
            separated_list1(space1, fret_parser),
            tuple((space0, tag(")"))),
        ),
        |frets: Vec<Option<u8>>| -> Result<Node> {
            let len = frets.len();
            let frets: [Option<u8>; 6] = frets
                .try_into()
                .map_err(|_| anyhow::anyhow!("tab は6弦ぶん書くこと: {} 本だった", len))?;
            Ok(Node::Tab(frets, false))
        },
    )(s)
}

#[tracable_parser]
pub fn node_parser(s: Span) -> IResult<Span, Node> {
    alt((
        // `tab(` は他のどの枝とも先頭が重ならないが、コード名の枝より
        // 前に置いて読み落としが起きないようにする
        tab_parser,
        map(tag("="), |_| Node::Sustain),
        map(tag("_"), |_| Node::Rest),
        map(tag("%"), |_| Node::Repeat),
        map(tag("N.C."), |_| Node::Rest),
        // arp と s25 は Node を返すので、chord_expr_parser の中には現れない。
        // `arp(s25(V7))` や `blk(arp(I))` がパースエラーになるのはこの帰結
        map(
            delimited(tag("arp("), chord_expr_parser, tag(")")),
            Node::Arpeggio,
        ),
        map(
            delimited(tag("s25("), chord_expr_parser, tag(")")),
            Node::Split,
        ),
        map(chord_expr_parser, Node::Chord),
    ))(s)
}

#[tracable_parser]
fn key_parser(s: Span) -> IResult<Span, Key> {
    alt((
        map(pitch_parser, Key::Absolute),
        map(degree_parser, Key::Relative),
    ))(s)
}

#[tracable_parser]
fn chord_node_parser(s: Span) -> IResult<Span, ChordNode> {
    map(
        tuple((
            key_parser,
            many0(modifier_parser),
            opt(tensions_parser),
            opt(preceded(tag("/"), key_parser)),
            // `@N` は押さえ方への指示。1〜6 以外は読まないので、`A@7` は
            // "@7" が余って measure のパースごと失敗する
            opt(preceded(tag("@"), one_of("123456"))),
        )),
        |(key, modifiers, tensions, on, root_string)| ChordNode {
            key,
            modifiers: BTreeSet::from_iter(
                vec![Modifier::Major(5)]
                    .into_iter()
                    .chain(modifiers.into_iter())
                    .chain(tensions.into_iter().flatten()),
            ),
            on,
            root_string: root_string.map(|c| c as u8 - b'0'),
        },
    )(s)
}

#[tracable_parser]
fn degree_number_parser(s: Span) -> IResult<Span, u8> {
    map(capture(DEGREE_NUMBER_REGEX.to_owned()), |cap| {
        cap[1].parse::<u8>().unwrap()
    })(s)
}

#[tracable_parser]
fn degree_name_parser(s: Span) -> IResult<Span, u8> {
    map(capture(DEGREE_NAME_REGEX.to_owned()), |cap| {
        parser_roman_num(&cap[1]).unwrap()
    })(s)
}

#[tracable_parser]
fn accidental_parser(s: Span) -> IResult<Span, Accidental> {
    map(capture(Regex::new(r"^([b#])").unwrap()), |cap| {
        Accidental::from_str(&cap[1]).unwrap()
    })(s)
}

#[tracable_parser]
fn degree_parser(s: Span) -> IResult<Span, Degree> {
    map(
        tuple((opt(accidental_parser), degree_name_parser)),
        |(a, d)| Degree(d, a.unwrap_or(Accidental::Natural)),
    )(s)
}

#[tracable_parser]
fn pitch_parser(s: Span) -> IResult<Span, Pitch> {
    map(capture(PITCH_REGEX.to_owned()), |cap| {
        Pitch::from_str(&cap[1]).unwrap()
    })(s)
}

#[tracable_parser]
fn modifier_parser(s: Span) -> IResult<Span, Modifier> {
    alt((
        map(alt((tag("-5"), tag("b5"))), |_| Modifier::Flat5th),
        map(tag("sus2"), |_| Modifier::Sus2),
        map(tag("sus4"), |_| Modifier::Sus4),
        map(tag("dim7"), |_| Modifier::Dim7),
        // `omit` は dim の別名 `o` より先に試す。後ろに置くと
        // `Comit3` の `o` が dim に食われて読めなくなる
        map(
            tuple((alt((tag("omit"), tag("no"))), degree_number_parser)),
            |(_, d)| Modifier::Omit(d),
        ),
        map(alt((tag("dim"), tag("o"))), |_| Modifier::Dim),
        map(tag("aug7"), |_| Modifier::Aug7),
        map(alt((tag("aug"), tag("+"))), |_| Modifier::Aug),
        map(tuple((tag("add"), degree_number_parser)), |(_, d)| {
            Modifier::Add(d)
        }),
        map(tag("mM7"), |_| Modifier::MinorMajaor7),
        map(
            tuple((alt((tag("maj"), tag("M"))), opt(degree_number_parser))),
            |(_, d)| Modifier::Major(d.unwrap_or(5)),
        ),
        map(tuple((tag("m"), opt(degree_number_parser))), |(_, d)| {
            Modifier::Minor(d.unwrap_or(5))
        }),
        map(
            tuple((accidental_parser, degree_number_parser)),
            |(a, d)| Modifier::Tension(Degree(d, a)),
        ),
        map(degree_number_parser, Modifier::Dominant),
    ))(s)
}

#[tracable_parser]
fn tensions_parser(s: Span) -> IResult<Span, Vec<Modifier>> {
    map(
        delimited(
            tag("("),
            separated_list1(
                tag(","),
                tuple((opt(accidental_parser), degree_number_parser)),
            ),
            tag(")"),
        ),
        |tensions| {
            tensions
                .into_iter()
                .map(|(a, d)| Modifier::Tension(Degree(d, a.unwrap_or(Accidental::Natural))))
                .collect()
        },
    )(s)
}

#[cfg(test)]
mod tests {
    use super::chord_node_parser;
    use anyhow::Result;
    use nom_locate::LocatedSpan;
    use nom_tracable::TracableInfo;

    fn span<'a>(s: &'a str) -> LocatedSpan<&'a str, TracableInfo> {
        LocatedSpan::new_extra(s, TracableInfo::new())
    }

    /// parse a chord and return its (0-based, sorted) semitone set
    fn semis(code: &str) -> Vec<u8> {
        let (rest, node) = chord_node_parser(span(code)).unwrap();
        assert_eq!(
            rest.into_fragment(),
            "",
            "parser did not consume all of {:?}",
            code
        );
        node.to_chord().unwrap().semitones.iter().cloned().collect()
    }

    #[test]
    fn test_chord_semitones_table() {
        // reference chord tones (root = 0). Verifies parser + Chord::modify together.
        let cases: &[(&str, &[u8])] = &[
            ("C", &[0, 4, 7]),
            ("Cm", &[0, 3, 7]),
            ("C7", &[0, 4, 7, 10]),    // dominant 7 (flat 7)
            ("Cmaj7", &[0, 4, 7, 11]), // major 7
            ("CM7", &[0, 4, 7, 11]),   // M7 == maj7
            ("Cm7", &[0, 3, 7, 10]),
            ("C9", &[0, 4, 7, 10, 14]), // dominant 9
            ("Cmaj9", &[0, 4, 7, 11, 14]),
            ("Cm9", &[0, 3, 7, 10, 14]),
            ("C11", &[0, 4, 7, 10, 14, 17]),
            ("C13", &[0, 4, 7, 10, 14, 17, 21]),
            ("C6", &[0, 4, 7, 9]),
            ("Csus2", &[0, 2, 7]),
            ("Csus4", &[0, 5, 7]),
            ("Cdim", &[0, 3, 6]),
            ("Cdim7", &[0, 3, 6, 9]), // fully diminished (bb7)
            ("Caug", &[0, 4, 8]),
            ("Caug7", &[0, 4, 8, 10]), // 7#5
            ("CmM7", &[0, 3, 7, 11]),
            ("Cadd9", &[0, 4, 7, 14]),
            ("C7(b9)", &[0, 4, 7, 10, 13]),
            ("C7(#9)", &[0, 4, 7, 10, 15]),
            ("C7(b13)", &[0, 4, 7, 10, 20]),
        ];
        for (code, expected) in cases {
            assert_eq!(&semis(code), expected, "chord {:?}", code);
        }
    }

    #[test]
    fn test_omit_is_not_shadowed_by_dim() {
        let semitones = |code: &str| {
            let (_r, node) = chord_node_parser(span(code)).unwrap();
            node.to_chord()
                .unwrap()
                .semitones
                .into_iter()
                .collect::<Vec<_>>()
        };
        // omit が読めること (3rd が抜けてルートと5thだけになる)
        assert_eq!(semitones("Comit3"), vec![0, 7]);
        // 別名の no も従来どおり
        assert_eq!(semitones("Cno3"), vec![0, 7]);
        // `o` 単体は今までどおり dim
        assert_eq!(semitones("Co"), vec![0, 3, 6]);
    }

    #[test]
    fn test_chord_display_roundtrip() {
        // parse -> Display -> parse must preserve the chord tones
        for code in [
            "C", "C7", "Cmaj7", "Cm7", "Csus2", "Csus4", "Cdim7", "Caug7", "C7(b9)", "C7(#9)",
            "C7(b13)", "C7(9)", "C7(9,13)", "CM7(9)",
        ] {
            let (_r, node) = chord_node_parser(span(code)).unwrap();
            let text = format!("{}", node);
            let (_r2, node2) = chord_node_parser(span(&text)).unwrap_or_else(|_| {
                panic!("re-parse of displayed {:?} (from {:?}) failed", text, code)
            });
            assert_eq!(
                node.to_chord().unwrap().semitones,
                node2.to_chord().unwrap().semitones,
                "roundtrip {:?} -> {:?}",
                code,
                text
            );
        }
    }

    #[test]
    fn test_tentions_parser() -> Result<()> {
        for tension in ["(b9)", "(#9)", "(b5)", "(#5)", "(b13)", "(b9,#11)"] {
            let span = span(tension);
            let (res, _ast) = super::tensions_parser(span)?;
            assert_eq!(res.into_fragment(), "");
        }
        Ok(())
    }

    #[test]
    fn test_transform_parser() {
        use crate::model::ast::{ChordExpr, Node};
        for code in ["blk(F)", "sub(G7)", "dom(Dm7)"] {
            let (rest, node) = super::node_parser(span(code)).unwrap();
            assert_eq!(rest.into_fragment(), "", "{}", code);
            assert!(
                matches!(node, Node::Chord(ChordExpr::Map(_, _))),
                "{}",
                code
            );
        }
        let (rest, node) = super::node_parser(span("s25(G7)")).unwrap();
        assert_eq!(rest.into_fragment(), "");
        assert!(matches!(node, Node::Split(_)));
    }

    #[test]
    fn test_arp_can_wrap_a_transform() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::{ChordExpr, ChordMap, Expr, Node};
        let ast = RechordImporter.import("key: C\narp(blk(I)) |\n").unwrap();
        fn first_node(e: &Expr) -> Option<&Node> {
            match e {
                Expr::Measure(nodes, _) => nodes.first(),
                Expr::Seq(items) => items.iter().find_map(first_node),
                Expr::Keyed(_, inner) | Expr::Scaled(_, inner) => first_node(inner),
                _ => None,
            }
        }
        let Some(Node::Arpeggio(expr)) = first_node(&ast) else {
            panic!("arp にならない: {:?}", ast)
        };
        assert!(
            matches!(expr, ChordExpr::Map(ChordMap::Blk, _)),
            "中身が blk の Map にならない: {:?}",
            expr
        );
    }

    #[test]
    fn test_transforms_can_nest() {
        use crate::import::{Importer, RechordImporter};
        assert!(RechordImporter.import("blk(sub(G7)) |\n").is_ok());
        assert!(RechordImporter.import("arp(blk(sub(G7))) |\n").is_ok());
        assert!(RechordImporter.import("s25(blk(C)) |\n").is_ok());
    }

    #[test]
    fn test_node_returning_functions_cannot_nest() {
        use crate::import::{Importer, RechordImporter};
        // arp と s25 は Node を返すので ChordExpr の位置に置けない
        assert!(RechordImporter.import("arp(s25(G7)) |\n").is_err());
        assert!(RechordImporter.import("blk(arp(C)) |\n").is_err());
        assert!(RechordImporter.import("s25(s25(G7)) |\n").is_err());
        assert!(RechordImporter.import("blk(s25(G7)) |\n").is_err());
        assert!(RechordImporter.import("arp(arp(C)) |\n").is_err());
    }

    #[test]
    fn test_arpeggio_parser() -> Result<()> {
        for code in ["arp(C)", "arp(Cm7)", "arp(C/E)"] {
            let span = span(code);
            let (res, node) = super::node_parser(span)?;
            assert_eq!(res.into_fragment(), "");
            assert!(matches!(node, crate::model::ast::Node::Arpeggio(_)));
        }
        Ok(())
    }

    #[test]
    fn test_chord_node_parser() -> Result<()> {
        for chord in [
            "C",
            "Cm",
            "CmM7",
            "Csus2",
            "Csus4",
            "C-5",
            "Caug",
            "Caug7",
            "Cdim",
            "Cdim7",
            "C/D",
            "C7sus4(b9)",
            "C7(13)",
            "AbmM7/Eb",
        ] {
            let span = span(chord);
            let (res, _ast) = chord_node_parser(span)?;
            assert_eq!(res.into_fragment(), "");
        }
        Ok(())
    }

    #[test]
    fn test_transform_display_roundtrip() {
        for code in ["blk(F)", "s25(G7)", "sub(G7)", "dom(Dm7)"] {
            let (_r, node) = super::node_parser(span(code)).unwrap();
            let text = format!("{}", node);
            let (_r2, node2) = super::node_parser(span(&text)).unwrap();
            assert_eq!(node, node2, "{} -> {}", code, text);
        }
    }

    #[test]
    fn test_root_string_parser() {
        use crate::model::key::Key;
        use crate::model::pitch::Pitch;

        let (_r, node) = chord_node_parser(span("A@6")).unwrap();
        assert_eq!(node.root_string, Some(6));

        // @N は /on より後ろ
        let (_r, node) = chord_node_parser(span("A/E@6")).unwrap();
        assert_eq!(node.on, Some(Key::Absolute(Pitch::E)));
        assert_eq!(node.root_string, Some(6));

        let (_r, node) = chord_node_parser(span("A")).unwrap();
        assert_eq!(node.root_string, None);
    }

    #[test]
    fn test_root_string_out_of_range_is_rejected() {
        use crate::import::{Importer, RechordImporter};
        // 1〜6 以外は読めず、余りが残るのでパース全体が失敗する
        assert!(RechordImporter.import("A@0 |\n").is_err());
        assert!(RechordImporter.import("A@7 |\n").is_err());
    }
}
