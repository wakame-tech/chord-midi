//! 今のテキストに書かれている定義をボタンにする。
//! クリックでカーソル位置に `$name` を入れる。
//!
//! AST ではなく生テキストを見る (`textedit::define_names`)。打鍵の
//! 途中でパースが落ちていてもチップが消えないようにするため

use chord_midi::textedit::define_names;
use yew::prelude::*;

use crate::cursor::Edit;

#[derive(Properties, PartialEq)]
pub struct DefineChipsProps {
    /// 今のエディタの中身
    pub text: String,
    pub on_edit: Callback<Edit>,
}

#[function_component(DefineChips)]
pub fn define_chips(props: &DefineChipsProps) -> Html {
    // 正規表現1本の走査なので、キー入力のたびに呼んでも重くない。
    // use_memo で text をキーにして覚える選択肢もあるが、fretboard.rs の
    // identify() と同じく「どうせ毎回 props が変わる」ので割に合わない
    let names = define_names(&props.text);
    let body = if names.is_empty() {
        // 0件でもブロックは消さない。列の高さが跳ねないのと、
        // ここで記法を教えられるのが理由
        html! {
            <p class="chips-empty">{"行頭に $名前 = ... と書くと、ここに出る"}</p>
        }
    } else {
        let chips = names
            .iter()
            .map(|name| {
                let label = format!("${}", name);
                let onclick = {
                    let on_edit = props.on_edit.clone();
                    let label = label.clone();
                    Callback::from(move |_| on_edit.emit(Edit::Insert(label.clone())))
                };
                html! { <button class="btn chip" {onclick}>{label}</button> }
            })
            .collect::<Html>();
        html! { <div class="chips">{chips}</div> }
    };
    html! {
        <div class="block">
            <span class="block-label">{"定義 (カーソル位置に挿入)"}</span>
            {body}
        </div>
    }
}
