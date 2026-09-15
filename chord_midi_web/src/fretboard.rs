use chord_midi::fingering::{identify, Candidate, Source};
use yew::prelude::*;

use crate::diagram::POSITION_MARKS;

/// 描く一番上のフレット。0（開放）から数える
const MAX_FRET: u8 = 12;

#[derive(Properties, PartialEq)]
pub struct FretboardProps {
    /// 6弦→1弦。None = ミュート、Some(0) = 開放
    pub frets: [Option<u8>; 6],
    pub on_change: Callback<[Option<u8>; 6]>,
    /// 記法をエディタに挿入する
    pub on_insert: Callback<String>,
}

#[function_component(Fretboard)]
pub fn fretboard(props: &FretboardProps) -> Html {
    let candidates = identify(props.frets);
    let on_clear = {
        let on_change = props.on_change.clone();
        Callback::from(move |_| on_change.emit([None; 6]))
    };
    html! {
        <div>
            <p class="fretboard-help">
                {"指板を押すとその弦の音が決まる。同じところをもう一度押すとミュート。"}
            </p>
            { board(props) }
            <div class="block">
                <button class="btn" onclick={on_clear}>{"クリア"}</button>
            </div>
            <div class="block">
                <span class="block-label">{"コードの候補"}</span>
                { results(&candidates, &props.on_insert) }
            </div>
        </div>
    }
}

fn board(props: &FretboardProps) -> Html {
    // 1弦を上に描く。frets は 6弦→1弦 の順なので逆から回す
    let rows = (0..6usize)
        .rev()
        .map(|s| {
            let value = match props.frets[s] {
                None => "×".to_string(),
                Some(0) => "○".to_string(),
                Some(n) => n.to_string(),
            };
            let cells = (0..=MAX_FRET)
                .map(|fret| {
                    let onclick = {
                        let frets = props.frets;
                        let on_change = props.on_change.clone();
                        Callback::from(move |_| {
                            let mut next = frets;
                            // 同じところをもう一度押したらその弦をミュートにする
                            next[s] = if next[s] == Some(fret) {
                                None
                            } else {
                                Some(fret)
                            };
                            on_change.emit(next);
                        })
                    };
                    let on = props.frets[s] == Some(fret);
                    // 0列は開放弦。ナットの左側なので弦だけ描いてフレット線は描かない
                    let class = classes!(
                        "fb-cell",
                        (fret == 0).then_some("is-open"),
                        (fret == 1).then_some("is-nut"),
                    );
                    let dot = if on {
                        html! { <span class="fb-dot"></span> }
                    } else {
                        Html::default()
                    };
                    html! { <div {class} {onclick}>{dot}</div> }
                })
                .collect::<Html>();
            html! {
                <>
                    <div class="fb-label">
                        {format!("{}弦", 6 - s)}
                        <span class="fb-value">{value}</span>
                    </div>
                    {cells}
                </>
            }
        })
        .collect::<Html>();

    // 指板の下のフレット番号。実ギターのポジションマークと同じ位置
    let numbers = (0..=MAX_FRET)
        .map(|fret| {
            let text = if POSITION_MARKS.contains(&fret) {
                fret.to_string()
            } else {
                String::new()
            };
            html! { <div class="fb-number">{text}</div> }
        })
        .collect::<Html>();

    html! {
        <div class="fretboard-scroll">
            <div class="fb-grid">{rows}</div>
            <div class="fb-numbers">
                // 左端は弦名の列のぶん
                <div></div>
                {numbers}
            </div>
        </div>
    }
}

fn results(candidates: &[Candidate], on_insert: &Callback<String>) -> Html {
    if candidates.is_empty() {
        return html! { <p class="empty">{"該当なし"}</p> };
    }
    let cards = candidates
        .iter()
        .map(|c| {
            let onclick = {
                let on_insert = on_insert.clone();
                let notation = c.notation.clone();
                Callback::from(move |_| on_insert.emit(notation.clone()))
            };
            let (badge, badge_class) = match c.source {
                Source::Data(_) => ("原典", "fb-badge is-data"),
                Source::Theory => ("計算", "fb-badge"),
            };
            // 原典での呼び名が rechord の名前と違うときだけ併記する。
            // diagram.rs の「≒」と同じ考え方
            let alias = match c.source {
                Source::Data(f) if f.name != c.name => html! {
                    <div class="fb-alias">{format!("原典では {}", f.name)}</div>
                },
                _ => Html::default(),
            };
            let note = if c.unreachable {
                html! {
                    <div class="fb-note">{"この形は今の押さえ方データでは再現できない"}</div>
                }
            } else {
                Html::default()
            };
            html! {
                <div class="fb-card">
                    <div class="fb-card-head">
                        <span class="fb-card-name">{&c.name}</span>
                        <span class={badge_class}>{badge}</span>
                    </div>
                    {alias}
                    <div class="fb-card-body">
                        <code class="fb-notation">{&c.notation}</code>
                        <button class="btn" {onclick}>{"挿入"}</button>
                    </div>
                    {note}
                </div>
            }
        })
        .collect::<Html>();
    html! { <div class="fb-cards">{cards}</div> }
}
