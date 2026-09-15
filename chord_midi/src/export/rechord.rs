use super::{Exporter, RechordExporter};
use crate::model::ast::{Beat, ChordNode, Expr, Func, Node, Pattern};
use crate::model::modifier::Modifier;
use std::{fmt::Display, io::Write};

impl Exporter for RechordExporter {
    fn export(&self, f: &mut impl Write, ast: Expr) -> anyhow::Result<()> {
        writeln!(f, "{}", ast)?;
        Ok(())
    }
}

/// 式を関数の引数や `( )` の中に埋め込む形にする。
///
/// 小節の Display は末尾に `" | "` を出すので、そのままでは
/// `pow(C | E D | )` になってしまう。最後の区切りだけを落とす
fn inline(expr: &Expr) -> String {
    let s = expr.to_string();
    let s = s.trim_end();
    s.strip_suffix('|').unwrap_or(s).trim_end().to_string()
}

/// 本体を書いて、行末なら改行を、そうでなければ次の式との空白を1つ置く
fn write_tail(
    f: &mut std::fmt::Formatter<'_>,
    body: std::fmt::Arguments<'_>,
    br: bool,
) -> std::fmt::Result {
    write!(f, "{}", body)?;
    if br {
        writeln!(f)
    } else {
        write!(f, " ")
    }
}

/// 小節を1つ書く。
///
/// `xN` は小節の中身と区切りの間に置く (`C x4 |`)。パーサもその位置で
/// 読むので、`C | x4` のように区切りの後ろへ出すと読み戻せない
fn write_measure(
    f: &mut std::fmt::Formatter<'_>,
    nodes: &[Node],
    repeat: Option<u8>,
    br: bool,
) -> std::fmt::Result {
    let body = nodes
        .iter()
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    write!(f, "{}", body)?;
    if let Some(n) = repeat {
        write!(f, " x{}", n)?;
    }
    write!(f, " | ")?;
    if br {
        writeln!(f)?;
    }
    Ok(())
}

/// `pow(...)` / `oct(...)` / `pat(...)` を書く。
///
/// `xN` は閉じ括弧の後ろ、行末の判定より前に置く (`pow(C) x2`)
fn write_apply(
    f: &mut std::fmt::Formatter<'_>,
    func: &Func,
    inner: &Expr,
    repeat: Option<u8>,
    br: bool,
) -> std::fmt::Result {
    let inner = inline(inner);
    match func {
        Func::Pow => write!(f, "pow({})", inner)?,
        Func::Oct => write!(f, "oct({})", inner)?,
        // パターンの引用符はここで付ける。`Pattern` 自身の Display は
        // 中身だけを出すので二重にはならない
        Func::Pat(arg) => match arg.as_ref() {
            Expr::Pattern(p) => write!(f, "pat(\"{}\", {})", p, inner)?,
            other => write!(f, "pat({}, {})", other, inner)?,
        },
    }
    if let Some(n) = repeat {
        write!(f, " x{}", n)?;
    }
    if br {
        writeln!(f)
    } else {
        write!(f, " ")
    }
}

