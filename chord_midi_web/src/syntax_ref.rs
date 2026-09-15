//! 記法の一覧表。CLI の `--help-syntax` と同じ
//! `chord_midi::reference` のテーブルから作る。
//!
//! 既定で開いておく (常設表示) が、行数が多いので畳めるようにする。
//! パネル自体が `overflow-y: auto` なので、開いたまま使える。
//!
//! 各行の例はクリックでカーソル位置に挿入できる。記法パネルの他の部品
//! (`fretboard.rs` の候補カードなど) と同じく、`Edit` を投げるだけで
//! テキストの持ち主である `App` には触らない。

use chord_midi::reference::{Function, Symbol, BEATS, FUNCTIONS, NESTING, SYMBOLS};
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

/// 1項目を縦に積む。幅 280px のパネルに3列も4列も入らず、
/// `.syntax-table code` が `white-space: nowrap` なので、シグネチャの長い
/// 関数と入れ子を表にすると説明列が1文字ずつ折り返して読めなくなる。
/// 記号とリズムの文字は記号列が短いので表のままでよい
fn stacked_entry(
    signature: String,
    kind: Option<&'static str>,
    summary: &'static str,
    example: &'static str,
    on_edit: &Callback<Edit>,
) -> Html {
    let onclick = {
        let on_edit = on_edit.clone();
        Callback::from(move |_| on_edit.emit(Edit::Insert(example.to_string())))
    };
    html! {
        <div class="syntax-entry">
            <div class="syntax-entry-head">
                <code>{signature}</code>
                {
                    match kind {
                        // 何を受けて何を返すか。返すものが「コード」の関数だけが
                        // 他の関数の中に書ける。この区別が崩れると
                        // 「書けると思ったのに書けない」が起きる
                        Some(k) => html! { <span class="syntax-kind">{k}</span> },
                        None => html! {},
                    }
                }
            </div>
            <div class="syntax-entry-summary">{summary}</div>
            <code class="syntax-example" title="クリックしてカーソル位置に挿入" {onclick}>
                {example}
            </code>
        </div>
    }
}

fn function_list(rows: &[Function], on_edit: &Callback<Edit>) -> Html {
    html! {
        <div class="syntax-list">
            { for rows.iter().map(|f| stacked_entry(
                format!("{}({})", f.name, f.args),
                Some(f.scope.label()),
                f.summary,
                f.example,
                on_edit,
            )) }
        </div>
    }
}

/// 入れ子は `Symbol` だが、シグネチャが長いので関数と同じく縦に積む
fn nesting_list(rows: &[Symbol], on_edit: &Callback<Edit>) -> Html {
    html! {
        <div class="syntax-list">
            { for rows.iter().map(|s| stacked_entry(
                s.symbol.to_string(),
                None,
                s.summary,
                s.example,
                on_edit,
            )) }
        </div>
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
                <span class="block-label">{"関数 (引数 -> 返すもの)"}</span>
                { function_list(FUNCTIONS, &props.on_edit) }
            </div>
            <div class="block">
                <span class="block-label">
                    {"入れ子 (コードを返す関数だけが他の関数の中に書ける)"}
                </span>
                { nesting_list(NESTING, &props.on_edit) }
            </div>
        </details>
    }
}
