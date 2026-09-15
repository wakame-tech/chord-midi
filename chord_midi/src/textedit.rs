//! textarea のカーソル位置を起点にテキストを書き換える純粋関数。
//!
//! DOM に触らないので workspace のテストに載る。`chord_midi_web` は
//! workspace から外れていて `cargo test` が回らないため、判断のある処理は
//! ここに置き、web 側は web-sys を呼ぶだけの薄い層にする。

use once_cell::sync::Lazy;
use regex::Regex;

/// 編集の結果。位置はすべてバイトオフセット
#[derive(Debug, Clone, PartialEq)]
pub struct Edited {
    pub text: String,
    /// 編集後のテキストでのカーソル位置
    pub cursor: usize,
}

/// `start <= end` に直し、文字境界へ丸める
fn clamp(text: &str, start: usize, end: usize) -> (usize, usize) {
    let (start, end) = if start <= end {
        (start, end)
    } else {
        (end, start)
    };
    (floor_boundary(text, start), floor_boundary(text, end))
}

/// `i` を文字の先頭まで戻す。textarea 由来の位置をそのまま
/// スライスに使うと、日本語の途中で panic する
fn floor_boundary(text: &str, mut i: usize) -> usize {
    if i >= text.len() {
        return text.len();
    }
    while i > 0 && !text.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// `[start, end)` を `replacement` で置き換え、カーソルを置換後の末尾に置く
pub fn replace_range(text: &str, start: usize, end: usize, replacement: &str) -> Edited {
    let (start, end) = clamp(text, start, end);
    let mut out = String::with_capacity(text.len() + replacement.len());
    out.push_str(&text[..start]);
    out.push_str(replacement);
    out.push_str(&text[end..]);
    Edited {
        cursor: start + replacement.len(),
        text: out,
    }
}

/// `[start, end)` を `open` / `close` で包む。
/// 選択が空なら `open` + `close` を挿入し、カーソルを括弧の内側に置く
pub fn wrap_range(text: &str, start: usize, end: usize, open: &str, close: &str) -> Edited {
    let (start, end) = clamp(text, start, end);
    let selected = &text[start..end];
    let mut out = String::with_capacity(text.len() + open.len() + close.len());
    out.push_str(&text[..start]);
    out.push_str(open);
    out.push_str(selected);
    out.push_str(close);
    out.push_str(&text[end..]);
    // 選択があれば包んだ後ろへ、無ければ括弧の内側へ
    let cursor = if selected.is_empty() {
        start + open.len()
    } else {
        start + open.len() + selected.len() + close.len()
    };
    Edited { text: out, cursor }
}

/// UTF-16 コード単位のオフセット (textarea の `selectionStart`) をバイト位置に直す
pub fn utf16_to_byte(text: &str, offset: u32) -> usize {
    let mut units = 0u32;
    for (i, c) in text.char_indices() {
        if units >= offset {
            return i;
        }
        units += c.len_utf16() as u32;
    }
    text.len()
}

/// バイト位置を UTF-16 コード単位のオフセットに直す
pub fn byte_to_utf16(text: &str, offset: usize) -> u32 {
    let offset = floor_boundary(text, offset);
    text[..offset].chars().map(|c| c.len_utf16() as u32).sum()
}

/// カーソルのある行の開始バイト位置と、その行の中身 (改行を含まない)
fn line_at(text: &str, cursor: usize) -> (usize, &str) {
    let start = text[..cursor].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let end = text[cursor..]
        .find('\n')
        .map(|i| cursor + i)
        .unwrap_or(text.len());
    (start, &text[start..end])
}

/// カーソルが `pat("....")` のパターン文字列の中にあるとき、
/// 引用符の内側の `[start, end)` をバイトオフセットで返す。
///
/// AST を使わないので、打鍵の途中でパースが落ちていても動く。探索は
/// カーソルのある1行に閉じる。パターン文字列に改行は入らない
/// (`pattern_parser` が `o` `.` `x` しか受け付けない) ので、これで
/// 引用符の閉じ忘れが行をまたいで暴走することがない。
pub fn pat_pattern_span(text: &str, cursor: usize) -> Option<(usize, usize)> {
    let cursor = floor_boundary(text, cursor);
    let (line_start, line) = line_at(text, cursor);
    // コメント行は触らない
    if line.trim_start().starts_with('#') {
        return None;
    }
    let mut from = 0usize;
    while let Some(rel) = line[from..].find("pat(\"") {
        let open = from + rel + "pat(\"".len();
        // 引用符が閉じていない。この行のここから先に区間は作れない
        let close = open + line[open..].find('"')?;
        // 中身がパターン文字でないなら別物。壊さない
        if line[open..close]
            .chars()
            .all(|c| matches!(c, 'o' | '.' | 'x'))
        {
            let (a, b) = (line_start + open, line_start + close);
            // どちらの引用符に張り付いていても「中」とみなす
            if (a..=b).contains(&cursor) {
                return Some((a, b));
            }
        }
        from = close + 1;
    }
    None
}

/// 行頭の `$name =` を拾う。`import/chord.rs` と同じく module 直下の Lazy に置く
static DEFINE_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^[ \t]*\$([0-9A-Za-z_\p{Hiragana}\p{Katakana}\p{Han}ー]+)[ \t]*=").unwrap()
});