impl Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Expr::Seq(nodes) => {
                for node in nodes {
                    write!(f, "{}", node)?;
                    // 小節の位置に置かれた参照は `$a | C |` のように `|` で
                    // 閉じる。`Ref` 自身の Display が `$a` だけなのは、
                    // `pat($a, C)` の引数としても使われるため。
                    // 行末フラグも、区切りを出すこの場所でしか見られない
                    if let Expr::Ref(_, br) = node {
                        write!(f, " | ")?;
                        if *br {
                            writeln!(f)?;
                        }
                    }
                }
                Ok(())
            }
            Expr::Comment(comment) => {
                writeln!(f, "# {}", comment)
            }
            Expr::Keyed(p, inner) => {
                writeln!(f, "key: {}", p)?;
                write!(f, "{}", inner)
            }
            Expr::Scaled(s, inner) => {
                writeln!(f, "scale: {}", s)?;
                write!(f, "{}", inner)
            }
            Expr::Apply(func, inner, br) => write_apply(f, func, inner, None, *br),
            Expr::Measure(nodes, br) => write_measure(f, nodes, None, *br),
            Expr::Pattern(p) => write!(f, "\"{}\"", p),
            Expr::Define(name, body, br) => {
                // 定義は1行。本体が小節で終わると末尾に `" | "` が付くので
                // 行末の空白だけ落として `$a = C | Am |` の形に揃える。
                // 行末コメントが続くときだけ改行せず、空白1つで繋ぐ
                write_tail(
                    f,
                    format_args!("${} = {}", name, body.to_string().trim_end()),
                    *br,
                )
            }
            // 区切りも行末も付けない素の形。`pat($a, C)` の引数にも使われる。
            // 小節の位置に置かれたときの `|` と改行は `Seq` 側が出す
            Expr::Ref(name, _) => write!(f, "${}", name),
            // `expand` が置いた跡。書き戻す記法は `$name` 側にあるので、
            // ここは展開された中身をそのまま出す。`resolve()` 前の AST に
            // `Section` は現れないので round-trip には効かない
            Expr::Section(_, inner) => write!(f, "{}", inner),
            Expr::Repeat(inner, n, br) => {
                // 1項目だけの `Seq` は素の式と区別が付かない。`(C) x2` が
                // `C x2` と同じ意味になるよう、括弧の判定の前に剥がす
                let mut target = inner.as_ref();
                while let Expr::Seq(items) = target {
                    if items.len() == 1 {
                        target = &items[0];
                    } else {
                        break;
                    }
                }
                // 行末は必ず `xN` の後ろに来るので、中身が持つフラグではなく
                // リピート自身の `br` を見る。`(C) x2` のように 1項目の `Seq`
                // を剥がした後は、中身のフラグはそもそも当てにならない
                match target {
                    // 小節 / 関数 / 参照の繰り返しに括弧は要らない
                    Expr::Measure(nodes, _) => write_measure(f, nodes, Some(*n), *br),
                    Expr::Apply(func, body, _) => write_apply(f, func, body, Some(*n), *br),
                    // 参照とグループの繰り返しも小節の位置にあるので `|` で
                    // 閉じる。`$a` が `$a |` に、`C` が `C |` になるのと同じ。
                    // 閉じないと `$a x2 | D |` の `|` が書き戻しで落ちる。
                    // `Apply` だけ別なのは、パーサが `pow(C) x2 |` の `|` を
                    // 受け付けない (apply_parser に measure_sep が無い) ため
                    Expr::Ref(name, _) => write_tail(f, format_args!("${} x{} |", name, n), *br),
                    // 2小節以上のまとまりと、入れ子の繰り返しにだけ `( )` を
                    // 出す。括弧が無いと `xN` がどこに掛かるか読み戻せない
                    Expr::Seq(_) | Expr::Repeat(_, _, _) => {
                        write_tail(f, format_args!("({}) x{} |", inline(target), n), *br)
                    }
                    other => write_tail(f, format_args!("{} x{}", inline(other), n), *br),
                }
            }
        }
    }
}

impl Display for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Node::Chord(chord) => write!(f, "{}", chord),
            Node::Brush(chord) => write!(f, "{}", chord),
            Node::Arpeggio(chord) => write!(f, "arp({})", chord),
            Node::Transform(t, c) => write!(f, "{}({})", transform_prefix(t), c),
            Node::Rest => write!(f, "N.C."),
            Node::Sustain => write!(f, "="),
            Node::Repeat => write!(f, "%"),
        }
    }
}

impl Display for Pattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for beat in &self.0 {
            let c = match beat {
                Beat::Hit => 'o',
                Beat::Rest => '.',
                Beat::Brush => 'x',
            };
            write!(f, "{}", c)?;
        }
        Ok(())
    }
}

fn transform_prefix(t: &crate::model::ast::Transform) -> &'static str {
    use crate::model::ast::Transform::*;
    match t {
        S25 => "s25",
        Sub => "sub",
        Dom => "dom",
        Blk => "blk",
    }
}

