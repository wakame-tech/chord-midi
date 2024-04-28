use chord_midi::{
    export::{Exporter, MidiExporter},
    import::{Importer, RechordImporter},
};
use std::{io::BufWriter, ops::Deref};
use web_sys::{
    wasm_bindgen::{JsCast, JsValue},
    Blob,
};
use yew::prelude::*;

fn get_element_by_id(id: &str) -> web_sys::Element {
    web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .get_element_by_id(id)
        .unwrap()
}

fn new_blob(arr: &[u8], mime_type: &str) -> Blob {
    // new Blob([u8arr], {type: 'audio/midi'})
    let u8_arr = unsafe { web_sys::js_sys::Uint8Array::view(arr) };
    let parts = web_sys::js_sys::Array::new();
    parts.push(&u8_arr);
    let mut options = web_sys::BlobPropertyBag::new();
    options.type_(mime_type);
    Blob::new_with_blob_sequence_and_options(&parts, &options).unwrap()
}

#[function_component(App)]
fn app() -> Html {
    let result_state = use_state(|| String::new());
    let url_state = use_state(|| String::new());
    let error_state = use_state(|| String::new());

    let on_button_click = use_callback(
        [result_state.clone(), url_state.clone(), error_state.clone()],
        |_, [result, url, error_state]| {
            let input = get_element_by_id("input")
                .dyn_into::<web_sys::HtmlTextAreaElement>()
                .unwrap()
                .value()
                + "\n";
            web_sys::console::log_1(&JsValue::from(format!("{}", input)));
            let res = RechordImporter.import(&input);
            if let Err(err) = res {
                error_state.set(format!("{}", err));
                web_sys::console::log_1(&JsValue::from(format!("{}", err)));
                return;
            }
            let ast = res.unwrap();
            result.set(format!("{}", ast));

            let mut writer = BufWriter::new(Vec::new());
            MidiExporter { bpm: 120 }.export(&mut writer, ast).unwrap();

            let blob = new_blob(writer.buffer(), "audio/midi");
            let object_url = web_sys::Url::create_object_url_with_blob(&blob).unwrap();
            url.set(object_url);
        },
    );

    html! {
        <div>
            <h1>{"chord_midi_web"}</h1>
            <textarea rows=8 cols=50 id="input"></textarea>
            <button onclick={on_button_click}>{"submit"}</button>
            <p style="color: red">{error_state.deref()}</p>
            <p>{result_state.deref()}</p>

            <midi-player
                src={url_state.deref().to_string()} visualizer="#visualizer"
            />
            <midi-visualizer type="piano-roll" id="visualizer"/>
        </div>
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