/// テキストから定義名を拾う。現れた順で、重複は最初の1つだけ残す。
///
/// AST を通さないのは、打鍵の途中でパースが落ちていてもチップを
/// 消さないため。`=` を要求するので、参照 (`$A | $B |`) は拾わない
pub fn define_names(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for caps in DEFINE_REGEX.captures_iter(text) {
        let name = caps[1].to_string();
        if !out.contains(&name) {
            out.push(name);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replace_range_puts_cursor_after_the_replacement() {
        let e = replace_range("C | Am |", 4, 6, "F");
        assert_eq!(e.text, "C | F |");
        assert_eq!(e.cursor, 5);
    }

    #[test]
    fn test_wrap_range_wraps_a_selection() {
        // `C | E D` を選んで pow で包む
        let text = "C | E D\n";
        let e = wrap_range(text, 0, 7, "pow(", ")");
        assert_eq!(e.text, "pow(C | E D)\n");
        // 選択があるときは包んだ後ろにカーソルを置く
        assert_eq!(e.cursor, 12);
    }

    #[test]
    fn test_wrap_range_without_selection_puts_cursor_inside() {
        let e = wrap_range("", 0, 0, "pow(", ")");
        assert_eq!(e.text, "pow()");
        // 括弧の内側
        assert_eq!(e.cursor, 4);
    }

    #[test]
    fn test_wrap_range_accepts_reversed_selection() {
        // 右から左へドラッグすると start > end で渡ってくる
        let e = wrap_range("C | E D\n", 7, 0, "pow(", ")");
        assert_eq!(e.text, "pow(C | E D)\n");
    }

    #[test]
    fn test_pat_span_inside_the_quotes() {
        let text = "pat(\"oxox\", C)\n";
        // `oxox` は 5..9
        assert_eq!(pat_pattern_span(text, 6), Some((5, 9)));
        // 引用符に張り付いていても中とみなす
        assert_eq!(pat_pattern_span(text, 5), Some((5, 9)));
        assert_eq!(pat_pattern_span(text, 9), Some((5, 9)));
    }

    #[test]
    fn test_pat_span_is_none_outside_the_quotes() {
        let text = "pat(\"oxox\", C)\n";
        // `C` の上
        assert_eq!(pat_pattern_span(text, 12), None);
        // `pat(` の上
        assert_eq!(pat_pattern_span(text, 1), None);
    }

    #[test]
    fn test_pat_span_picks_the_containing_one_when_nested() {
        //             0123456789...
        let text = "pat(\"oxox\", pat(\"oo\", C))\n";
        let outer = pat_pattern_span(text, 6).unwrap();
        let inner = pat_pattern_span(text, 18).unwrap();
        assert_eq!(&text[outer.0..outer.1], "oxox");
        assert_eq!(&text[inner.0..inner.1], "oo");
    }

    #[test]
    fn test_pat_span_ignores_unterminated_quote() {
        assert_eq!(pat_pattern_span("pat(\"oxox, C)\n", 7), None);
    }

    #[test]
    fn test_pat_span_ignores_non_pattern_body() {
        // 中身が o . x 以外なら別物。壊さない
        assert_eq!(pat_pattern_span("pat(\"abc\", C)\n", 6), None);
    }

    #[test]
    fn test_pat_span_ignores_a_reference_argument() {
        // Phase 2 の `pat($eight, ...)`。引用符が無いので区間は作らない
        assert_eq!(pat_pattern_span("pat($eight, C)\n", 7), None);
    }

    #[test]
    fn test_pat_span_ignores_comment_line() {
        assert_eq!(pat_pattern_span("# pat(\"oxox\", C)\n", 8), None);
    }

    #[test]
    fn test_pat_span_stays_on_the_cursor_line() {
        let text = "pat(\"oxox\", C)\nC | Am |\n";
        // 2行目にカーソルがあるなら1行目の pat は当たらない
        assert_eq!(pat_pattern_span(text, 17), None);
    }

    #[test]
    fn test_define_names_picks_line_heads() {
        let names = define_names("$eight = \"o.o.o.o.\"\n$riff  = pow(C | E D)\n$riff | C |\n");
        assert_eq!(names, vec!["eight".to_string(), "riff".to_string()]);
    }

    #[test]
    fn test_define_names_allows_japanese() {
        assert_eq!(
            define_names("$Aメロ = C | Am |\n"),
            vec!["Aメロ".to_string()]
        );
    }

    #[test]
    fn test_define_names_ignores_references_and_sustain() {
        // 参照だけの行、`=` (伸ばす) の小節、`$` の付かない行は拾わない
        assert!(define_names("$A | $B |\nC | = |\nkey: C\n").is_empty());
    }

    #[test]
    fn test_define_names_dedupes_keeping_first() {
        assert_eq!(define_names("$a = C\n$a = D\n"), vec!["a".to_string()]);
    }

    #[test]
    fn test_utf16_and_byte_round_trip() {
        let text = "$Aメロ = C\n";
        // `$Aメロ` は UTF-16 で 4 単位 ($ A メ ロ)、バイトで 8 (1+1+3+3)
        let b = utf16_to_byte(text, 4);
        assert_eq!(b, 8);
        assert_eq!(byte_to_utf16(text, b), 4);
        // 範囲外は末尾に丸める
        assert_eq!(utf16_to_byte(text, 999), text.len());
    }

    #[test]
    fn test_utf16_and_byte_never_land_mid_character() {
        // 日本語が混ざったテキストのすべての位置で、textarea 由来の値を
        // そのままスライスに使っても panic しないことを確かめる
        let text = "$Aメロ = C | Am |\n$Aメロ | $Aメロ |\n";
        let total: u32 = text.chars().map(|c| c.len_utf16() as u32).sum();
        for u in 0..=total {
            let b = utf16_to_byte(text, u);
            assert!(text.is_char_boundary(b), "u={u} b={b} は文字境界でない");
            assert_eq!(byte_to_utf16(text, b), u, "u={u} で往復しない");
        }
        // 文字の途中のバイト位置を渡しても丸められる ('メ' は 2..5)
        assert!(!text.is_char_boundary(3));
        let replaced = replace_range(text, 3, 8, "B");
        assert_eq!(&replaced.text[..7], "$AB = C");
        let wrapped = wrap_range(text, 8, 4, "pow(", ")");
        assert!(wrapped.text.starts_with("$Apow(メロ)"));
    }
}
