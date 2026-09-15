use crate::import::{Importer, RechordImporter};
use crate::model::ast::{Ast, ChordNode, Directive, Node};
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

fn shift_ast(ast: &mut Ast, delta: i8, seen: &mut Seen) {
    match ast {
        Ast::Score(items) => {
            for item in items {
                shift_ast(item, delta, seen);
            }
        }
        Ast::Directive(Directive::Key(p)) => {
            seen.key_directive = true;
            *p = shift_pitch(*p, delta);
        }
        Ast::Measure(nodes, _) => {
            for node in nodes {
                shift_node(node, delta, seen);
            }
        }
        Ast::Apply(_, items, _) => {
            for item in items {
                shift_ast(item, delta, seen);
            }
        }
        Ast::Directive(Directive::Scale(_)) | Ast::Comment(_) => {}
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
    if let Some(line) = lines.iter_mut().find(|l| l.trim_start().starts_with("key:")) {
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
    if let Some(line) = lines.iter_mut().find(|l| l.trim_start().starts_with("scale:")) {
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
        assert_eq!(
            shift_key("C | Am |\nF | G |", 1),
            "C# | A#m |\nF# | G# |"
        );
    }

    #[test]
    fn test_shift_key_keeps_comments_and_directives() {
        // 何度押しても書式が崩れないこと
        let once = shift_key("# memo\nkey: C\nscale: dorian\nC |", 1);
        assert_eq!(once, "# memo\nkey: C#\nscale: dorian\nC# |");
        assert_eq!(
            shift_key(&once, -1),
            "# memo\nkey: C\nscale: dorian\nC |"
        );
    }

    #[test]
    fn test_shift_key_falls_back_on_unparsable_text() {
        // パースできないテキストでもボタンが死なないこと
        assert_eq!(shift_key("key: C\n???", 1), "key: C#\n???");
    }

    #[test]
    fn test_set_scale_existing() {
        assert_eq!(set_scale("key: C\nscale: major\nI |", Scale::Dorian), "key: C\nscale: dorian\nI |");
    }

    #[test]
    fn test_set_scale_absent_after_key() {
        assert_eq!(set_scale("key: C\nI |", Scale::Lydian), "key: C\nscale: lydian\nI |");
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
}
