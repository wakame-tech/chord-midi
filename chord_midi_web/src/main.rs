use chord_midi::{
    edit::{set_scale, shift_key},
    export::{Exporter, MidiExporter},
    fingering::{self, MeasureRef},
    import::{Importer, RechordImporter},
    model::scale::Scale,
};
use diagram::ChordDiagram;
use std::io::BufWriter;
use std::str::FromStr;
use web_sys::wasm_bindgen::closure::Closure;
use web_sys::wasm_bindgen::JsCast;
use web_sys::{Blob, HtmlInputElement, HtmlSelectElement, HtmlTextAreaElement};
use yew::prelude::*;

mod diagram;
mod presets;

fn new_blob(arr: &[u8], mime_type: &str) -> Blob {
    let u8_arr = unsafe { web_sys::js_sys::Uint8Array::view(arr) };
    let parts = web_sys::js_sys::Array::new();
    parts.push(&u8_arr);
    let mut options = web_sys::BlobPropertyBag::new();
    options.type_(mime_type);
    Blob::new_with_blob_sequence_and_options(&parts, &options).unwrap()
}

fn compile(input: &str, bpm: u8) -> Result<(String, Vec<MeasureRef>, Vec<u8>), String> {
    let ast = RechordImporter
        .import(&format!("{}\n", input))
        .map_err(|e| e.to_string())?;
    // AST 表示は度数表記のまま出したいので resolve する前に文字列化する
    let ast_str = format!("{}", ast);
    // resolve() も export() も self を consume するので、解決済みの AST を
    // 使い回す。MidiExporter は内部でもう一度 resolve() を呼ぶが冪等。
    let resolved = ast.resolve();
    let measures = fingering::measure_refs(&resolved).map_err(|e| e.to_string())?;
    let mut writer = BufWriter::new(Vec::new());
    MidiExporter { bpm }
        .export(&mut writer, resolved)
        .map_err(|e| e.to_string())?;
    let bytes = writer.into_inner().map_err(|e| e.to_string())?;
    Ok((ast_str, measures, bytes))
}

/// 画面のタブ。プレイヤーはタブの外に置くので、切り替えても再生は止まらない
#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Editor,
    Preview,
}

