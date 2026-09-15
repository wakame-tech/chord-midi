//! リズム入力のグリッド。マスを `o` → `.` → `x` と回して、
//! できたパターンを `pat()` としてエディタに入れる。
//!
//! 文字列は `chord_midi` の `Pattern` の `Display` で作る。自分で
//! 組むとパーサの受け付ける形とズレる余地が残るため

use chord_midi::model::ast::{Beat, Pattern};
use web_sys::HtmlSelectElement;
use yew::prelude::*;

use crate::cursor::Edit;

#[derive(Properties, PartialEq)]
pub struct RhythmGridProps {
    pub on_edit: Callback<Edit>,
}

/// グリッドで扱う分割数。記法は 1 と 2 も許すが、マスが少なすぎて
/// GUI にする意味がないので手で書いてもらう
const DIVISIONS: [usize; 3] = [4, 8, 16];

/// 最初に出す分割数。8分が一番よく使われるため
const DEFAULT_DIVISION: usize = 8;

/// `o` → `.` → `x` → `o` の順に回す
fn next_beat(b: Beat) -> Beat {
    match b {
        Beat::Hit => Beat::Rest,
        Beat::Rest => Beat::Brush,
        Beat::Brush => Beat::Hit,
    }
}

fn beat_char(b: Beat) -> &'static str {
    match b {
        Beat::Hit => "o",
        Beat::Rest => ".",
        Beat::Brush => "x",
    }
}

/// 読み上げ用の呼び名。`reference::BEATS` の `summary` と同じ言葉にする
fn beat_name(b: Beat) -> &'static str {
    match b {
        Beat::Hit => "打弦",
        Beat::Rest => "休符",
        Beat::Brush => "ブラッシング",
    }
}

/// 頭打ち。README の `"o.o.o.o."` と同じ形
fn default_beats(n: usize) -> Vec<Beat> {
    (0..n)
        .map(|i| if i % 2 == 0 { Beat::Hit } else { Beat::Rest })
        .collect()
}

#[function_component(RhythmGrid)]
pub fn rhythm_grid(props: &RhythmGridProps) -> Html {
    let beats = use_state(|| default_beats(DEFAULT_DIVISION));
    let pattern = Pattern((*beats).clone()).to_string();

    // 分割数を変えたらマスは作り直す。4分割の `o.o.` を8分割へ
    // 引き伸ばす決まった正解が無いため
    let on_division = {
        let beats = beats.clone();
        Callback::from(move |e: Event| {
            let el: HtmlSelectElement = e.target_unchecked_into();
            if let Ok(n) = el.value().parse::<usize>() {
                beats.set(default_beats(n));
            }
        })
    };

    let cells = (*beats)
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let b = *b;
            let onclick = {
                let beats = beats.clone();
                Callback::from(move |_| {
                    let mut next = (*beats).clone();
                    next[i] = next_beat(b);
                    beats.set(next);
                })
            };
            let class = classes!("rhythm-cell", matches!(b, Beat::Rest).then_some("is-rest"));
            // マスは `o` / `.` / `x` の見た目でしか区別できないので、
            // aria-label で「3マス目 休符」と言葉にする
            html! {
                <button type="button" {class} {onclick}
                    aria-label={format!("{}マス目 {}", i + 1, beat_name(b))}>
                    {beat_char(b)}
                </button>
            }
        })
        .collect::<Html>();

    let on_insert = {
        let on_edit = props.on_edit.clone();
        let pattern = pattern.clone();
        Callback::from(move |_| on_edit.emit(Edit::Rhythm(pattern.clone())))
    };

    html! {
        <div class="block">
            <span class="block-label">{"リズム"}</span>
            <div class="rhythm-row">
                <select onchange={on_division} aria-label={"分割数"}>
                    { for DIVISIONS.iter().map(|n| html! {
                        <option value={n.to_string()} selected={*n == beats.len()}>
                            {format!("{}分割", n)}
                        </option>
                    }) }
                </select>
                <code class="rhythm-preview">{pattern}</code>
            </div>
            <div class="rhythm-grid">{cells}</div>
            <button class="btn btn-primary" onclick={on_insert}>{"pat() を入れる"}</button>
            <p class="rhythm-help">
                {"マスを押すと o (打弦) → . (休符) → x (ブラッシング) と変わる。\
                  分割数を変えるとマスは作り直す。"}
            </p>
            <p class="rhythm-help">
                {"カーソルが pat(\"…\") の引用符の中にあるときは、パターンだけを差し替える。"}
            </p>
        </div>
    }
}
