use chord_midi::fingering::{identify, Candidate, Source};
use yew::prelude::*;

use crate::diagram::POSITION_MARKS;

/// 描く一番上のフレット。0（開放）から数える。12 はオクターブなので、
/// 可動形も含めてそこまで描ければ実用上足りる。
/// style.css の --fretboard-cols の列数は MAX_FRET + 1 に合わせること
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
    // 216通り (12ルート x 18品質) のビット演算だけなので、レンダリングごとに
    // 呼んでも重くない。use_memo は検討済みだが、鍵にする props.frets が
    // クリックのたびにどうせ変わるので割に合わない
    let candidates = identify(props.frets);
    let all_muted = props.frets.iter().all(Option::is_none);
    let on_clear = {
        let on_change = props.on_change.clone();
        Callback::from(move |_| on_change.emit([None; 6]))
    };
    html! {
        <div>
            <p class="fretboard-help">
                {"指板を押すとその弦の音が決まる。同じところをもう一度押すとミュート。"}
            </p>
            { board(props.frets, &props.on_change) }
            <div class="block">
                <button class="btn" onclick={on_clear}>{"クリア"}</button>
            </div>
            <div class="block">
                <span class="block-label">{"コードの候補"}</span>
                { candidate_cards(&candidates, &props.on_insert, all_muted) }
            </div>
        </div>
    }
}

fn board(frets: [Option<u8>; 6], on_change: &Callback<[Option<u8>; 6]>) -> Html {
    // 1弦を上に描く。frets は 6弦→1弦 の順なので逆から回す
    let rows = (0..6usize)
        .rev()
        .map(|s| {
            let value = match frets[s] {
                None => "×".to_string(),
                Some(0) => "○".to_string(),
                Some(n) => n.to_string(),
            };
            let cells = (0..=MAX_FRET)
                .map(|fret| {
                    let onclick = {
                        let on_change = on_change.clone();
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
                    let on = frets[s] == Some(fret);
                    // 0列は開放弦。ナットの左側なので弦だけ描いてフレット線は描かない。
                    // ナットを0の右ではなく1の左に引くのは、0列にはそもそもフレット線を
                    // 描かないので引く場所が無いため
                    let class = classes!(
                        "fretboard-cell",
                        (fret == 0).then_some("is-open"),
                        (fret == 1).then_some("is-nut"),
                    );
                    let dot = on.then(|| html! { <span class="fretboard-dot"></span> });
                    // キーボードでも押せるよう実ボタンにする。弦とフレットは
                    // 見た目のグリッド位置でしか区別できないので aria-label で言葉にする
                    html! {
                        <button type="button" {class} {onclick}
                            aria-label={format!("{}弦 {}フレット", 6 - s, fret)}
                            aria-pressed={on.to_string()}>
                            {dot}
                        </button>
                    }
                })
                .collect::<Html>();
            html! {
                <>
                    <div class="fretboard-label">
                        {format!("{}弦", 6 - s)}
                        <span class="fretboard-value">{value}</span>
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
            html! { <div class="fretboard-number">{text}</div> }
        })
        .collect::<Html>();

    html! {
        <div class="fretboard-scroll">
            <div class="fretboard-grid">{rows}</div>
            <div class="fretboard-numbers">
                // 左端は弦名の列のぶん。fretboard-label と一緒に横スクロールで固定する
                <div class="fretboard-numbers-spacer"></div>
                {numbers}
            </div>
        </div>
    }
}

fn candidate_cards(
    candidates: &[Candidate],
    on_insert: &Callback<String>,
    all_muted: bool,
) -> Html {
    if candidates.is_empty() {
        let message = if all_muted {
            "指板を押すと候補が出ます"
        } else {
            "該当なし"
        };
        return html! { <p class="empty">{message}</p> };
    }
    let cards = candidates
        .iter()
        .map(|c| {
            let onclick = {
                let on_insert = on_insert.clone();
                let notation = c.notation.clone();
                Callback::from(move |_| on_insert.emit(notation.clone()))
            };
            let (label, is_data) = match c.source {
                Source::Data(_) => ("原典", true),
                Source::Theory => ("計算", false),
            };
            let badge_class = classes!("fretboard-badge", is_data.then_some("is-data"));
            // 原典での呼び名が rechord の名前と違うときだけ併記する。
            // diagram.rs の「≒」と同じ考え方
            let alias = match c.source {
                Source::Data(f) if f.name != c.name => Some(html! {
                    <div class="fretboard-alias">{format!("原典では {}", f.name)}</div>
                }),
                _ => None,
            };
            let note = (!c.reproduces_shape).then(|| {
                html! {
                    <div class="fretboard-note">
                        {"この形は今の押さえ方データでは再現できない"}
                    </div>
                }
            });
            html! {
                <div class="fretboard-card">
                    <div class="fretboard-card-head">
                        <span class="fretboard-card-name">{&c.name}</span>
                        <span class={badge_class}>{label}</span>
                    </div>
                    {alias}
                    <div class="fretboard-card-body">
                        <code class="fretboard-notation">{&c.notation}</code>
                        <button class="btn" {onclick}>{"挿入"}</button>
                    </div>
                    {note}
                </div>
            }
        })
        .collect::<Html>();
    html! { <div class="fretboard-cards">{cards}</div> }
}
