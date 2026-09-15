//! 押さえ方を端末向けの ASCII 図に起こす。
//!
//! web 版 (`chord_midi_web::diagram`) が SVG 風の DOM で描くものと同じ内容を、
//! 等幅フォント前提の文字だけで描く。指の表記はギターで一般的な番号
//! (人差し指=1, 中指=2, 薬指=3, 親指=T) を使う。日本語1文字だと端末で
//! 全角幅になって桁がずれるため。

use chord_midi::fingering::{Finger, Fingering};

/// 一度に描くフレット数。web 版と揃える
const FRET_COUNT: u8 = 5;

/// 行頭の "弦番号 + 開放/ミュート記号 + 指板の左端" の文字数
const ROW_PREFIX_WIDTH: usize = 5;

/// 1マスの文字数。`-X-|` の4文字
const CELL_WIDTH: usize = 4;

fn finger_label(f: Finger) -> char {
    match f {
        Finger::Index => '1',
        Finger::Middle => '2',
        Finger::Ring => '3',
        Finger::Thumb => 'T',
    }
}

/// 描き始めるフレット。5フレットに収まらないときだけ最小フレットから始める
fn window_start(f: &Fingering) -> u8 {
    match (f.min_fret(), f.max_fret()) {
        (Some(min), Some(max)) if max > FRET_COUNT => min,
        _ => 1,
    }
}

/// コード1つ分の図。見つからなければ「押さえ方データなし」だけ返す
pub fn render(label: &str, fingering: Option<&'static Fingering>) -> String {
    let Some(f) = fingering else {
        return format!("{}\n  (押さえ方データなし)\n", label);
    };
    // 代替の押さえ方が選ばれたときだけ、原典での名前を併記する
    let head = if f.name == label {
        label.to_string()
    } else {
        format!("{} (≒ {})", label, f.name)
    };

    let start = window_start(f);
    let mut out = String::new();
    out.push_str(&head);
    out.push('\n');

    // フレット番号の行。数字がマスの中央 (`-X-` の X) に来るように詰める
    out.push_str(&" ".repeat(ROW_PREFIX_WIDTH));
    for i in 0..FRET_COUNT {
        out.push_str(&format!(" {:<width$}", start + i, width = CELL_WIDTH - 1));
    }
    // 右端の詰め物は行末に残さない
    while out.ends_with(' ') {
        out.pop();
    }
    out.push('\n');

    // 1弦を上に描く。frets は 6弦→1弦 の順なので逆から回す
    for s in (0..6usize).rev() {
        let marker = match f.frets[s] {
            None => 'x',
            Some(0) => 'o',
            Some(_) => ' ',
        };
        out.push_str(&format!("{} {} |", 6 - s, marker));
        for i in 0..FRET_COUNT {
            let fret = start + i;
            let dot = match f.frets[s] {
                Some(v) if v == fret => f.fingers[s].map(finger_label).unwrap_or('*'),
                _ => '-',
            };
            out.push_str(&format!("-{}-|", dot));
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use chord_midi::fingering::FINGERINGS;

    fn by_name(name: &str) -> &'static Fingering {
        FINGERINGS.iter().find(|f| f.name == name).unwrap()
    }

    #[test]
    fn test_render_open_chord() {
        let out = render("Am7", Some(by_name("Am7")));
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "Am7");
        // フレット番号が各マスの中央 (`-X-` の X) の桁に乗っている
        for i in 0..FRET_COUNT as usize {
            let col = ROW_PREFIX_WIDTH + i * CELL_WIDTH + 1;
            assert_eq!(lines[1].as_bytes()[col], b'1' + i as u8);
        }
        for line in &lines[2..] {
            assert_eq!(
                line.len(),
                ROW_PREFIX_WIDTH + FRET_COUNT as usize * CELL_WIDTH
            );
        }
        // Am7 は 6弦/1弦がミュート、5弦が開放
        assert!(lines[7].starts_with("6 x |"));
        assert!(lines[2].starts_with("1 x |"));
        assert!(lines[6].starts_with("5 o |"));
        // 4弦の2フレットを中指で押さえる
        assert_eq!(lines[5], "4   |---|-2-|---|---|---|");
    }

    #[test]
    fn test_render_alias_is_shown() {
        let f = by_name("Am7");
        let out = render("Am", Some(f));
        assert_eq!(out.lines().next().unwrap(), "Am (≒ Am7)");
    }

    #[test]
    fn test_render_missing() {
        let out = render("X", None);
        assert!(out.contains("押さえ方データなし"));
    }
}
