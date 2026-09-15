use crate::model::ast::{Expr, Func};
use anyhow::{anyhow, Result};
use std::collections::HashMap;

/// 定義の環境。`$name` から展開済みの本体への写像
#[derive(Default)]
struct Env {
    defs: HashMap<String, Expr>,
    /// いま本体を展開している最中の定義の名前。自己参照を見分けるためだけに持つ
    defining: Option<String>,
}

/// `Define` を環境に収容して `Seq` から除き、`Ref` を値に置き換え、
/// `Repeat` を n 回に展開する。
///
/// 通過後の `Expr` には `Define` / `Ref` / `Repeat` が残らない。
/// 前方参照は禁止なので、`Seq` を前から順に舐めるだけで解決できる。
pub fn expand(expr: Expr) -> Result<Expr> {
    let mut env = Env::default();
    expand_in(expr, &mut env)
}

fn expand_in(expr: Expr, env: &mut Env) -> Result<Expr> {
    match expr {
        Expr::Seq(items) => {
            let mut out = Vec::new();
            for item in items {
                match item {
                    // 定義は環境に入れて、式としては 0 小節を返す
                    Expr::Define(name, body) => define(name, *body, env)?,
                    other => push_flat(&mut out, expand_in(other, env)?),
                }
            }
            Ok(Expr::Seq(out))
        }
        Expr::Ref(name) => match env.defs.get(&name) {
            Some(body) => Ok(body.clone()),
            None if env.defining.as_deref() == Some(name.as_str()) => Err(anyhow!(
                "${} の定義が自分自身を参照している (循環参照)",
                name
            )),
            None => Err(anyhow!(
                "${} は定義されていない (定義は使う前に書くこと)",
                name
            )),
        },
        Expr::Repeat(inner, n) => {
            let inner = expand_in(*inner, env)?;
            let mut out = Vec::new();
            for _ in 0..n {
                push_flat(&mut out, inner.clone());
            }
            Ok(Expr::Seq(out))
        }
        // `pat` の第1引数だけは値なので、展開したうえで型を検める
        Expr::Apply(Func::Pat(arg), inner, br) => {
            // 何を渡されたかを言えるよう、参照なら名前を控えておく
            let given = match arg.as_ref() {
                Expr::Ref(name) => Some(format!("${}", name)),
                _ => None,
            };
            let arg = expand_in(*arg, env)?;
            if !matches!(arg, Expr::Pattern(_)) {
                return Err(match given {
                    Some(name) => anyhow!(
                        "pat の第1引数はリズムパターンでなければならない: {} は{}",
                        name,
                        describe(&arg)
                    ),
                    None => anyhow!(
                        "pat の第1引数はリズムパターンでなければならない ({}が渡された)",
                        describe(&arg)
                    ),
                });
            }
            Ok(Expr::Apply(
                Func::Pat(Box::new(arg)),
                Box::new(expand_in(*inner, env)?),
                br,
            ))
        }
        Expr::Apply(func, inner, br) => Ok(Expr::Apply(func, Box::new(expand_in(*inner, env)?), br)),
        Expr::Keyed(p, inner) => Ok(Expr::Keyed(p, Box::new(expand_in(*inner, env)?))),
        Expr::Scaled(s, inner) => Ok(Expr::Scaled(s, Box::new(expand_in(*inner, env)?))),
        // 定義は `Seq` の要素としてしか書けない。`(...)` や `xN` の中には置けない
        Expr::Define(name, _) => Err(anyhow!(
            "${} の定義が式の位置にある。定義は行の先頭に書くこと",
            name
        )),
        Expr::Comment(c) => Ok(Expr::Comment(c)),
        Expr::Measure(nodes, br) => Ok(Expr::Measure(nodes, br)),
        Expr::Pattern(p) => Ok(Expr::Pattern(p)),
    }
}

/// 定義を環境に収容する。
///
/// 本体はここで、その時点の環境のまま展開してしまう。こうすると
/// 自己参照 (`$a = $a`) も前方参照 (`$b = $a` が `$a` の定義より前) も
/// 「まだ環境に無い」として弾かれるので、循環検出のスタックが要らない
fn define(name: String, body: Expr, env: &mut Env) -> Result<()> {
    if env.defs.contains_key(&name) {
        return Err(anyhow!("${} は既に定義されている", name));
    }
    let outer = env.defining.replace(name.clone());
    let body = expand_in(body, env);
    env.defining = outer;
    env.defs.insert(name, body?);
    Ok(())
}

/// 展開の結果を親のリストへ差し込む。`Ref` と `Repeat` が置いていく入れ子の
/// `Seq` をここで畳み、「小節が平らに並んだ `Seq`」という形を保つ
fn push_flat(out: &mut Vec<Expr>, expr: Expr) {
    match expr {
        Expr::Seq(items) => out.extend(items),
        other => out.push(other),
    }
}

/// エラー文に出す、式の種類の呼び名。AST をそのまま貼ると読めないので
fn describe(expr: &Expr) -> &'static str {
    match expr {
        Expr::Comment(_) => "コメント",
        Expr::Keyed(_, _) => "key: の指定",
        Expr::Scaled(_, _) => "scale: の指定",
        Expr::Measure(_, _) => "小節",
        Expr::Apply(_, _, _) => "関数の適用",
        Expr::Seq(_) => "小節列",
        Expr::Pattern(_) => "リズムパターン",
        Expr::Define(_, _) => "定義",
        Expr::Ref(_) => "参照",
        Expr::Repeat(_, _) => "リピート",
    }
}

