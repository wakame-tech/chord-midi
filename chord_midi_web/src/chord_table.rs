//! コード表タブ。ルート × 品質の格子ボタンを押すと、下の指板に
//! その押さえ方が出る。
//!
//! 押さえ方の探索は自前で持たない。コード名の文字列を組んで
//! `RechordImporter` → `resolve` → `chord_refs` → `shape_of` の
//! 既存経路をそのまま通す

use chord_midi::fingering::{chord_refs, shape_of, Shape, ShapeKind, FINGERINGS};
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

/// コード表の表示モード。画面が縦に伸びないよう、どちらか一方だけを出す
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    /// ルート × 品質の格子。どのコード名でも引けるが、近似が混ざる
    Grid,
    /// 原典にある押さえ方そのもの。近似が起きない
    Easy,
}

/// 押しているもの。モードで選べるものが違う
#[derive(Clone, Copy, PartialEq)]
enum Picked {
    None,
    /// `(ROOTS の添字, QUALITIES の添字)`
    Grid(usize, usize),
    /// `FINGERINGS` の添字
    Easy(usize),
}

/// 簡単コードのボタン1つ。`FINGERINGS` の添字と、画面に出す名前。
///
/// 同じ名前の形が複数ある (`C` が2つ、`C/E` `Em7` `D` `G` も2つずつ。
/// 押さえやすさ違いの別の形)。同じラベルが並ぶと区別できないので、
/// グループ内で2つ目以降に連番を振る
struct EasyButton {
    index: usize,
    label: String,
}

/// 原典の押さえ方をグループごとに分ける。可動形 (パワーコード /
/// オクターブ形) は原典の形ではないので外す
fn easy_groups() -> Vec<(&'static str, Vec<EasyButton>)> {
    let mut out: Vec<(&'static str, Vec<EasyButton>)> = Vec::new();
    for (index, f) in FINGERINGS.iter().enumerate() {
        if f.kind != ShapeKind::Normal {
            continue;
        }
        let group = match out.iter_mut().find(|(g, _)| *g == f.group) {
            Some(g) => g,
            None => {
                out.push((f.group, Vec::new()));
                out.last_mut().expect("push した直後")
            }
        };
        let seen = group
            .1
            .iter()
            .filter(|b| FINGERINGS[b.index].name == f.name)
            .count();
        let label = if seen == 0 {
            f.name.to_string()
        } else {
            format!("{} ({})", f.name, seen + 1)
        };
        group.1.push(EasyButton { index, label });
    }
    out
}

#[function_component(ChordTable)]
pub fn chord_table() -> Html {
    let mode = use_state(|| Mode::Grid);
    let selected = use_state(|| Picked::None);

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
                        let selected = selected.clone();
                        Callback::from(move |_| selected.set(Picked::Grid(ri, qi)))
                    };
                    let is_active = *selected == Picked::Grid(ri, qi);
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

    let easy = easy_groups()
        .into_iter()
        .map(|(group, buttons)| {
            let cells = buttons
                .iter()
                .map(|b| {
                    let onclick = {
                        let selected = selected.clone();
                        let index = b.index;
                        Callback::from(move |_| selected.set(Picked::Easy(index)))
                    };
                    let is_active = *selected == Picked::Easy(b.index);
                    let class =
                        classes!("btn", "chord-table-cell", is_active.then_some("is-active"));
                    html! { <button type="button" {class} {onclick}>{&b.label}</button> }
                })
                .collect::<Html>();
            html! {
                <div class="block">
                    <span class="block-label">{group}</span>
                    <div class="chord-table-easy">{cells}</div>
                </div>
            }
        })
        .collect::<Html>();

    let detail = match *selected {
        Picked::None => html! {
            <p class="empty">{"コードを押すと押さえ方が出ます。"}</p>
        },
        // 原典の形をそのまま出す。`find` を通さないので近似が起きない
        Picked::Easy(index) => {
            let f = &FINGERINGS[index];
            let shape = Shape::from(f);
            html! {
                <>
                    <div class="chord-table-name">{f.name}</div>
                    { board(shape.frets, shape.fingers, None) }
                </>
            }
        }
        Picked::Grid(ri, qi) => {
            let name = format!("{}{}", ROOTS[ri], QUALITIES[qi].1);
            match shape_for(&name) {
                None => html! {
                    <>
                        <div class="chord-table-name">{name}</div>
                        <p class="empty">{"押さえ方データなし"}</p>
                    </>
                },
                Some(shape) => {
                    // 代替の押さえ方が選ばれたときだけ原典での名前を併記する。
                    // プレビューの図と同じ規則
                    let alias = match shape.name {
                        Some(n) if n != name => html! {
                            <span class="chord-alias">{format!(" (≒ {})", n)}</span>
                        },
                        _ => html! {},
                    };
                    html! {
                        <>
                            <div class="chord-table-name">{name}{alias}</div>
                            { board(shape.frets, shape.fingers, None) }
                        </>
                    }
                }
            }
        }
    };

    let switch = [(Mode::Grid, "格子"), (Mode::Easy, "簡単コード")]
        .into_iter()
        .map(|(m, label)| {
            let mode = mode.clone();
            let is_active = *mode == m;
            let onclick = Callback::from(move |_| mode.set(m));
            html! {
                <button
                    class={classes!("btn", "chord-table-mode", is_active.then_some("is-active"))}
                    {onclick}
                >{label}</button>
            }
        })
        .collect::<Html>();

    let body = match *mode {
        Mode::Grid => html! {
            <div class="chord-table-grid">
                // 左上の角。ルート名の列の見出しは無い
                <div class="chord-table-head"></div>
                {header}
                {rows}
            </div>
        },
        Mode::Easy => html! { <div>{easy}</div> },
    };

    html! {
        <div class="chord-table">
            <div class="chord-table-modes">{switch}</div>
            {body}
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
}
