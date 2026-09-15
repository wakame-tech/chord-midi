//! 記法の一覧表。CLI の `--help-syntax` と同じ
//! `chord_midi::reference` のテーブルから作る。
//!
//! 既定で開いておく (常設表示) が、行数が多いので畳めるようにする。
//! パネル自体が `overflow-y: auto` なので、開いたまま使える。
//!
//! 各行の例はクリックでカーソル位置に挿入できる。記法パネルの他の部品
//! (`fretboard.rs` の候補カードなど) と同じく、`Edit` を投げるだけで
//! テキストの持ち主である `App` には触らない。

use chord_midi::reference::{Function, Symbol, BEATS, FUNCTIONS, SYMBOLS};
use yew::prelude::*;

use crate::cursor::Edit;

/// 例のセル。クリックでカーソル位置に挿入する
fn example_cell(example: &'static str, on_edit: &Callback<Edit>) -> Html {
    let onclick = {
        let on_edit = on_edit.clone();
        Callback::from(move |_| on_edit.emit(Edit::Insert(example.to_string())))
    };
    html! {
        <td>
            <code class="syntax-example"
                title="クリックしてカーソル位置に挿入" {onclick}>
                {example}
            </code>
        </td>
    }
}

fn symbol_table(rows: &[Symbol], on_edit: &Callback<Edit>) -> Html {
    html! {
        <table class="syntax-table">
            { for rows.iter().map(|s| html! {
                <tr>
                    <td><code>{s.symbol}</code></td>
                    <td>{s.summary}</td>
                    { example_cell(s.example, on_edit) }
                </tr>
            }) }
        </table>
    }
}

fn function_table(rows: &[Function], on_edit: &Callback<Edit>) -> Html {
    html! {
        <table class="syntax-table">
            { for rows.iter().map(|f| html! {
                <tr>
                    <td><code>{format!("{}({})", f.name, f.args)}</code></td>
                    // 小節列に掛かるか、コード1つに掛かるか。この区別が
                    // 崩れると「書けると思ったのに書けない」が起きる
                    <td>{f.scope.label()}</td>
                    <td>{f.summary}</td>
                    { example_cell(f.example, on_edit) }
                </tr>
            }) }
        </table>
    }
}

#[derive(Properties, PartialEq)]
pub struct SyntaxReferenceProps {
    /// 記法パネルの他の部品と共通。例をクリックしたときにカーソル位置へ挿入する
    pub on_edit: Callback<Edit>,
}

#[function_component(SyntaxReference)]
pub fn syntax_reference(props: &SyntaxReferenceProps) -> Html {
    html! {
        <details class="syntax-ref" open=true>
            <summary>{"記法の一覧"}</summary>
            <div class="block">
                <span class="block-label">{"記号"}</span>
                { symbol_table(SYMBOLS, &props.on_edit) }
            </div>
            <div class="block">
                <span class="block-label">{"リズムの文字 (pat の第1引数)"}</span>
                { symbol_table(BEATS, &props.on_edit) }
            </div>
            <div class="block">
                <span class="block-label">{"関数 (単位: 小節列 / コード)"}</span>
                { function_table(FUNCTIONS, &props.on_edit) }
            </div>
        </details>
    }
}
