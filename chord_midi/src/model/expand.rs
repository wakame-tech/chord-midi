use crate::model::ast::{Expr, Func};
use anyhow::{anyhow, Result};
use std::collections::HashMap;

/// 展開してよい小節の総数。120bpm の 4/4 で 4096 小節 ≒ 2時間15分。
/// 実用の曲がこれを超えることはまず無く、超えたら書き間違いと見てよい。
///
/// `$b = $a x32` のような入れ子は乗算的に増えるので、上限が無いと
/// 打ち間違い1つでメモリを食い尽くす。web は打鍵のたびに展開するので致命的
const MAX_MEASURES: usize = 4096;

/// 展開済みの定義。本体と、その本体が持つ小節の数
struct Def {
    body: Expr,
    measures: usize,
}

/// 定義の環境。`$name` から展開済みの本体への写像
#[derive(Default)]
struct Env {
    defs: HashMap<String, Def>,
    /// いま本体を展開している最中の定義の名前。自己参照を見分けるためだけに持つ
    defining: Option<String>,
    /// これまでに作った小節の総数。`MAX_MEASURES` を超えたら止める
    measures: usize,
}

impl Env {
    /// これから作る小節を先に計上する。作ってから数えるのでは、
    /// 数え終わる前にメモリを食い尽くすので、必ず複製の前に呼ぶこと
    fn charge(&mut self, n: usize) -> Result<()> {
        self.measures = self.measures.saturating_add(n);
        if self.measures > MAX_MEASURES {
            return Err(anyhow!(
                "展開すると小節が多すぎる (上限 {} 小節)。xN の入れ子を減らすこと",
                MAX_MEASURES
            ));
        }
        Ok(())
    }
}

/// 展開済みの式が持つ小節の数
fn measures_in(expr: &Expr) -> usize {
    match expr {
        Expr::Measure(_, _) => 1,
        Expr::Seq(items) => items.iter().map(measures_in).sum(),
        Expr::Apply(_, inner, _) => measures_in(inner),
        Expr::Keyed(_, inner) | Expr::Scaled(_, inner) => measures_in(inner),
        // 展開の跡。小節を増やしも減らしもしない
        Expr::Section(_, inner) => measures_in(inner),
        Expr::Comment(_) | Expr::Pattern(_) => 0,
        // 展開後には残らない
        Expr::Define(_, _, _) | Expr::Ref(_, _) | Expr::Repeat(_, _, _) => 0,
    }
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
                    Expr::Define(name, body, _) => define(name, *body, env)?,
                    other => push_flat(&mut out, expand_in(other, env)?),
                }
            }
            Ok(Expr::Seq(out))
        }
        Expr::Ref(name, _) => {
            let Some(measures) = env.defs.get(&name).map(|def| def.measures) else {
                return Err(if env.defining.as_deref() == Some(name.as_str()) {
                    anyhow!("${} の定義が自分自身を参照している (循環参照)", name)
                } else {
                    anyhow!("${} は定義されていない (定義は使う前に書くこと)", name)
                });
            };
            // 複製する前に、複製で増える小節を計上する
            env.charge(measures)?;
            Ok(section(&name, env.defs[&name].body.clone()))
        }
        Expr::Repeat(inner, n, _) => {
            let inner = expand_in(*inner, env)?;
            // 中身は既に1つぶん計上されている。増えるのは残りの n-1 個
            let per = measures_in(&inner);
            env.charge(per * (n as usize).saturating_sub(1))?;
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
                Expr::Ref(name, _) => Some(format!("${}", name)),
                _ => None,
            };
            // 第1引数は小節ではなく値なので、由来の名前を持ち回っても意味が無い。
            // `Section` を被せたままだと `as_pattern` が中身を見られなくなる
            let arg = strip_sections(expand_in(*arg, env)?);
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
        Expr::Apply(func, inner, br) => {
            Ok(Expr::Apply(func, Box::new(expand_in(*inner, env)?), br))
        }
        Expr::Keyed(p, inner) => Ok(Expr::Keyed(p, Box::new(expand_in(*inner, env)?))),
        Expr::Scaled(s, inner) => Ok(Expr::Scaled(s, Box::new(expand_in(*inner, env)?))),
        // 定義は `Seq` の要素としてしか書けない。`(...)` や `xN` の中には置けない
        Expr::Define(name, _, _) => Err(anyhow!(
            "${} の定義が式の位置にある。定義は行の先頭に書くこと",
            name
        )),
        // 既に展開済みの跡。`resolve` を2度通しても中身を数え直すだけで、
        // `Section` が二重に被ることはない
        Expr::Section(name, inner) => Ok(Expr::Section(name, Box::new(expand_in(*inner, env)?))),
        Expr::Comment(c) => Ok(Expr::Comment(c)),
        Expr::Measure(nodes, br) => {
            env.charge(1)?;
            Ok(Expr::Measure(nodes, br))
        }
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
    let body = body?;
    let measures = measures_in(&body);
    env.defs.insert(name, Def { body, measures });
    Ok(())
}