#[function_component(App)]
fn app() -> Html {
    let input = use_state(String::new);
    let bpm = use_state(|| 120u8);
    let loop_on = use_state(|| false);
    let result = use_state(String::new);
    let measures = use_state(Vec::<MeasureRef>::new);
    let tab = use_state(|| Tab::Editor);
    // select の表示値。入力テキストの scale: 指定ではなく「最後に適用したスケール」
    let scale_name = use_state(|| "major".to_string());
    let error = use_state(String::new);
    let url = use_state(String::new);
    let playing = use_mut_ref(|| false);
    let pending = use_mut_ref(|| None::<String>);
    // live mirror of the URL currently applied to the player src. Read this in
    // the once-registered stop listener, whose captured `url` UseStateHandle would
    // otherwise be a stale first-render snapshot (so its prev-URL revoke never ran).
    let applied = use_mut_ref(|| String::new());
    let player_ref = use_node_ref();
    let gutter_ref = use_node_ref();

    // input か bpm が変わるたびに再計算
    {
        let result = result.clone();
        let measures = measures.clone();
        let error = error.clone();
        let url = url.clone();
        let playing = playing.clone();
        let pending = pending.clone();
        let applied = applied.clone();
        use_effect_with(((*input).clone(), *bpm), move |(input, bpm)| {
            if input.trim().is_empty() {
                error.set(String::new());
                result.set(String::new());
                measures.set(Vec::new());
                let old_pending = pending.borrow_mut().take();
                if let Some(old) = old_pending {
                    let _ = web_sys::Url::revoke_object_url(&old);
                }
                if !*playing.borrow() {
                    let prev = applied.borrow().clone();
                    if !prev.is_empty() {
                        let _ = web_sys::Url::revoke_object_url(&prev);
                    }
                    *applied.borrow_mut() = String::new();
                    url.set(String::new());
                }
                return;
            }
            match compile(input, *bpm) {
                Ok((ast_str, ms, bytes)) => {
                    error.set(String::new());
                    result.set(ast_str);
                    measures.set(ms);
                    let blob = new_blob(&bytes, "audio/midi");
                    let object_url =
                        web_sys::Url::create_object_url_with_blob(&blob).unwrap();
                    if *playing.borrow() {
                        let old_pending = pending.borrow_mut().replace(object_url);
                        if let Some(old) = old_pending {
                            let _ = web_sys::Url::revoke_object_url(&old);
                        }
                    } else {
                        let prev = applied.borrow().clone();
                        if !prev.is_empty() {
                            let _ = web_sys::Url::revoke_object_url(&prev);
                        }
                        *applied.borrow_mut() = object_url.clone();
                        url.set(object_url);
                    }
                }
                Err(e) => {
                    error.set(e);
                    measures.set(Vec::new());
                }
            }
        });
    }

    // url / loop_on が変わるたび midi-player の属性を更新（loop は予約語なので属性直接操作）
    {
        let player_ref = player_ref.clone();
        use_effect_with(((*url).clone(), *loop_on), move |(url, loop_on)| {
            if let Some(el) = player_ref.cast::<web_sys::Element>() {
                if url.is_empty() {
                    let _ = el.remove_attribute("src");
                } else {
                    let _ = el.set_attribute("src", url);
                }
                if *loop_on {
                    let _ = el.set_attribute("loop", "");
                } else {
                    let _ = el.remove_attribute("loop");
                }
            }
        });
    }

    // プレイヤーの start / stop を監視。再生中は src を差し替えず pending に退避し、停止時に反映する
    {
        let player_ref = player_ref.clone();
        let playing = playing.clone();
        let pending = pending.clone();
        let url = url.clone();
        let applied = applied.clone();
        use_effect_with((), move |_| {
            if let Some(el) = player_ref.cast::<web_sys::Element>() {
                {
                    let playing = playing.clone();
                    let cb = Closure::<dyn FnMut()>::new(move || {
                        *playing.borrow_mut() = true;
                    });
                    let _ = el
                        .add_event_listener_with_callback("start", cb.as_ref().unchecked_ref());
                    cb.forget();
                }
                {
                    let playing = playing.clone();
                    let pending = pending.clone();
                    let url = url.clone();
                    let applied = applied.clone();
                    let cb = Closure::<dyn FnMut()>::new(move || {
                        *playing.borrow_mut() = false;
                        let next = pending.borrow_mut().take();
                        if let Some(next) = next {
                            let prev = applied.borrow().clone();
                            if !prev.is_empty() && prev != next {
                                let _ = web_sys::Url::revoke_object_url(&prev);
                            }
                            *applied.borrow_mut() = next.clone();
                            url.set(next);
                        }
                    });
                    let _ = el
                        .add_event_listener_with_callback("stop", cb.as_ref().unchecked_ref());
                    cb.forget();
                }
            }
            || ()
        });
    }

    let on_input = {
        let input = input.clone();
        Callback::from(move |e: InputEvent| {
            let ta: HtmlTextAreaElement = e.target_unchecked_into();
            input.set(ta.value());
        })
    };
    // 行番号ガターをテキストエリアのスクロールに追従させる
    let on_scroll = {
        let gutter_ref = gutter_ref.clone();
        Callback::from(move |e: Event| {
            let ta: HtmlTextAreaElement = e.target_unchecked_into();
            if let Some(gutter) = gutter_ref.cast::<web_sys::Element>() {
                gutter.set_scroll_top(ta.scroll_top());
            }
        })
    };
    let on_bpm = {
        let bpm = bpm.clone();
        Callback::from(move |e: InputEvent| {
            let el: HtmlInputElement = e.target_unchecked_into();
            if let Ok(v) = el.value().parse::<u8>() {
                bpm.set(v);
            }
        })
    };
    let on_loop = {
        let loop_on = loop_on.clone();
        Callback::from(move |e: Event| {
            let el: HtmlInputElement = e.target_unchecked_into();
            loop_on.set(el.checked());
        })
    };
    let on_key_up = {
        let input = input.clone();
        Callback::from(move |_| input.set(shift_key(input.as_str(), 1)))
    };
    let on_key_down = {
        let input = input.clone();
        Callback::from(move |_| input.set(shift_key(input.as_str(), -1)))
    };
    let on_scale = {
        let input = input.clone();
        let scale_name = scale_name.clone();
        Callback::from(move |e: Event| {
            let el: HtmlSelectElement = e.target_unchecked_into();
            if let Ok(scale) = Scale::from_str(&el.value()) {
                input.set(set_scale(input.as_str(), scale));
                scale_name.set(el.value());
            }
        })
    };

    let cards = {
        // 出現するコード名ごとに色を割り当てる。同じコードは常に同じ色
        const PALETTE: [&str; 8] = [
            "#c0392b", "#d35400", "#b7950b", "#1e8449", "#1f618d", "#6c3483", "#a93226",
            "#117864",
        ];
        let mut labels: Vec<String> = Vec::new();
        let mut rows: Vec<Html> = Vec::new();
        for measure in measures.iter() {
            let mut cards: Vec<Html> = Vec::new();
            for c in measure.chords.iter() {
                let index = match labels.iter().position(|l| l == &c.label) {
                    Some(i) => i,
                    None => {
                        labels.push(c.label.clone());
                        labels.len() - 1
                    }
                };
                cards.push(html! {
                    <ChordDiagram
                        label={c.label.clone()}
                        fingering={fingering::find(c)}
                        color={PALETTE[index % PALETTE.len()].to_string()}
                    />
                });
            }
            rows.push(html! { <div class="measure">{cards}</div> });
            // 入力が改行されている小節の後ろで、カードの並びも折り返す
            if measure.line_break {
                rows.push(html! { <div class="line-break"></div> });
            }
        }
        rows
    };

    let on_editor_tab = {
        let tab = tab.clone();
        Callback::from(move |_| tab.set(Tab::Editor))
    };
    let on_preview_tab = {
        let tab = tab.clone();
        Callback::from(move |_| tab.set(Tab::Preview))
    };

    let has_error = !(*error).is_empty();

    let editor = html! {
        <>
            <div class="editor-wrap">
                <div class="gutter" ref={gutter_ref}>
                    { for (1..=(*input).split('\n').count()).map(|n| html! {
                        <div class="gutter-line">{n}</div>
                    }) }
                </div>
                <textarea
                    class="editor"
                    rows=10
                    spellcheck="false"
                    wrap="off"
                    placeholder="key: C\nC | Am | F | G |"
                    oninput={on_input}
                    onscroll={on_scroll}
                    value={(*input).clone()}
                ></textarea>
            </div>
            if has_error {
                <p class="error">{(*error).clone()}</p>
            }

            <div class="controls">
                <div class="field">
                    <label for="bpm">{"BPM"}</label>
                    <input id="bpm" type="range" min="40" max="240"
                        style={format!("--fill: {}%", (*bpm as f32 - 40.0) / 2.0)}
                        value={bpm.to_string()} oninput={on_bpm} />
                    <output class="field-value">{*bpm}</output>
                </div>
                <div class="field">
                    <label class="switch">
                        <input type="checkbox" checked={*loop_on} onchange={on_loop} />
                        <span class="switch-track"></span>
                        <span class="switch-label">{"ループ再生"}</span>
                    </label>
                </div>
                <div class="field">
                    <label>{"キー"}</label>
                    <button class="btn btn-icon" onclick={on_key_down} title="半音下げる">{"♭"}</button>
                    <button class="btn btn-icon" onclick={on_key_up} title="半音上げる">{"♯"}</button>
                </div>
                <div class="field">
                    <label for="scale">{"スケール"}</label>
                    <select id="scale" onchange={on_scale}>
                        { for ["major","minor","dorian","phrygian","lydian","mixolydian","locrian"]
                            .iter().map(|s| html!{
                                <option value={*s} selected={*s == scale_name.as_str()}>{*s}</option>
                            }) }
                    </select>
                </div>
            </div>

            <div class="block">
                <span class="block-label">{"進行プリセット"}</span>
                <div class="presets">
                    { for presets::PRESETS.iter().map(|(name, degrees)| {
                        let input = input.clone();
                        let degrees = degrees.to_string();
                        let onclick = Callback::from(move |_| {
                            let cur = (*input).clone();
                            let next = if cur.trim().is_empty() { degrees.clone() }
                                       else { format!("{}\n{}", cur.trim_end(), degrees) };
                            input.set(next);
                        });
                        html!{ <button class="btn" {onclick}>{name}</button> }
                    }) }
                </div>
            </div>

            <div class="block">
                {
                    if (*url).is_empty() {
                        html! { <button class="btn" disabled=true>{"MIDIを保存"}</button> }
                    } else {
                        html! {
                            <a class="btn btn-primary" href={(*url).clone()}
                                download="chord.mid">{"MIDIを保存"}</a>
                        }
                    }
                }
            </div>

            <details class="ast">
                <summary>{"解釈結果"}</summary>
                <pre>{(*result).clone()}</pre>
            </details>
        </>
    };

    let preview = if cards.is_empty() {
        html! {
            <p class="empty">
                {"エディタタブでコード進行を入力すると、ここに押さえ方が出ます。"}
            </p>
        }
    } else {
        html! { <div class="fingerings">{cards}</div> }
    };

    html! {
        <>
            <header class="app-header">
                <div class="app-title">
                    <span class="app-name">{"chord-midi"}</span>
                    <span class="app-tagline">{"コード進行から MIDI とギターの押さえ方を作る"}</span>
                </div>
                // プレイヤーはタブの外。タブを切り替えても再生が止まらないようにする
                <div class="player">
                    <midi-player ref={player_ref} visualizer="#visualizer" />
                </div>
            </header>
            // 何も読み込んでいないときは高さを潰す。要素は外さない（player の参照先なので）
            <midi-visualizer type="piano-roll" id="visualizer"
                class={if (*url).is_empty() { "is-empty" } else { "" }} />

            <nav class="tabs">
                <button
                    class={classes!("tab", (*tab == Tab::Editor).then_some("is-active"))}
                    onclick={on_editor_tab}
                >
                    {"エディタ"}
                    if has_error {
                        <span class="tab-badge" title="入力にエラーがあります">{"!"}</span>
                    }
                </button>
                <button
                    class={classes!("tab", (*tab == Tab::Preview).then_some("is-active"))}
                    onclick={on_preview_tab}
                >
                    {"プレビュー"}
                </button>
            </nav>

            <main class="panel">
                if *tab == Tab::Editor { {editor} } else { {preview} }
            </main>
        </>
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
