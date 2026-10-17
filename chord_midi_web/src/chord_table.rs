//! コード表タブ。ルート × 品質の格子ボタンを押すと、下の指板に
//! その押さえ方が出る。
//!
//! 押さえ方の探索は自前で持たない。コード名の文字列を組んで
//! `RechordImporter` → `resolve` → `chord_refs` → `shape_of` の
//! 既存経路をそのまま通す

use chord_midi::fingering::{chord_refs, shape_of, Fingering, Shape, ShapeKind, FINGERINGS};
use chord_midi::import::{Importer, RechordImporter};
use chord_midi::model::pitch::Pitch;
use std::str::FromStr;
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

/// 格子で選んだコード。`(ROOTS の添字, QUALITIES の添字)`
type Picked = Option<(usize, usize)>;

/// 押さえ方の亜種1つ
struct Variant {
    label: String,
    shape: Shape,
}

/// 押さえる指の本数。開放弦とミュートは数えない
fn pressed_fingers(f: &Fingering) -> usize {
    f.frets
        .iter()
        .filter(|x| matches!(x, Some(n) if *n > 0))
        .count()
}

/// コード1つの押さえ方の亜種を集める。
///
/// 原典に同名の形があればそれを全部 (押さえる指の少ない順)、無ければ
/// `find` の近似を1つ。加えて、ルートが一致する可動形 (パワーコードと
/// オクターブ形) を並べる。
///
/// ラベルは押さえる指の本数にする。原典に「通常 / 簡単」の区別は入って
/// おらず、`Em7` と `G` は指が一番少ない形こそが標準形なので、機械的に
/// 決めると嘘になる
fn variants_for(root: &str, suffix: &str) -> Vec<Variant> {
    let name = format!("{}{}", root, suffix);
    let mut out: Vec<Variant> = Vec::new();

    let mut exact: Vec<&'static Fingering> = FINGERINGS
        .iter()
        .filter(|f| f.kind == ShapeKind::Normal && f.name == name)
        .collect();
    exact.sort_by_key(|f| pressed_fingers(f));
    for f in &exact {
        out.push(Variant {
            label: format!("指{}本", pressed_fingers(f)),
            shape: Shape::from(*f),
        });
    }

    // 原典に無い名前は近似に落ちる。どの形で代用しているかをラベルに出す
    if exact.is_empty() {
        if let Some(shape) = shape_for(&name) {
            let label = match shape.name {
                Some(n) => format!("近似 (≒ {})", n),
                None => "近似".to_string(),
            };
            out.push(Variant { label, shape });
        }
    }

    // 可動形はルートのピッチクラスだけで決まる。品質には関係しない
    if let Ok(pitch) = Pitch::from_str(root) {
        let pc = pitch as u8;
        for (kind, label) in [
            (ShapeKind::Power, "パワー"),
            (ShapeKind::Octave, "オクターブ"),
        ] {
            for f in FINGERINGS.iter().filter(|f| f.kind == kind && f.bass == pc) {
                out.push(Variant {
                    // `lowest_string` は 0 = 6弦
                    label: format!("{} ({}弦ルート)", label, 6 - f.lowest_string),
                    shape: Shape::from(f),
                });
            }
        }
    }

    out
}