/// 展開した本体に「`$name` から来た」という跡を付ける。
///
/// 本体が小節列なら、包みを1つ被せるのではなく要素ごとに包む。
/// まとめて包むと `push_flat` が畳めない `Seq` が残り、
/// 「小節が平らに並んだ `Seq`」という形が崩れてしまう
fn section(name: &str, body: Expr) -> Expr {
    match body {
        Expr::Seq(items) => Expr::Seq(items.into_iter().map(|it| section(name, it)).collect()),
        other => Expr::Section(name.to_string(), Box::new(other)),
    }
}

/// `Section` の包みを剥がして中身を取り出す。入れ子の定義を辿ると
/// 何重にも被ることがあるので、無くなるまで剥がす
fn strip_sections(mut expr: Expr) -> Expr {
    while let Expr::Section(_, inner) = expr {
        expr = *inner;
    }
    expr
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
        Expr::Define(_, _, _) => "定義",
        Expr::Ref(_, _) => "参照",
        Expr::Repeat(_, _, _) => "リピート",
        Expr::Section(_, _) => "展開済みの参照",
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
            Expr::Define(_, _, _) | Expr::Ref(_, _) | Expr::Repeat(_, _, _) => false,
            Expr::Seq(items) => items.iter().all(is_clean),
            Expr::Apply(func, inner, _) => {
                let arg_ok = match func {
                    crate::model::ast::Func::Pat(a) => is_clean(a),
                    _ => true,
                };
                arg_ok && is_clean(inner)
            }
            Expr::Keyed(_, inner) | Expr::Scaled(_, inner) => is_clean(inner),
            Expr::Section(_, inner) => is_clean(inner),
            _ => true,
        }
    }

    fn count_measures(e: &Expr) -> usize {
        match e {
            Expr::Measure(_, _) => 1,
            Expr::Seq(items) => items.iter().map(count_measures).sum(),
            Expr::Apply(_, inner, _) => count_measures(inner),
            Expr::Keyed(_, inner) | Expr::Scaled(_, inner) => count_measures(inner),
            Expr::Section(_, inner) => count_measures(inner),
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

    /// 参照の参照でも `pat` の第1引数はリズムパターンのまま取り出せること。
    /// `Section` は値の位置では意味が無いので、何重に被っていても剥がす
    #[test]
    fn test_pattern_ref_chain_in_pat() {
        use crate::model::ast::Func;
        let e = expanded("$eight = \"o.o.o.o.\"\n$same = $eight\npat($same, C)\n");
        fn pat_arg(e: &Expr) -> Option<&Expr> {
            match e {
                Expr::Apply(Func::Pat(a), _, _) => Some(a.as_ref()),
                Expr::Seq(items) => items.iter().find_map(pat_arg),
                _ => None,
            }
        }
        let arg = pat_arg(&e).unwrap_or_else(|| panic!("no pat: {:?}", e));
        assert!(matches!(arg, Expr::Pattern(_)), "{:?}", arg);
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

    /// 参照を展開しても `Seq` は平らなまま。`Section` は小節列をまとめて
    /// 包むのではなく、小節1つずつに被せる
    #[test]
    fn test_section_does_not_nest_seq() {
        let e = expanded("$a = C | Am |\n$a x2\n");
        let Expr::Seq(items) = &e else {
            panic!("not a seq: {:?}", e)
        };
        assert_eq!(items.len(), 4, "{:?}", items);
        assert!(
            items.iter().all(|it| matches!(
                it,
                Expr::Section(name, inner)
                    if name == "a" && matches!(inner.as_ref(), Expr::Measure(_, _))
            )),
            "{:?}",
            items
        );
    }

    /// 入れ子の定義では `Section` も入れ子になる。深さは定義の入れ子の分だけで、
    /// 同じ名前が何重にも重なったりはしない
    #[test]
    fn test_nested_defines_nest_sections_once_each() {
        let e = expanded("$a = C\n$b = $a | $a |\n$b\n");
        let Expr::Seq(items) = &e else {
            panic!("not a seq: {:?}", e)
        };
        assert_eq!(items.len(), 2, "{:?}", items);
        for item in items {
            let Expr::Section(outer, inner) = item else {
                panic!("not a section: {:?}", item)
            };
            assert_eq!(outer, "b");
            let Expr::Section(inner_name, body) = inner.as_ref() else {
                panic!("not a nested section: {:?}", inner)
            };
            assert_eq!(inner_name, "a");
            assert!(matches!(body.as_ref(), Expr::Measure(_, _)), "{:?}", body);
        }
    }

    /// `resolve()` を2度通しても `Section` が二重に被らないこと。
    /// 1度目で `Ref` は消えているので、2度目の展開は中身を数え直すだけ
    #[test]
    fn test_resolve_does_not_re_wrap_sections() {
        let once = RechordImporter
            .import("$a = C\n$b = $a | $a |\n$b | F |\n")
            .unwrap()
            .resolve()
            .unwrap();
        let twice = once.clone().resolve().unwrap();
        assert_eq!(once, twice);
    }

    /// `expand` は度数を実音に変えない。`Ref` を度数のままの本体に置き換えるだけで、
    /// 解決は後から `into_pitch` が `Keyed` のスコープを降りながら行う。
    /// 度数表記のリフをキーを変えて使い回せるのはこの順序のおかげ
    #[test]
    fn test_ref_resolves_with_the_key_at_the_use_site() {
        use crate::model::key::Key;
        use crate::model::pitch::Pitch;
        use crate::model::scale::Scale;

        // 定義は key: C の下、使うのは key: D の下。
        // expand は度数を解決しないので、使う位置のスコープが効く。
        // これにより度数表記のリフをキーを変えて使い回せる
        let e = expanded("key: C\n$a = I | IV |\nkey: D\n$a\n").into_pitch(Pitch::C, Scale::Major);

        fn pitches(e: &Expr, out: &mut Vec<Pitch>) {
            match e {
                Expr::Seq(items) => items.iter().for_each(|i| pitches(i, out)),
                Expr::Keyed(_, inner) | Expr::Scaled(_, inner) => pitches(inner, out),
                Expr::Apply(_, inner, _) => pitches(inner, out),
                Expr::Section(_, inner) => pitches(inner, out),
                Expr::Measure(nodes, _) => {
                    for n in nodes {
                        if let crate::model::ast::Node::Chord(c) = n {
                            if let Key::Absolute(p) = c.key {
                                out.push(p);
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        let mut got = Vec::new();
        pitches(&e, &mut got);
        assert_eq!(got, vec![Pitch::D, Pitch::G], "使う位置の key: が効くこと");
    }

    #[test]
    fn test_expansion_is_capped() {
        // xN の入れ子で乗算的に増える。web は打鍵のたびに展開するので、
        // 上限が無いとブラウザが固まる
        let msg = err("$a = (C | D | E | F) x32\n$b = $a x32\n$c = $b x32\n$c\n");
        assert!(msg.contains("4096") || msg.contains("多すぎる"), "{}", msg);
    }

    #[test]
    fn test_reasonable_song_is_not_capped() {
        // 実用の範囲は通ること。(4小節) x32 = 128小節
        let e = expanded("(C | Am | F | G) x32\n");
        assert_eq!(count_measures(&e), 128);
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
