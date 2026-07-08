use chord_midi::{
    export::{Exporter, MidiExporter},
    import::{Importer, RechordImporter},
};
use std::io::BufWriter;
use web_sys::{Blob, HtmlInputElement, HtmlTextAreaElement};
use yew::prelude::*;

fn new_blob(arr: &[u8], mime_type: &str) -> Blob {
    let u8_arr = unsafe { web_sys::js_sys::Uint8Array::view(arr) };
    let parts = web_sys::js_sys::Array::new();
    parts.push(&u8_arr);
    let mut options = web_sys::BlobPropertyBag::new();
    options.type_(mime_type);
    Blob::new_with_blob_sequence_and_options(&parts, &options).unwrap()
}

fn compile(input: &str, bpm: u8) -> Result<(String, Vec<u8>), String> {
    let ast = RechordImporter
        .import(&format!("{}\n", input))
        .map_err(|e| e.to_string())?;
    let ast_str = format!("{}", ast);
    let mut writer = BufWriter::new(Vec::new());
    MidiExporter { bpm }
        .export(&mut writer, ast)
        .map_err(|e| e.to_string())?;
    let bytes = writer.into_inner().map_err(|e| e.to_string())?;
    Ok((ast_str, bytes))
}

#[function_component(App)]
fn app() -> Html {
    let input = use_state(String::new);
    let bpm = use_state(|| 120u8);
    let loop_on = use_state(|| false);
    let result = use_state(String::new);
    let error = use_state(String::new);
    let url = use_state(String::new);
    let player_ref = use_node_ref();

    // input か bpm が変わるたびに再計算
    {
        let result = result.clone();
        let error = error.clone();
        let url = url.clone();
        use_effect_with(((*input).clone(), *bpm), move |(input, bpm)| {
            if input.trim().is_empty() {
                error.set(String::new());
                result.set(String::new());
                let prev = (*url).clone();
                if !prev.is_empty() {
                    let _ = web_sys::Url::revoke_object_url(&prev);
                }
                url.set(String::new());
                return;
            }
            match compile(input, *bpm) {
                Ok((ast_str, bytes)) => {
                    error.set(String::new());
                    result.set(ast_str);
                    let blob = new_blob(&bytes, "audio/midi");
                    let object_url =
                        web_sys::Url::create_object_url_with_blob(&blob).unwrap();
                    let prev = (*url).clone();
                    if !prev.is_empty() {
                        let _ = web_sys::Url::revoke_object_url(&prev);
                    }
                    url.set(object_url);
                }
                Err(e) => error.set(e),
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
