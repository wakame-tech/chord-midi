//! コード表タブ。ルート × 品質の格子ボタンを押すと、下の指板に
//! その押さえ方が出る。
//!
//! 押さえ方の探索は自前で持たない。コード名の文字列を組んで
//! `RechordImporter` → `resolve` → `chord_refs` → `shape_of` の
//! 既存経路をそのまま通す

use chord_midi::fingering::{chord_refs, shape_of, Shape};
use chord_midi::import::{Importer, RechordImporter};
use yew::prelude::*;

use crate::fretboard::board;

/// 格子の行。記法のルート名をそのまま使う
pub const ROOTS: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

/// 格子の列。`(見出し, 記法の接尾辞)`。メジャーは接尾辞が無いので
/// 見出しだけ `(maj)` と書く
pub const QUALITIES: [(&str, &str); 11] = [
    ("(maj)", ""),
    ("m", "m"),
    ("7", "7"),
    ("m7", "m7"),
    ("M7", "M7"),
    ("m7b5", "m7b5"),
    ("sus4", "sus4"),
    ("6", "6"),
    ("add9", "add9"),
    ("dim", "dim"),
    ("aug", "aug"),
];

/// コード名1つの押さえ方を引く。読めない名前や形が無い組み合わせは None。
///
/// 1小節だけの進行として読ませる。`resolve` を通すのは、度数表記が
/// 混ざっていなくても `chord_refs` が解決済みの木を要求するため
pub fn shape_for(name: &str) -> Option<Shape> {
    let ast = RechordImporter.import(&format!("{} |\n", name)).ok()?;
    let resolved = ast.resolve().ok()?;
    let refs = chord_refs(&resolved).ok()?;
    shape_of(refs.first()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shape_for_reads_every_cell_of_the_grid() {
        let mut found = 0;
        let mut missing: Vec<String> = Vec::new();
        for root in ROOTS {
            for (_, suffix) in QUALITIES {
                let name = format!("{}{}", root, suffix);
                match shape_for(&name) {
                    Some(_) => found += 1,
                    None => missing.push(name),
                }
            }
        }
        // 132通りのうち、いくつかは押さえ方が無い。全部無いとか全部あるとかに
        // なっていたら、記法の組み立てか経路のどこかが壊れている
        assert!(found > 60, "引けた数が少なすぎる: {} 件", found);
        assert!(missing.len() < 60, "引けない数が多すぎる: {:?}", missing);
    }

    #[test]
    fn shape_for_finds_a_known_chord() {
        let shape = shape_for("Am7").expect("Am7 は原典にある");
        assert_eq!(shape.frets, [None, Some(0), Some(2), Some(0), Some(1), None]);
        assert_eq!(shape.name, Some("Am7"));
    }

    #[test]
    fn shape_for_rejects_a_bad_name() {
        assert!(shape_for("Hm7").is_none());
    }
}
