use crate::import::{Importer, RechordImporter};
use crate::model::ast::{ChordNode, Expr, Node};
use crate::model::key::Key;
use crate::model::pitch::Pitch;
use crate::model::scale::Scale;
use std::str::FromStr;

fn shift_pitch(p: Pitch, delta: i8) -> Pitch {
    let n = (p as i8 + delta).rem_euclid(12) as u8;
    Pitch::try_from(n).unwrap()
}

/// 進行全体を `delta` 半音だけ移調する。
///
/// 実音表記のコード (`C | Am |`) はコードそのものをずらし、度数表記
/// (`I | VIm |`) は `key:` ディレクティブをずらすことで追従させる。両方が
/// 混ざっていてもそれぞれ正しく動く。
///
/// パースできないテキストのときは `key:` の行だけを書き換える従来の
/// 挙動に落とす。入力途中でもボタンが死なないようにするため。
pub fn shift_key(text: &str, delta: i8) -> String {
    transpose(text, delta).unwrap_or_else(|| shift_key_directive(text, delta))
}

fn transpose(text: &str, delta: i8) -> Option<String> {
    let mut ast = RechordImporter.import(&format!("{}\n", text)).ok()?;
    let mut seen = Seen::default();
    shift_ast(&mut ast, delta, &mut seen);

    // Display は小節の区切りに " | " を出すので行末に空白が残る。落としておく
    let body = format!("{}", ast)
        .lines()
        .map(|line| line.trim_end())
        .collect::<Vec<_>>()
        .join("\n");

    // 度数表記があるのに key: が無いときだけ補う。実音表記だけのときは
    // コード自体がずれているので key: を足す必要がない
    if seen.relative && !seen.key_directive {
        Some(format!("key: {}\n{}", shift_pitch(Pitch::C, delta), body))
    } else {
        Some(body)
    }
}

#[derive(Default)]
struct Seen {
    relative: bool,
    key_directive: bool,
}

fn shift_ast(ast: &mut Expr, delta: i8, seen: &mut Seen) {
    match ast {
        Expr::Seq(items) => {
            for item in items {
                shift_ast(item, delta, seen);
            }
        }
        Expr::Keyed(p, inner) => {
            seen.key_directive = true;
            *p = shift_pitch(*p, delta);
            shift_ast(inner, delta, seen);
        }
        Expr::Scaled(_, inner) => shift_ast(inner, delta, seen),
        Expr::Measure(nodes, _) => {
            for node in nodes {
                shift_node(node, delta, seen);
            }
        }
        Expr::Apply(_, inner, _) => shift_ast(inner, delta, seen),
        // 定義の本体にもコードが入るので潜る。参照は名前だけなので触らない
        Expr::Define(_, body, _) => shift_ast(body, delta, seen),
        // リピートの中身にもコードが入るので潜る。回数は移調で変わらない
        Expr::Repeat(inner, _, _) => shift_ast(inner, delta, seen),
        // 展開の跡。移調は編集中の AST に掛かるので普通は現れないが、
        // 現れたら中身のコードを動かす
        Expr::Section(_, inner) => shift_ast(inner, delta, seen),
        Expr::Ref(_, _) => {}
        Expr::Comment(_) => {}
        // パターン文字列は移調で変わらない。`Func::Pat` の引数としてしか
        // 現れず、ここでは Apply の中身 (`inner`) を辿るだけなので実際には
        // 素通りしない経路だが、網羅性のため明示しておく
        Expr::Pattern(_) => {}
    }
}

fn shift_node(node: &mut Node, delta: i8, seen: &mut Seen) {
    match node {
        Node::Chord(c) | Node::Arpeggio(c) | Node::Transform(_, c) | Node::Brush(c) => {
            shift_chord(c, delta, seen)
        }
        Node::Rest | Node::Sustain | Node::Repeat => {}
    }
}

fn shift_chord(chord: &mut ChordNode, delta: i8, seen: &mut Seen) {
    shift_chord_key(&mut chord.key, delta, seen);
    if let Some(on) = chord.on.as_mut() {
        shift_chord_key(on, delta, seen);
    }
}

fn shift_chord_key(key: &mut Key, delta: i8, seen: &mut Seen) {
    match key {
        Key::Absolute(p) => *p = shift_pitch(*p, delta),
        // 度数表記は key: に追従するのでここでは触らない
        Key::Relative(_) => seen.relative = true,
    }
}

/// パースできないときのフォールバック。`key:` の行だけを書き換える
fn shift_key_directive(text: &str, delta: i8) -> String {
    let mut lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
    if let Some(line) = lines
        .iter_mut()
        .find(|l| l.trim_start().starts_with("key:"))
    {
        if let Some((_, rest)) = line.split_once("key:") {
            if let Ok(p) = Pitch::from_str(rest.trim()) {
                *line = format!("key: {}", shift_pitch(p, delta));
            }
        }
        lines.join("\n")
    } else {
        let mut out = vec![format!("key: {}", shift_pitch(Pitch::C, delta))];
        out.extend(lines);
        out.join("\n")
    }
}