#[cfg(test)]
mod tests {
    use super::expand;
    use crate::import::{Importer, RechordImporter};
    use crate::model::ast::Expr;

    fn expanded(code: &str) -> Expr {
        expand(RechordImporter.import(code).unwrap()).unwrap()
    }

    fn err(code: &str) -> String {
        expand(RechordImporter.import(code).unwrap())
            .unwrap_err()
            .to_string()
    }

    /// 展開後に Define / Ref / Repeat が残っていないこと
    fn is_clean(e: &Expr) -> bool {
        match e {
            Expr::Define(_, _) | Expr::Ref(_) | Expr::Repeat(_, _) => false,
            Expr::Seq(items) => items.iter().all(is_clean),
            Expr::Apply(func, inner, _) => {
                let arg_ok = match func {
                    crate::model::ast::Func::Pat(a) => is_clean(a),
                    _ => true,
                };
                arg_ok && is_clean(inner)
            }
            Expr::Keyed(_, inner) | Expr::Scaled(_, inner) => is_clean(inner),
            _ => true,
        }
    }

    fn count_measures(e: &Expr) -> usize {
        match e {
            Expr::Measure(_, _) => 1,
            Expr::Seq(items) => items.iter().map(count_measures).sum(),
            Expr::Apply(_, inner, _) => count_measures(inner),
            Expr::Keyed(_, inner) | Expr::Scaled(_, inner) => count_measures(inner),
            _ => 0,
        }
    }

    #[test]
    fn test_ref_is_replaced() {
        let e = expanded("$riff = C | E |\n$riff | D |\n");
        assert!(is_clean(&e), "{:?}", e);
        assert_eq!(count_measures(&e), 3);
    }

    #[test]
    fn test_define_contributes_no_measures() {
        let e = expanded("$riff = C | E |\n");
        assert_eq!(count_measures(&e), 0);
    }

    #[test]
    fn test_repeat_expands() {
        let e = expanded("C x4 |\n");
        assert_eq!(count_measures(&e), 4);
    }

    #[test]
    fn test_group_repeat_expands() {
        let e = expanded("(C | Am) x3\n");
        assert_eq!(count_measures(&e), 6);
    }

    #[test]
    fn test_nested_refs() {
        let e = expanded("$a = C\n$b = $a | $a\n$b x2\n");
        assert!(is_clean(&e), "{:?}", e);
        assert_eq!(count_measures(&e), 4);
    }

    #[test]
    fn test_pattern_ref_in_pat() {
        use crate::model::ast::Func;
        let e = expanded("$eight = \"o.o.o.o.\"\npat($eight, C)\n");
        assert!(is_clean(&e), "{:?}", e);
        fn pat_arg_is_pattern(e: &Expr) -> bool {
            match e {
                Expr::Apply(Func::Pat(a), _, _) => matches!(a.as_ref(), Expr::Pattern(_)),
                Expr::Seq(items) => items.iter().any(pat_arg_is_pattern),
                _ => false,
            }
        }
        assert!(pat_arg_is_pattern(&e), "{:?}", e);
    }

    #[test]
    fn test_undefined_ref_errors() {
        assert!(err("$nope | C |\n").contains("nope"));
    }

    #[test]
    fn test_cyclic_ref_errors() {
        let msg = err("$a = $a\n$a\n");
        assert!(msg.contains("a"), "{}", msg);
    }

    #[test]
    fn test_forward_reference_errors() {
        let msg = err("$b = $a\n$a = C\n$b\n");
        assert!(msg.contains("a"), "{}", msg);
    }

    #[test]
    fn test_type_mismatch_errors() {
        let msg = err("$riff = C | E |\npat($riff, C)\n");
        assert!(
            msg.contains("riff") || msg.contains("パターン") || msg.contains("pattern"),
            "{}",
            msg
        );
    }

    #[test]
    fn test_redefine_errors() {
        let msg = err("$a = C\n$a = D\n$a\n");
        assert!(msg.contains("a"), "{}", msg);
    }

    /// グループや参照を展開した跡に入れ子の `Seq` を残さないこと。
    /// `(C | Am) x3` は 6 小節が平らに並んだ `Seq` になる
    #[test]
    fn test_expansion_does_not_nest_seq() {
        let e = expanded("(C | Am) x3\n");
        let Expr::Seq(items) = &e else {
            panic!("not a seq: {:?}", e)
        };
        assert_eq!(items.len(), 6, "{:?}", items);
        assert!(
            items.iter().all(|it| matches!(it, Expr::Measure(_, _))),
            "{:?}",
            items
        );
    }

    /// `Apply` の中身は常に `Seq`。展開で畳んでもその形は崩さない
    #[test]
    fn test_apply_body_stays_seq() {
        use crate::model::ast::Func;
        let e = expanded("$a = C | Am |\npow($a x2)\n");
        let Expr::Seq(items) = &e else {
            panic!("not a seq: {:?}", e)
        };
        let Some(Expr::Apply(Func::Pow, inner, _)) = items.first() else {
            panic!("not an apply: {:?}", items)
        };
        let Expr::Seq(inner) = inner.as_ref() else {
            panic!("apply body is not a seq: {:?}", inner)
        };
        assert_eq!(inner.len(), 4, "{:?}", inner);
    }
}