#[function_component(ChordTable)]
pub fn chord_table() -> Html {
    // 格子で選んだコードと、その中で選んだ亜種の添字。
    // コードを選び直したら亜種は先頭に戻す
    let picked = use_state(|| None as Picked);
    let variant = use_state(|| 0usize);

    let header = QUALITIES
        .iter()
        .map(|(label, _)| html! { <div class="chord-table-head">{*label}</div> })
        .collect::<Html>();

    let rows = ROOTS
        .iter()
        .enumerate()
        .map(|(ri, root)| {
            let cells = QUALITIES
                .iter()
                .enumerate()
                .map(|(qi, (_, suffix))| {
                    let onclick = {
                        let picked = picked.clone();
                        let variant = variant.clone();
                        Callback::from(move |_| {
                            picked.set(Some((ri, qi)));
                            variant.set(0);
                        })
                    };
                    let is_active = *picked == Some((ri, qi));
                    let class =
                        classes!("btn", "chord-table-cell", is_active.then_some("is-active"));
                    html! {
                        <button type="button" {class} {onclick}>
                            {format!("{}{}", root, suffix)}
                        </button>
                    }
                })
                .collect::<Html>();
            html! {
                <>
                    <div class="chord-table-root">{*root}</div>
                    {cells}
                </>
            }
        })
        .collect::<Html>();

    let detail = match *picked {
        None => html! {
            <p class="empty">{"コードを押すと押さえ方が出ます。"}</p>
        },
        Some((ri, qi)) => {
            let name = format!("{}{}", ROOTS[ri], QUALITIES[qi].1);
            let list = variants_for(ROOTS[ri], QUALITIES[qi].1);
            // 亜種を選び直す前にコードが変わると添字がはみ出る。先頭に丸める
            let index = (*variant).min(list.len().saturating_sub(1));
            let buttons = list
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    let onclick = {
                        let variant = variant.clone();
                        Callback::from(move |_| variant.set(i))
                    };
                    let class = classes!(
                        "btn",
                        "chord-table-variant",
                        (i == index).then_some("is-active")
                    );
                    html! { <button type="button" {class} {onclick}>{&v.label}</button> }
                })
                .collect::<Html>();
            let shape = match list.get(index) {
                None => html! { <p class="empty">{"押さえ方データなし"}</p> },
                Some(v) => board(v.shape.frets, v.shape.fingers, None),
            };
            html! {
                <>
                    <div class="chord-table-name">{name}</div>
                    <div class="chord-table-variants">{buttons}</div>
                    {shape}
                </>
            }
        }
    };

    html! {
        <div class="chord-table">
            <div class="chord-table-grid">
                // 左上の角。ルート名の列の見出しは無い
                <div class="chord-table-head"></div>
                {header}
                {rows}
            </div>
            <div class="chord-table-detail">{detail}</div>
        </div>
    }
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
        assert_eq!(
            shape.frets,
            [None, Some(0), Some(2), Some(0), Some(1), None]
        );
        assert_eq!(shape.name, Some("Am7"));
    }

    #[test]
    fn shape_for_rejects_a_bad_name() {
        assert!(shape_for("Hm7").is_none());
    }

    #[test]
    fn variants_list_both_shapes_when_the_book_has_two() {
        // C は原典に2つある。押さえる指の少ない順
        let v = variants_for("C", "");
        let labels: Vec<&str> = v.iter().map(|x| x.label.as_str()).collect();
        assert_eq!(labels[0], "指2本");
        assert_eq!(labels[1], "指3本");
        assert_eq!(
            v[0].shape.frets,
            [None, Some(3), None, Some(0), Some(1), None]
        );
        assert_eq!(
            v[1].shape.frets,
            [None, Some(3), Some(2), Some(0), Some(1), None]
        );
    }

    #[test]
    fn variants_fall_back_to_an_approximation() {
        // Cm は原典に無いので近似が1つだけ出る。後ろは可動形
        let v = variants_for("C", "m");
        assert!(
            v[0].label.starts_with("近似 (≒ "),
            "近似のラベル: {}",
            v[0].label
        );
    }

    #[test]
    fn variants_include_movable_shapes() {
        let v = variants_for("C", "");
        let labels: Vec<&str> = v.iter().map(|x| x.label.as_str()).collect();
        for want in [
            "パワー (6弦ルート)",
            "パワー (5弦ルート)",
            "オクターブ (6弦ルート)",
            "オクターブ (5弦ルート)",
        ] {
            assert!(labels.contains(&want), "{} が無い: {:?}", want, labels);
        }
    }

    #[test]
    fn every_cell_of_the_grid_has_at_least_one_variant() {
        for root in ROOTS {
            for (_, suffix) in QUALITIES {
                let v = variants_for(root, suffix);
                assert!(!v.is_empty(), "亜種が空: {}{}", root, suffix);
            }
        }
    }

    /// 原典に同名の形が2つあるのは5種だけ。設計書の表と食い違ったら気づけるようにする
    #[test]
    fn only_five_chords_have_two_shapes_in_the_book() {
        let mut names: Vec<&str> = Vec::new();
        for f in FINGERINGS.iter().filter(|f| f.kind == ShapeKind::Normal) {
            if FINGERINGS
                .iter()
                .filter(|g| g.kind == ShapeKind::Normal && g.name == f.name)
                .count()
                > 1
                && !names.contains(&f.name)
            {
                names.push(f.name);
            }
        }
        names.sort_unstable();
        assert_eq!(names, ["C", "C/E", "D", "Em7", "G"]);
    }
}