impl Display for ChordNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // テンションはまとめて `(9,b13)` の形で出す。パーサが読むのはこの形
        // だけなので、素で並べると `C7(9)` が `C79` になって読み戻せない
        let (tensions, mods): (Vec<_>, Vec<_>) = self
            .modifiers
            .iter()
            .partition(|m| matches!(m, Modifier::Tension(_)));
        let mods = mods
            .iter()
            .map(|m| format!("{}", m))
            .collect::<Vec<_>>()
            .join("");
        let tensions = if tensions.is_empty() {
            String::new()
        } else {
            format!(
                "({})",
                tensions
                    .iter()
                    .map(|m| format!("{}", m))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        };
        let on = self
            .on
            .as_ref()
            .map(|p| format!("/{}", p))
            .unwrap_or("".to_string());
        // `@N` を再出力する。これでキー移調がテキストを書き戻しても消えない
        let root_string = self
            .root_string
            .map(|n| format!("@{}", n))
            .unwrap_or_default();
        write!(f, "{}{}{}{}{}", self.key, mods, tensions, on, root_string)
    }
}

#[cfg(test)]
mod tests {
    use crate::import::{Importer, RechordImporter};

    #[test]
    fn test_arpeggio_roundtrip() {
        let ast = RechordImporter.import("arp(C) |\n").unwrap();
        let s = format!("{}", ast);
        assert!(s.contains("arp(C)"));
    }

    fn roundtrip(code: &str) -> String {
        let ast = RechordImporter.import(&format!("{}\n", code)).unwrap();
        format!("{}", ast)
            .lines()
            .map(|l| l.trim_end())
            .collect::<Vec<_>>()
            .join("\n")
            .trim_end()
            .to_string()
    }

    #[test]
    fn test_pow_roundtrip() {
        assert_eq!(roundtrip("pow(C | E D)"), "pow(C | E D)");
        assert_eq!(roundtrip("pow(C)"), "pow(C)");
    }

    #[test]
    fn test_oct_roundtrip() {
        assert_eq!(roundtrip("oct(A@6 | D@5)"), "oct(A@6 | D@5)");
        assert_eq!(roundtrip("oct(A)"), "oct(A)");
        assert_eq!(
            roundtrip("pat(\"oxox\", oct(A@6))"),
            "pat(\"oxox\", oct(A@6))"
        );
    }

    #[test]
    fn test_pat_roundtrip() {
        assert_eq!(
            roundtrip("pat(\"o.o.oxox\", C | E D)"),
            "pat(\"o.o.oxox\", C | E D)"
        );
    }

    #[test]
    fn test_nested_apply_roundtrip() {
        assert_eq!(roundtrip("pat(\"oxox\", pow(C))"), "pat(\"oxox\", pow(C))");
    }

    #[test]
    fn test_apply_keeps_line_breaks_roundtrip() {
        assert_eq!(
            roundtrip("pow(C)\npat(\"oo\", D)"),
            "pow(C)\npat(\"oo\", D)"
        );
    }

    #[test]
    fn test_apply_then_measure_roundtrip() {
        assert_eq!(roundtrip("pow(C) F |"), "pow(C) F |");
    }

    #[test]
    fn test_root_string_roundtrip() {
        assert_eq!(roundtrip("A@6 |"), "A@6 |");
        assert_eq!(roundtrip("A/E@6 |"), "A/E@6 |");
        assert_eq!(roundtrip("pow(A@6 | D@5)"), "pow(A@6 | D@5)");
    }

    #[test]
    fn test_key_directive_roundtrip() {
        assert_eq!(roundtrip("key: C\nC | Am |"), "key: C\nC | Am |");
    }

    #[test]
    fn test_scale_directive_roundtrip() {
        assert_eq!(
            roundtrip("key: C\nscale: dorian\nI | IV |"),
            "key: C\nscale: dorian\nI | IV |"
        );
    }

    #[test]
    fn test_scale_only_directive_roundtrip() {
        assert_eq!(
            roundtrip("scale: dorian\nC | Am |"),
            "scale: dorian\nC | Am |"
        );
    }

    #[test]
    fn test_comment_and_key_roundtrip() {
        assert_eq!(roundtrip("# memo\nkey: C\nC |"), "# memo\nkey: C\nC |");
    }

    #[test]
    fn test_key_then_comment_roundtrip() {
        assert_eq!(roundtrip("key: C\n# memo\nC |"), "key: C\n# memo\nC |");
    }