/// Set (or insert) the `scale:` directive. If absent, insert right after the
/// `key:` line (or at the top if there is no key line).
pub fn set_scale(text: &str, scale: Scale) -> String {
    let mut lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
    if let Some(line) = lines
        .iter_mut()
        .find(|l| l.trim_start().starts_with("scale:"))
    {
        *line = format!("scale: {}", scale);
        lines.join("\n")
    } else {
        let insert_at = lines
            .iter()
            .position(|l| l.trim_start().starts_with("key:"))
            .map(|i| i + 1)
            .unwrap_or(0);
        lines.insert(insert_at, format!("scale: {}", scale));
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shift_key_existing() {
        assert_eq!(shift_key("key: C\nI |", 1), "key: C#\nI |");
        assert_eq!(shift_key("key: C\nI |", -1), "key: B\nI |");
        assert_eq!(shift_key("key: B\nI |", 1), "key: C\nI |");
    }

    #[test]
    fn test_shift_key_transposes_inside_oct() {
        assert_eq!(shift_key("oct(A@6)", 1), "oct(A#@6)");
        assert_eq!(shift_key("oct(A#@6)", -1), "oct(A@6)");
    }

    #[test]
    fn test_shift_key_absent_inserts() {
        assert_eq!(shift_key("I | IV |", 2), "key: D\nI | IV |");
    }

    #[test]
    fn test_shift_key_transposes_absolute_chords() {
        // 実音表記はコードそのものがずれる。key: は足さない
        assert_eq!(shift_key("C | Am |", 1), "C# | A#m |");
        assert_eq!(shift_key("C | Am |", -1), "B | G#m |");
    }

    #[test]
    fn test_shift_key_transposes_slash_chords() {
        assert_eq!(shift_key("C/E |", 1), "C#/F |");
    }

    #[test]
    fn test_shift_key_handles_mixed_notation() {
        // 度数は key: に追従し、実音はコード自体がずれる
        assert_eq!(shift_key("key: C\nI | F |", 1), "key: C#\nI | F# |");
    }

    #[test]
    fn test_shift_key_keeps_multiple_lines() {
        assert_eq!(shift_key("C | Am |\nF | G |", 1), "C# | A#m |\nF# | G# |");
    }

    #[test]
    fn test_shift_key_keeps_comments_and_directives() {
        // 何度押しても書式が崩れないこと
        let once = shift_key("# memo\nkey: C\nscale: dorian\nC |", 1);
        assert_eq!(once, "# memo\nkey: C#\nscale: dorian\nC# |");
        assert_eq!(shift_key(&once, -1), "# memo\nkey: C\nscale: dorian\nC |");
    }

    #[test]
    fn test_shift_key_falls_back_on_unparsable_text() {
        // パースできないテキストでもボタンが死なないこと
        assert_eq!(shift_key("key: C\n???", 1), "key: C#\n???");
    }

    #[test]
    fn test_set_scale_existing() {
        assert_eq!(
            set_scale("key: C\nscale: major\nI |", Scale::Dorian),
            "key: C\nscale: dorian\nI |"
        );
    }

    #[test]
    fn test_set_scale_absent_after_key() {
        assert_eq!(
            set_scale("key: C\nI |", Scale::Lydian),
            "key: C\nscale: lydian\nI |"
        );
    }

    #[test]
    fn test_shift_key_transposes_inside_pow() {
        assert_eq!(shift_key("pow(C | E D)", 1), "pow(C# | F D#)");
        assert_eq!(shift_key("pow(C# | F D#)", -1), "pow(C | E D)");
    }

    #[test]
    fn test_shift_key_keeps_function_line_breaks() {
        // 関数を別々の行に書いても行が潰れないこと
        assert_eq!(
            shift_key("pow(C)\npat(\"oo\", D)", 1),
            "pow(C#)\npat(\"oo\", D#)"
        );
    }

    #[test]
    fn test_shift_key_keeps_pattern_literal() {
        // パターン文字列は移調で変わらない
        assert_eq!(shift_key("pat(\"o.o.\", C)", 1), "pat(\"o.o.\", C#)");
    }

    #[test]
    fn test_shift_key_keeps_root_string() {
        assert_eq!(shift_key("pow(A@6)", 1), "pow(A#@6)");
        assert_eq!(shift_key("pow(A#@6)", -1), "pow(A@6)");
    }

    #[test]
    fn test_shift_key_transposes_every_key_directive() {
        // 途中転調も両方ずれる
        assert_eq!(
            shift_key("key: C\nI |\nkey: D\nI |", 1),
            "key: C#\nI |\nkey: D#\nI |"
        );
    }

    #[test]
    fn test_shift_key_scale_directive_survives() {
        assert_eq!(
            shift_key("key: C\nscale: dorian\nI |", 1),
            "key: C#\nscale: dorian\nI |"
        );
    }

    #[test]
    fn test_shift_key_transposes_inside_pow_under_key() {
        // Keyed -> Apply -> Measure の2段階の潜り込みでも移調される
        assert_eq!(
            shift_key("key: C\npow(C | E D)", 1),
            "key: C#\npow(C# | F D#)"
        );
        assert_eq!(
            shift_key("key: C#\npow(C# | F D#)", -1),
            "key: C\npow(C | E D)"
        );
    }

    #[test]
    fn test_shift_key_round_trips_with_mid_song_key_change_and_apply() {
        // 途中転調 + Apply を含む形でも往復で元に戻ること
        let original = "key: C\npow(C | E D)\nkey: D\nI |";
        let shifted = shift_key(original, 1);
        assert_eq!(shift_key(&shifted, -1), original);
    }

    #[test]
    fn test_shift_key_transposes_inside_define() {
        assert_eq!(
            shift_key("$riff = C | E |\n$riff | D |", 1),
            "$riff = C# | F |\n$riff | D# |"
        );
    }

    #[test]
    fn test_shift_key_keeps_pattern_define() {
        // リズムの定義は移調の対象外
        assert_eq!(
            shift_key("$eight = \"o.o.o.o.\"\npat($eight, C)", 1),
            "$eight = \"o.o.o.o.\"\npat($eight, C#)"
        );
    }

    #[test]
    fn test_shift_key_transposes_inside_repeat() {
        assert_eq!(shift_key("C x4 |", 1), "C# x4 |");
        assert_eq!(shift_key("(C | Am) x2", 1), "(C# | A#m) x2");
    }

    #[test]
    fn test_shift_key_round_trip_with_defines() {
        let src = "key: C\n$a = I | IV |\n$a x2";
        assert_eq!(shift_key(&shift_key(src, 1), -1), src);
    }

    #[test]
    fn test_shift_key_keeps_song_structure() {
        // 移調ボタンを押しても曲構成の行が潰れないこと。
        // 小節の位置の参照は行末でも `|` で閉じるので、src も正規化済みの
        // 形 (`$B |`) にしてある。これで往復が完全に元へ戻る
        let src = "$A = C | Am |\n$B = F | G |\n$A x2\n$B |\n$A |";
        assert_eq!(
            shift_key(src, 1),
            "$A = C# | A#m |\n$B = F# | G# |\n$A x2\n$B |\n$A |"
        );
        // 往復して元に戻る
        assert_eq!(shift_key(&shift_key(src, 1), -1), src);
    }

    #[test]
    fn test_shift_ast_descends_into_define_and_repeat() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::{Expr, Node};
        use crate::model::key::Key;
        use crate::model::pitch::Pitch;

        let mut ast = RechordImporter
            .import("$riff = C | E |\n$riff x2 | D |\n")
            .unwrap();
        let mut seen = Seen::default();
        shift_ast(&mut ast, 1, &mut seen);

        fn pitches(e: &Expr, out: &mut Vec<Pitch>) {
            match e {
                Expr::Seq(items) => items.iter().for_each(|i| pitches(i, out)),
                Expr::Define(_, body, _) => pitches(body, out),
                Expr::Repeat(inner, _, _) => pitches(inner, out),
                Expr::Section(_, inner) => pitches(inner, out),
                Expr::Apply(_, inner, _) => pitches(inner, out),
                Expr::Keyed(_, inner) | Expr::Scaled(_, inner) => pitches(inner, out),
                Expr::Measure(nodes, _) => {
                    for n in nodes {
                        if let Node::Chord(c) = n {
                            if let Key::Absolute(p) = c.key {
                                out.push(p);
                            }
                        }
                    }
                }
                Expr::Comment(_) | Expr::Pattern(_) | Expr::Ref(_, _) => {}
            }
        }

        let mut got = Vec::new();
        pitches(&ast, &mut got);
        // 定義の本体 C E が C# F に、外の D が D# に
        assert_eq!(got, vec![Pitch::Cs, Pitch::F, Pitch::Ds]);
    }

    #[test]
    fn test_shift_ast_sets_relative_seen_inside_define() {
        // 定義の本体が度数表記のときも seen.relative が立つこと。
        // これが立たないと、度数だけの定義を移調したときに key: が
        // 補われなくなってしまう
        use crate::import::{Importer, RechordImporter};

        let mut ast = RechordImporter.import("$a = I | IV |\n$a\n").unwrap();
        let mut seen = Seen::default();
        shift_ast(&mut ast, 1, &mut seen);
        assert!(seen.relative);
        assert!(!seen.key_directive);
    }

    #[test]
    fn test_shift_key_inserts_key_when_only_define_is_relative() {
        // 度数表記が定義の中にしか無くても key: が補われること
        assert_eq!(
            shift_key("$a = I | IV |\n$a", 1),
            "key: C#\n$a = I | IV |\n$a |"
        );
    }

    #[test]
    fn test_shift_key_preserves_key_directive_scope_across_ref() {
        // key: のスコープは「使う位置」で効く。度数表記の定義の本体は
        // 移調で変わらず、各 key: だけがずれる
        assert_eq!(
            shift_key("key: C\n$a = I | IV |\nkey: D\n$a", 1),
            "key: C#\n$a = I | IV |\nkey: D#\n$a |"
        );
    }
}
