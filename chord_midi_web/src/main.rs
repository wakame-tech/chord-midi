use chord_midi::{
    edit::{set_scale, shift_key},
    export::{Exporter, MidiExporter},
    fingering::{self, MeasureRef},
    import::{Importer, RechordImporter},
    lint::lint,
    model::scale::Scale,
    presets,
};
use cursor::Edit;
use diagram::ChordDiagram;
use fretboard::Fretboard;
use std::io::BufWriter;
use std::str::FromStr;
use web_sys::wasm_bindgen::closure::Closure;
use web_sys::wasm_bindgen::JsCast;
use web_sys::{Blob, HtmlInputElement, HtmlSelectElement, HtmlTextAreaElement};
use yew::prelude::*;

mod cursor;
mod diagram;
mod fretboard;
mod rhythm;

fn new_blob(arr: &[u8], mime_type: &str) -> Blob {
    let u8_arr = unsafe { web_sys::js_sys::Uint8Array::view(arr) };
    let parts = web_sys::js_sys::Array::new();
    parts.push(&u8_arr);
    let mut options = web_sys::BlobPropertyBag::new();
    options.type_(mime_type);
    Blob::new_with_blob_sequence_and_options(&parts, &options).unwrap()
}

/// 「解釈結果」に出す3つの形。CLI の `--print-ast` と同じ区分
#[derive(Clone, Copy, PartialEq)]
enum AstForm {
    Syntax,
    Expanded,
    Text,
}

/// 解釈結果の3形。どれも文字列なので、表示側は選ぶだけ
#[derive(Clone, Default, PartialEq)]
struct AstViews {
    syntax: String,
    expanded: String,
    text: String,
}

/// 記法エラーではなく、読みにくさへの助言。表示はエラーと別スタイルにし、
/// 出ていても再生は止めない
type Warnings = Vec<String>;

/// 入力の末尾に1行足す。空なら line だけ、それ以外は改行してから足す。
/// 進行プリセットのボタンと運指タブの挿入、両方が使う共通の結合規則
fn append_line(cur: &str, line: &str) -> String {
    if cur.trim().is_empty() {
        line.to_string()
    } else {
        format!("{}\n{}", cur.trim_end(), line)
    }
}

fn compile(input: &str, bpm: u8) -> Result<(AstViews, Vec<MeasureRef>, Vec<u8>, Warnings), String> {
    let ast = RechordImporter
        .import(&format!("{}\n", input))
        .map_err(|e| e.to_string())?;
    // パース直後 (resolve/展開前) の木を見る。lint 自体は AST も MIDI も
    // 変えないので、ここで見ても resolve 後の結果には影響しない
    let warnings = lint(&ast);
    // テキスト表示は度数表記のまま出したいので resolve する前に文字列化する
    let text = format!("{}", ast);
    let syntax = serde_json::to_string_pretty(&ast).map_err(|e| e.to_string())?;
    // resolve() も export() も self を consume するので、解決済みの AST を
    // 使い回す。MidiExporter は内部でもう一度 resolve() を呼ぶが冪等。
    let resolved = ast.resolve().map_err(|e| e.to_string())?;
    let expanded = serde_json::to_string_pretty(&resolved).map_err(|e| e.to_string())?;
    let measures = fingering::measure_refs(&resolved).map_err(|e| e.to_string())?;
    let mut writer = BufWriter::new(Vec::new());
    MidiExporter { bpm }
        .export(&mut writer, resolved)
        .map_err(|e| e.to_string())?;
    let bytes = writer.into_inner().map_err(|e| e.to_string())?;
    Ok((
        AstViews {
            syntax,
            expanded,
            text,
        },
        measures,
        bytes,
        warnings,
    ))
}

/// 画面のタブ。プレイヤーはタブの外に置くので、切り替えても再生は止まらない
#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Editor,
    Preview,
    Fretboard,
}