    #[test]
    fn test_comment_mid_song_roundtrip() {
        assert_eq!(
            roundtrip("key: C\nC |\n# memo\nAm |"),
            "key: C\nC |\n# memo\nAm |"
        );
    }

    #[test]
    fn test_mid_song_key_change_roundtrip() {
        assert_eq!(
            roundtrip("key: C\nI |\nkey: D\nI |"),
            "key: C\nI |\nkey: D\nI |"
        );
    }

    #[test]
    fn test_key_with_apply_roundtrip() {
        assert_eq!(roundtrip("key: C\npow(C | E D)"), "key: C\npow(C | E D)");
        assert_eq!(
            roundtrip("key: C\npat(\"oxox\", C)"),
            "key: C\npat(\"oxox\", C)"
        );
    }

    #[test]
    fn test_blank_line_roundtrip() {
        assert_eq!(roundtrip("key: C\nC |\n\nAm |"), "key: C\nC |\nAm |");
    }

    #[test]
    fn test_blank_line_after_directive_roundtrip() {
        assert_eq!(roundtrip("key: C\n\nC | Am |"), "key: C\nC | Am |");
        assert_eq!(
            roundtrip("key: C\n\n$a = C |\n$a"),
            "key: C\n$a = C |\n$a |"
        );
    }

    #[test]
    fn test_define_roundtrip() {
        assert_eq!(
            roundtrip("$riff = pow(C | E D)\n$riff | C |"),
            "$riff = pow(C | E D)\n$riff | C |"
        );
    }

    #[test]
    fn test_pattern_define_roundtrip() {
        assert_eq!(
            roundtrip("$eight = \"o.o.o.o.\"\npat($eight, C)"),
            "$eight = \"o.o.o.o.\"\npat($eight, C)"
        );
    }

    #[test]
    fn test_repeat_on_measure_roundtrip() {
        // 小節1つの繰り返しには括弧を付けない
        assert_eq!(roundtrip("C x4 |"), "C x4 |");
    }

    #[test]
    fn test_repeat_on_group_roundtrip() {
        // 2小節以上の繰り返しには括弧を付ける
        assert_eq!(roundtrip("(C | Am) x3 |"), "(C | Am) x3 |");
    }

    #[test]
    fn test_repeat_on_ref_roundtrip() {
        // 定義の本体の小節は `|` で閉じる。`$a = C` と `$a = C |` は同じ
        // AST になるので、Display はどちらも `|` を付けた形に寄せる
        assert_eq!(roundtrip("$a = C |\n$a x2"), "$a = C |\n$a x2 |");
        assert_eq!(roundtrip("$a = C\n$a x2"), "$a = C |\n$a x2 |");
    }

    #[test]
    fn test_define_trailing_comment_roundtrip() {
        // 行末コメントは行末のまま。`Define` の `br` が「行がここで終わるか」を
        // 持つので、小節行 (`C | Am | # memo`) と同じ扱いになる
        assert_eq!(
            roundtrip("$a = C |  # riff\n$a"),
            "$a = C | # riff\n$a |"
        );
        assert_eq!(
            roundtrip("$eight = \"o.o.o.o.\"  # 8分の刻み\npat($eight, C)"),
            "$eight = \"o.o.o.o.\" # 8分の刻み\npat($eight, C)"
        );
    }

    #[test]
    fn test_comment_after_define_stays_on_its_own_line() {
        // 独立した行のコメントは行を移らない。行末コメントと取り違えない
        assert_eq!(
            roundtrip("$a = C |\n# riff\n$a"),
            "$a = C |\n# riff\n$a |"
        );
    }

    #[test]
    fn test_japanese_define_roundtrip() {
        assert_eq!(
            roundtrip("$Aメロ = C | Am |\n$Aメロ x2"),
            "$Aメロ = C | Am |\n$Aメロ x2 |"
        );
    }

    #[test]
    fn test_define_under_key_roundtrip() {
        assert_eq!(
            roundtrip("key: C\n$a = I | IV |\n$a x2"),
            "key: C\n$a = I | IV |\n$a x2 |"
        );
    }

