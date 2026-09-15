use crate::model::ast::Expr;

/// 記法として正しいが、読みにくい書き方を警告する。
/// エラーではないので、パースも再生も止めない
pub fn lint(expr: &Expr) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen_measure = false;
    walk(expr, &mut seen_measure, &mut out);
    out
}

fn walk(expr: &Expr, seen_measure: &mut bool, out: &mut Vec<String>) {
    match expr {
        Expr::Seq(items) => {
            for item in items {
                walk(item, seen_measure, out);
            }
        }
        Expr::Keyed(_, inner) | Expr::Scaled(_, inner) => walk(inner, seen_measure, out),
        Expr::Measure(_, _) => *seen_measure = true,
        Expr::Apply(_, _, _) | Expr::Repeat(_, _, _) | Expr::Ref(_, _) => *seen_measure = true,
        Expr::Define(name, _, _) => {
            if *seen_measure {
                out.push(format!(
                    "${} の定義が小節より後ろにある。素材は曲より前にまとめると読みやすい",
                    name
                ));
            }
        }
        // 網羅的に書くこと。catch-all を置かない
        Expr::Comment(_) | Expr::Pattern(_) | Expr::Section(_, _) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::lint;
    use crate::import::{Importer, RechordImporter};

    fn warnings(code: &str) -> Vec<String> {
        lint(&RechordImporter.import(code).unwrap())
    }

    #[test]
    fn test_no_warning_when_defines_come_first() {
        assert!(warnings("$a = C\n$a | D |\n").is_empty());
    }

    #[test]
    fn test_warns_when_measure_precedes_define() {
        let w = warnings("C |\n$a = D\n$a\n");
        assert_eq!(w.len(), 1);
        assert!(w[0].contains("a"), "{}", w[0]);
    }

    #[test]
    fn test_no_warning_without_defines() {
        assert!(warnings("C | Am |\n").is_empty());
    }

    #[test]
    fn test_comment_and_directive_do_not_count_as_measures() {
        assert!(warnings("# memo\nkey: C\n$a = I\n$a\n").is_empty());
    }
}
