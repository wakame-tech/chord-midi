use super::{Exporter, RechordExporter};
use crate::model::ast::{Expr, Beat, ChordNode, Func, Node, Pattern};
use crate::model::modifier::Modifier;
use std::{fmt::Display, io::Write};

impl Exporter for RechordExporter {
    fn export(&self, f: &mut impl Write, ast: Expr) -> anyhow::Result<()> {
        writeln!(f, "{}", ast)?;
        Ok(())
    }
}

impl Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Expr::Seq(nodes) => {
                for node in nodes {
                    write!(f, "{}", node)?;
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
            Expr::Apply(func, inner, br) => {
                let inner = inner.to_string();
                // 小節の Display は末尾に " | " を出すので、関数の中では
                // 最後の区切りを落として `pow(C | E D)` の形に揃える
                let inner = inner.trim_end();
                let inner = inner.strip_suffix('|').unwrap_or(inner).trim_end();
                match func {
                    Func::Pow => write!(f, "pow({})", inner)?,
                    Func::Oct => write!(f, "oct({})", inner)?,
                    Func::Pat(pattern) => write!(f, "pat(\"{}\", {})", pattern, inner)?,
                }
                if *br {
                    writeln!(f)
                } else {
                    write!(f, " ")
                }
            }
            Expr::Measure(nodes, br) => {
                write!(
                    f,
                    "{}",
                    nodes
                        .iter()
                        .map(|n| n.to_string())
                        .collect::<Vec<_>>()
                        .join(" ")
                )?;
                write!(f, " | ")?;
                if *br {
                    writeln!(f)?;
                }
                Ok(())
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
        assert_eq!(roundtrip("pat(\"oxox\", oct(A@6))"), "pat(\"oxox\", oct(A@6))");
    }

    #[test]
    fn test_pat_roundtrip() {
        assert_eq!(roundtrip("pat(\"o.o.oxox\", C | E D)"), "pat(\"o.o.oxox\", C | E D)");
    }

    #[test]
    fn test_nested_apply_roundtrip() {
        assert_eq!(roundtrip("pat(\"oxox\", pow(C))"), "pat(\"oxox\", pow(C))");
    }

    #[test]
    fn test_apply_keeps_line_breaks_roundtrip() {
        assert_eq!(roundtrip("pow(C)\npat(\"oo\", D)"), "pow(C)\npat(\"oo\", D)");
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
        assert_eq!(roundtrip("scale: dorian\nC | Am |"), "scale: dorian\nC | Am |");
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
        assert_eq!(
            roundtrip("key: C\npow(C | E D)"),
            "key: C\npow(C | E D)"
        );
        assert_eq!(
            roundtrip("key: C\npat(\"oxox\", C)"),
            "key: C\npat(\"oxox\", C)"
        );
    }

    #[test]
    fn test_blank_line_roundtrip() {
        assert_eq!(roundtrip("key: C\nC |\n\nAm |"), "key: C\nC |\nAm |");
    }
}
