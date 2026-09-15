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

#[function_component(App)]
fn app() -> Html {
    let input = use_state(String::new);
    let bpm = use_state(|| 120u8);
    let loop_on = use_state(|| false);
    let result = use_state(String::new);
    let measures = use_state(Vec::<MeasureRef>::new);
    let error = use_state(String::new);
    let url = use_state(String::new);
    let playing = use_mut_ref(|| false);
    let pending = use_mut_ref(|| None::<String>);
    // live mirror of the URL currently applied to the player src. Read this in
    // the once-registered stop listener, whose captured `url` UseStateHandle would
    // otherwise be a stale first-render snapshot (so its prev-URL revoke never ran).
    let applied = use_mut_ref(|| String::new());
    let player_ref = use_node_ref();

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
        Callback::from(move |e: Event| {
            let el: HtmlSelectElement = e.target_unchecked_into();
            if let Ok(scale) = Scale::from_str(&el.value()) {
                input.set(set_scale(input.as_str(), scale));
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

    html! {
        <div>
            <h1>{"chord_midi_web"}</h1>
            <textarea rows=8 cols=50 oninput={on_input}
                value={(*input).clone()}></textarea>
            <div>
                <label>{format!("BPM: {}", *bpm)}</label>
                <input type="range" min="40" max="240" value={bpm.to_string()}
                    oninput={on_bpm} />
            </div>
            <div>
                <label>
                    <input type="checkbox" checked={*loop_on} onchange={on_loop} />
                    {"loop"}
                </label>
            </div>
            <div>
                <button onclick={on_key_down}>{"♭ key"}</button>
                <button onclick={on_key_up}>{"key ♯"}</button>
            </div>
            <div>
                <label>{"scale: "}</label>
                <select onchange={on_scale}>
                    { for ["major","minor","dorian","phrygian","lydian","mixolydian","locrian"]
                        .iter().map(|s| html!{ <option value={*s}>{*s}</option> }) }
                </select>
            </div>
            <div>
                { for presets::PRESETS.iter().map(|(name, degrees)| {
                    let input = input.clone();
                    let degrees = degrees.to_string();
                    let onclick = Callback::from(move |_| {
                        let cur = (*input).clone();
                        let next = if cur.trim().is_empty() { degrees.clone() }
                                   else { format!("{}\n{}", cur.trim_end(), degrees) };
                        input.set(next);
                    });
                    html!{ <button {onclick}>{name}</button> }
                }) }
            </div>
            <div class="fingerings">{cards}</div>
            <div>
                {
                    if (*url).is_empty() {
                        html! { <button disabled=true>{"MIDIを保存"}</button> }
                    } else {
                        html! {
                            <a href={(*url).clone()} download="chord.mid">{"MIDIを保存"}</a>
                        }
                    }
                }
            </div>
            <p style="color: red">{(*error).clone()}</p>
            <pre>{(*result).clone()}</pre>

            <midi-player ref={player_ref} visualizer="#visualizer" />
            <midi-visualizer type="piano-roll" id="visualizer" />
        </div>
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
