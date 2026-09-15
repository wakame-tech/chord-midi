//! 関数ボタン。選択範囲があれば包み、無ければ雛形を入れる。
//!
//! テキストは持たない。`fretboard.rs` と同じく、やりたいことを
//! `Callback` で親に投げるだけ

use chord_midi::reference::{Scope, FUNCTIONS};
use yew::prelude::*;

use crate::cursor::Edit;

#[derive(Properties, PartialEq)]
pub struct FuncPaletteProps {
    pub on_edit: Callback<Edit>,
}

/// 関数1つぶんの編集要求。`Edit::Wrap` は選択の有無で振る舞いが変わる
/// (選択があれば包む / 無ければ挿入して括弧の内側にカーソル) ので、
/// 設計書の「選択があれば包む、無ければ雛形」はこれ1つで足りる
fn wrap_for(name: &str) -> Edit {
    if name == "pat" {
        // pat だけは第1引数が要る。8分の頭打ちを雛形にする
        Edit::Wrap("pat(\"o.o.o.o.\", ".to_string(), ")".to_string())
    } else {
        Edit::Wrap(format!("{}(", name), ")".to_string())
    }
}

fn buttons(scope: Scope, on_edit: &Callback<Edit>) -> Html {
    FUNCTIONS
        .iter()
        .filter(|f| f.scope == scope)
        .map(|f| {
            let on_edit = on_edit.clone();
            let edit = wrap_for(f.name);
            let onclick = Callback::from(move |_| on_edit.emit(edit.clone()));
            html! {
                <button class="btn" {onclick}
                    title={format!("{}  例: {}", f.summary, f.example)}>
                    {f.name}
                </button>
            }
        })
        .collect::<Html>()
}

#[function_component(FuncPalette)]
pub fn func_palette(props: &FuncPaletteProps) -> Html {
    // `( )` と `xN` は関数ではないが、「選択を包む」操作は同じなので並べる
    let on_group = {
        let on_edit = props.on_edit.clone();
        Callback::from(move |_| on_edit.emit(Edit::Wrap("(".to_string(), ")".to_string())))
    };
    let on_repeat = {
        let on_edit = props.on_edit.clone();
        Callback::from(move |_| on_edit.emit(Edit::Wrap("(".to_string(), ") x2".to_string())))
    };
    html! {
        <>
            <div class="block">
                <span class="block-label">{"小節列に掛ける"}</span>
                <div class="palette">{ buttons(Scope::Measures, &props.on_edit) }</div>
            </div>
            <div class="block">
                <span class="block-label">{"コードに掛ける"}</span>
                <div class="palette">{ buttons(Scope::Chord, &props.on_edit) }</div>
            </div>
            <div class="block">
                <span class="block-label">{"まとめ・繰り返し"}</span>
                <div class="palette">
                    <button class="btn" onclick={on_group}
                        title="選んだ範囲を ( ) でまとめる">{"( )"}</button>
                    <button class="btn" onclick={on_repeat}
                        title="選んだ範囲を ( ) x2 で包む">{"( ) x2"}</button>
                </div>
            </div>
        </>
    }
}