#[function_component(App)]
fn app() -> Html {
    let input = use_state(String::new);
    let bpm = use_state(|| 120u8);
    let loop_on = use_state(|| false);
    let result = use_state(AstViews::default);
    let ast_form = use_state(|| AstForm::Syntax);
    let measures = use_state(Vec::<MeasureRef>::new);
    let tab = use_state(|| Tab::Editor);
    // 運指タブで押さえた形。タブを離れても消さないので App が持つ
    let held = use_state(|| [None::<u8>; 6]);
    // select の表示値。入力テキストの scale: 指定ではなく「最後に適用したスケール」
    let scale_name = use_state(|| "major".to_string());
    let error = use_state(String::new);
    // lint の警告。エラーと違って再生は止めないので、MIDI 生成の成否とは別に持つ
    let warning = use_state(String::new);
    let url = use_state(String::new);
    let playing = use_mut_ref(|| false);
    let pending = use_mut_ref(|| None::<String>);
    // live mirror of the URL currently applied to the player src. Read this in
    // the once-registered stop listener, whose captured `url` UseStateHandle would
    // otherwise be a stale first-render snapshot (so its prev-URL revoke never ran).
    let applied = use_mut_ref(|| String::new());
    let player_ref = use_node_ref();
    let gutter_ref = use_node_ref();
    let editor_ref = use_node_ref();
    // 挿入の後にカーソルを戻す位置 (UTF-16)。Yew が textarea の value を
    // 書き戻すとカーソルが末尾へ飛ぶので、描画の後に当て直す
    let caret = use_state(|| None::<u32>);

    // input か bpm が変わるたびに再計算
    {
        let result = result.clone();
        let measures = measures.clone();
        let error = error.clone();
        let warning = warning.clone();
        let url = url.clone();
        let playing = playing.clone();
        let pending = pending.clone();
        let applied = applied.clone();
        use_effect_with(((*input).clone(), *bpm), move |(input, bpm)| {
            if input.trim().is_empty() {
                error.set(String::new());
                warning.set(String::new());
                result.set(AstViews::default());
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
                Ok((views, ms, bytes, warnings)) => {
                    error.set(String::new());
                    warning.set(warnings.join("\n"));
                    result.set(views);
                    measures.set(ms);
                    let blob = new_blob(&bytes, "audio/midi");
                    let object_url = web_sys::Url::create_object_url_with_blob(&blob).unwrap();
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
                    warning.set(String::new());
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
                // visualizer 属性を付け直して紐付けをやり直す。Yew はツリーを
                // 組み立てる途中でこの属性をセットするので、その時点では
                // フッタの #visualizer がまだ document に無く解決に失敗する
                let _ = el.remove_attribute("visualizer");
                let _ = el.set_attribute("visualizer", "#visualizer");
                {
                    let playing = playing.clone();
                    let cb = Closure::<dyn FnMut()>::new(move || {
                        *playing.borrow_mut() = true;
                    });
                    let _ =
                        el.add_event_listener_with_callback("start", cb.as_ref().unchecked_ref());
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
                    let _ =
                        el.add_event_listener_with_callback("stop", cb.as_ref().unchecked_ref());
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
            "#c0392b", "#d35400", "#b7950b", "#1e8449", "#1f618d", "#6c3483", "#a93226", "#117864",
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
            // pat() が無い小節でも枠は残す。空けておかないとカードの上端が揃わない
            let rhythm = match &measure.pattern {
                Some(p) => html! { <div class="rhythm">{ p.to_string() }</div> },
                None => html! { <div class="rhythm"></div> },
            };
            rows.push(html! {
                <div class="measure">
                    { rhythm }
                    <div class="measure-cards">{cards}</div>
                </div>
            });
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
    let on_fretboard_tab = {
        let tab = tab.clone();
        Callback::from(move |_| tab.set(Tab::Fretboard))
    };
    let on_held_change = {
        let held = held.clone();
        Callback::from(move |next: [Option<u8>; 6]| held.set(next))
    };
    // 記法をエディタ末尾に足す。改行結合の規則は進行プリセットと共通
    // (append_line)。挿入するのは1小節ぶんの記法なので ` |` は呼び出し側で足す
    let on_insert = {
        let input = input.clone();
        Callback::from(move |notation: String| {
            let line = format!("{} |", notation);
            input.set(append_line(&input, &line));
        })
    };

    // 記法パネルの全部品がここを通る。テキストの持ち主は input なので、
    // 部品は「何をしたいか」だけを投げてくる
    let on_edit = {
        let input = input.clone();
        let caret = caret.clone();
        let editor_ref = editor_ref.clone();
        Callback::from(move |edit: Edit| {
            let Some(ta) = editor_ref.cast::<HtmlTextAreaElement>() else {
                return;
            };
            let (text, pos) = cursor::insert_at_cursor(&ta, input.as_str(), &edit);
            // 2つの set は同じコールバックの中なので、次の描画には
            // どちらも反映される。effect はその描画の後に走る
            input.set(text);
            caret.set(Some(pos));
        })
    };

    // 描画が終わってからカーソルを戻す。None に戻しておくのは、次に
    // 同じ位置へ挿入したときも dep が変わって effect が走るようにするため
    {
        let editor_ref = editor_ref.clone();
        let caret = caret.clone();
        // dep は closure に caret を move する前に読んでおく
        let dep = *caret;
        use_effect_with(dep, move |pos| {
            if let (Some(pos), Some(ta)) = (*pos, editor_ref.cast::<HtmlTextAreaElement>()) {
                let _ = ta.focus();
                let _ = ta.set_selection_range(pos, pos);
                caret.set(None);
            }
        });
    }

    let has_error = !(*error).is_empty();
    let has_warning = !(*warning).is_empty();

    let editor = html! {
        <div class="editor-tab">
            <div class="editor-main">
                <div class="editor-wrap">
                    <div class="gutter" ref={gutter_ref}>
                        { for (1..=(*input).split('\n').count()).map(|n| html! {
                            <div class="gutter-line">{n}</div>
                        }) }
                    </div>
                    <textarea
                        class="editor"
                        ref={editor_ref.clone()}
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
                if has_warning {
                    <p class="warning">{(*warning).clone()}</p>
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
                    <span class="block-label">{"進行プリセット (末尾に追記)"}</span>
                    <div class="presets">
                        { for presets::PRESETS.iter().map(|preset| {
                            let input = input.clone();
                            let progression = preset.progression.to_string();
                            let onclick = Callback::from(move |_| {
                                input.set(append_line(&input, &progression));
                            });
                            html!{
                                <button class="btn preset" {onclick}>
                                    <span class="preset-name">{preset.name}</span>
                                    <span class="preset-degrees">{preset.degrees}</span>
                                </button>
                            }
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
                    <div class="ast-forms">
                        { for [
                            (AstForm::Syntax, "構文"),
                            (AstForm::Expanded, "展開後"),
                            (AstForm::Text, "テキスト"),
                        ].into_iter().map(|(form, label)| {
                            let ast_form = ast_form.clone();
                            let is_active = *ast_form == form;
                            let onclick = Callback::from(move |_| ast_form.set(form));
                            html! {
                                <button
                                    class={classes!("btn", "btn-icon",
                                        is_active.then_some("is-active"))}
                                    {onclick}
                                >{label}</button>
                            }
                        }) }
                    </div>
                    <pre>{
                        match *ast_form {
                            AstForm::Syntax => result.syntax.clone(),
                            AstForm::Expanded => result.expanded.clone(),
                            AstForm::Text => result.text.clone(),
                        }
                    }</pre>
                </details>
            </div>
            <details class="side-panel" open=true>
                <summary>{"記法パネル"}</summary>
                <div class="side-panel-body">
                </div>
            </details>
        </div>
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

    let fretboard = html! {
        <Fretboard frets={*held} on_change={on_held_change} on_insert={on_insert} />
    };

    html! {
        <>
            <header class="app-header">
                <div class="app-title">
                    <span class="app-name">{"chord-midi"}</span>
                    <span class="app-tagline">{"コード進行から MIDI とギターの押さえ方を作る"}</span>
                </div>
            </header>

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
                <button
                    class={classes!("tab", (*tab == Tab::Fretboard).then_some("is-active"))}
                    onclick={on_fretboard_tab}
                >
                    {"運指"}
                </button>
            </nav>

            <main class="panel">
                {
                    match *tab {
                        Tab::Editor => editor,
                        Tab::Preview => preview,
                        Tab::Fretboard => fretboard,
                    }
                }
            </main>

            // プレイヤーはタブの外に固定で置く。タブを切り替えても再生は止まらない
            <footer class="app-footer">
                // 何も読み込んでいないときは高さを潰す。要素は外さない（player の参照先なので）
                <midi-visualizer type="piano-roll" id="visualizer"
                    class={if (*url).is_empty() { "is-empty" } else { "" }} />
                <div class="footer-inner">
                    <midi-player ref={player_ref} visualizer="#visualizer" />
                </div>
            </footer>
        </>
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
