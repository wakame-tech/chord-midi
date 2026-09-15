use chord_midi::fingering::Fingering;
use yew::prelude::*;

/// 一度に描くフレット数
const FRET_COUNT: u8 = 5;

#[derive(Properties, PartialEq)]
pub struct ChordDiagramProps {
    /// 入力に書かれたコード名
    pub label: String,
    /// 見つかった押さえ方。無ければ None
    pub fingering: Option<&'static Fingering>,
    /// カードの色 (CSS の色文字列)
    pub color: String,
}

#[function_component(ChordDiagram)]
pub fn chord_diagram(props: &ChordDiagramProps) -> Html {
    let body = match props.fingering {
        Some(f) => diagram(f),
        None => html! { <div class="diagram-empty">{"データなし"}</div> },
    };
    // 代替の押さえ方が選ばれたときだけ原典での名前を出す
    let alias = props.fingering.and_then(|f| {
        if f.name == props.label {
            None
        } else {
            Some(html! { <div class="chord-alias">{format!("≈ {}", f.name)}</div> })
        }
    });
    html! {
        <div class="chord-card" style={format!("--chord-color: {}", props.color)}>
            <div class="chord-name">{&props.label}</div>
            {alias}
            {body}
        </div>
    }
}

/// 描き始めるフレット。5フレットに収まらないときだけ最小フレットから始める
fn window_start(f: &Fingering) -> u8 {
    match (f.min_fret(), f.max_fret()) {
        (Some(min), Some(max)) if max > FRET_COUNT => min,
        _ => 1,
    }
}

fn diagram(f: &Fingering) -> Html {
    let start = window_start(f);
    // 1弦を上に描く。frets は 6弦→1弦 の順なので逆から回す
    let rows = (0..6usize)
        .rev()
        .map(|s| {
            let marker = match f.frets[s] {
                None => "×",
                Some(0) => "○",
                Some(_) => "",
            };
            let cells = (0..FRET_COUNT)
                .map(|i| {
                    let fret = start + i;
                    let dot = match f.frets[s] {
                        Some(v) if v == fret => html! {
                            <span class="dot">
                                {f.fingers[s].map(|g| g.label()).unwrap_or("")}
                            </span>
                        },
                        _ => Html::default(),
                    };
                    // ナットは0フレットにしか無い。窓が途中から始まるときは普通のフレット線
                    let class = match (i, start) {
                        (0, 1) => "cell nut",
                        (0, _) => "cell edge",
                        _ => "cell",
                    };
                    html! { <div {class}>{dot}</div> }
                })
                .collect::<Html>();
            html! {
                <>
                    <div class="marker">{marker}</div>
                    {cells}
                </>
            }
        })
        .collect::<Html>();

    html! {
        <div class="diagram">
            <div class="grid">{rows}</div>
            if start > 1 {
                <div class="fret-label">{format!("{}fr", start)}</div>
            }
        </div>
    }
}