    #[test]
    fn test_ref_in_pat_has_no_extra_space() {
        assert_eq!(
            roundtrip("$eight = \"o.o.o.o.\"\npat($eight, C | Am)"),
            "$eight = \"o.o.o.o.\"\npat($eight, C | Am)"
        );
    }

    #[test]
    fn test_ref_keeps_bar_separator() {
        assert_eq!(roundtrip("$a = C |\n$a | D |"), "$a = C |\n$a | D |");
    }

    #[test]
    fn test_song_structure_keeps_line_breaks() {
        // 曲構成の行が潰れないこと。Phase 2 の中心的な用途。
        // 小節の位置に置かれた参照は行末でも `|` で閉じる (`$B` -> `$B |`)。
        // 小節が `C |` で閉じるのと同じ扱い。`$A x2` だけが `|` を取らない
        assert_eq!(
            roundtrip("$A = C | Am |\n$B = F | G |\n$A x2\n$B\n$A"),
            "$A = C | Am |\n$B = F | G |\n$A x2 |\n$B |\n$A |"
        );
    }

    #[test]
    fn test_ref_keeps_line_break() {
        assert_eq!(roundtrip("$a = C |\n$a\nD |"), "$a = C |\n$a |\nD |");
    }

    #[test]
    fn test_ref_without_line_break_stays_inline() {
        assert_eq!(roundtrip("$a = C |\n$a | D |"), "$a = C |\n$a | D |");
    }

    #[test]
    fn test_repeat_keeps_bar_separator() {
        // `xN` の後ろの `|` が書き戻しで落ちないこと。小節の位置にある
        // リピートは `C x2 |` と同じく `|` で閉じる
        assert_eq!(roundtrip("$a = C |\n$a x2 | D |"), "$a = C |\n$a x2 | D |");
        assert_eq!(roundtrip("(C | Am) x2 | D |"), "(C | Am) x2 | D |");
    }

    #[test]
    fn test_repeat_keeps_line_break() {
        assert_eq!(roundtrip("(C | Am) x3\nD |"), "(C | Am) x3 |\nD |");
    }

    #[test]
    fn test_repeat_without_line_break_stays_inline() {
        assert_eq!(roundtrip("C x2 | D |"), "C x2 | D |");
    }

    #[test]
    fn test_nested_repeat_roundtrip() {
        assert_eq!(roundtrip("((C | Am) x2) x3"), "((C | Am) x2) x3 |");
    }

    /// 書き戻した結果をもう一度書き戻しても変わらないこと。
    /// これが崩れると、web の移調ボタンを押すたびにテキストが劣化する
    #[test]
    fn test_roundtrip_is_idempotent() {
        for code in [
            "$riff = pow(C | E D)\n$riff | C |",
            "$eight = \"o.o.o.o.\"\npat($eight, C)",
            "C x4 |",
            "(C | Am) x3",
            "$a = C\n$a x2",
            "$a = C |\n$a | D |",
            "$Aメロ = C | Am |\n$Aメロ x2",
            "key: C\n$a = I | IV |\n$a x2",
            "(C) x2",
            "((C | Am) x2) x3",
            "pow(C) x2\nAm |",
            "$a = C |\n$a x2\nD |",
            "# memo\nkey: C\n$a = C | Am |\n$a x2 D |",
            // 行末フラグが入って揺れやすくなった形
            "$A = C | Am |\n$B = F | G |\n$A x2\n$B\n$A",
            "$a = C |\n$a\nD |",
            "$a = C |\n$a |\nD |",
            "(C | Am) x3\nD |",
            "(C) x2\nD |",
            "$a = C |\n$a x2\n$a\nD |",
            // 定義の行末コメントと、独立した行のコメントが混ざらないこと
            "$a = C |  # riff\n$a",
            "$a = C |\n# riff\n$a",
            "$eight = \"o.o.o.o.\"  # 8分の刻み\npat($eight, C)",
            "$a = C |  # riff\n$b = D |  # other\n$a | $b |",
            // `xN` の後ろの `|`
            "$a = C |\n$a x2 | D |",
            "(C | Am) x2 | D |",
            "((C | Am) x2) x3 | D |",
        ] {
            let once = roundtrip(code);
            assert_eq!(roundtrip(&once), once, "冪等でない: {:?}", code);
        }
    }
}
