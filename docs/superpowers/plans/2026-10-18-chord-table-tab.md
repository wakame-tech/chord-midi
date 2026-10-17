# コード表タブ Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 12ルート × 11品質のボタンを押すと、その場の指板に押さえ方が出る「コード表」タブを足す。

**Architecture:** 新しい探索ロジックは書かない。ボタンのラベルからコード名の文字列を組み、`RechordImporter` → `resolve` → `chord_refs` → `fingering::shape_of` の既存経路を通す。指板のマス目を描く `board()` に表示専用モードを足して使い回す。

**Tech Stack:** Rust 2021 / yew 0.21 (csr) / trunk

**設計書:** `docs/superpowers/specs/2026-10-18-chord-table-tab-design.md`

---

## 前提

- 作業ブランチは `feat/chord-table-tab` (`feat/tab-notation` の上に stack。`fingering::shape_of` に依存する)
- web は workspace 外。ビルドは `cd chord_midi_web && cargo check --target wasm32-unknown-unknown`
- 画面の確認は `cd chord_midi_web && trunk serve --port 8099 --no-autoreload`
- 整形は各クレートで `cargo fmt --all`

## ファイル構成

| ファイル | 責務 | 変更 |
|---|---|---|
| `chord_midi_web/src/fretboard.rs` | 指板のマス目 | `board()` を表示専用でも呼べるようにして `pub(crate)` にする |
| `chord_midi_web/src/chord_table.rs` | **新規**。コード表タブの中身 (格子ボタン + 表示専用の指板) | 新しいコンポーネント |
| `chord_midi_web/src/main.rs` | 画面 | `Tab::ChordTable` を足してタブに並べる |
| `chord_midi_web/style.css` | 見た目 | 格子ボタンのスタイル |

---

### Task 1: `board()` を表示専用でも呼べるようにする

**Files:**
- Modify: `chord_midi_web/src/fretboard.rs:55` (`board` の引数とマスの描画)

表示専用のときはマスを `<button>` ではなく `<div>` で描く。押せる見た目にしないため。
`readonly: bool` と `on_change` を別々に渡すと「読み取り専用なのにコールバックがある」
矛盾した組み合わせが型の上で作れてしまうので、`Option` 1つにまとめる。

- [ ] **Step 1: `board` の引数を `Option` にする**

`chord_midi_web/src/fretboard.rs` の `fn board` のシグネチャを置き換える:

```rust
/// 指板のマス目を描く。
///
/// `on_change` が `None` なら表示専用で、マスを `<button>` ではなく
/// `<div>` で描く。コード表タブは押さえ方を見るだけなので、押せる
/// 見た目にしない
pub(crate) fn board(frets: [Option<u8>; 6], on_change: Option<&Callback<[Option<u8>; 6]>>) -> Html {
```

- [ ] **Step 2: マスの描画を分岐させる**

同じ関数の中、`let cells = (0..=MAX_FRET)` から `.collect::<Html>();` までを
次に置き換える:

```rust
            let cells = (0..=MAX_FRET)
                .map(|fret| {
                    let on = frets[s] == Some(fret);
                    // 0列は開放弦。ナットの左側なので弦だけ描いてフレット線は描かない。
                    // ナットを0の右ではなく1の左に引くのは、0列にはそもそもフレット線を
                    // 描かないので引く場所が無いため
                    let class = classes!(
                        "fretboard-cell",
                        (fret == 0).then_some("is-open"),
                        (fret == 1).then_some("is-nut"),
                    );
                    let dot = on.then(|| html! { <span class="fretboard-dot"></span> });
                    let Some(on_change) = on_change else {
                        // 表示専用。押せないので button にしない
                        return html! { <div {class}>{dot}</div> };
                    };
                    let onclick = {
                        let on_change = on_change.clone();
                        Callback::from(move |_| {
                            let mut next = frets;
                            // 同じところをもう一度押したらその弦をミュートにする
                            next[s] = if next[s] == Some(fret) {
                                None
                            } else {
                                Some(fret)
                            };
                            on_change.emit(next);
                        })
                    };
                    // キーボードでも押せるよう実ボタンにする。弦とフレットは
                    // 見た目のグリッド位置でしか区別できないので aria-label で言葉にする
                    html! {
                        <button type="button" {class} {onclick}
                            aria-label={format!("{}弦 {}フレット", 6 - s, fret)}
                            aria-pressed={on.to_string()}>
                            {dot}
                        </button>
                    }
                })
                .collect::<Html>();
```

- [ ] **Step 3: 呼び出し側を直す**

同じファイルの `fretboard` コンポーネントの中:

```rust
            { board(props.frets, Some(&props.on_change)) }
```

- [ ] **Step 4: コンパイルを通す**

Run: `cd /Users/kmt/dev/chord-midi/chord_midi_web && cargo check --target wasm32-unknown-unknown 2>&1 | tail -5`
Expected: エラー無し

- [ ] **Step 5: エディタタブの指板が今まで通り動くことを確かめる**

```bash
cd /Users/kmt/dev/chord-midi/chord_midi_web
trunk serve --port 8099 --no-autoreload
```
`http://127.0.0.1:8099/` で指板のマスを3つ押し、候補が出ることを見る。
確認したら `Ctrl-C` で止める。

- [ ] **Step 6: 整形してコミット**

```bash
cd /Users/kmt/dev/chord-midi/chord_midi_web
cargo fmt --all
cargo check -q --target wasm32-unknown-unknown
cd /Users/kmt/dev/chord-midi
git add -A
git commit -m "$(cat <<'MSG'
refactor(web): 指板のマス目を表示専用でも描けるようにする

on_change が None なら <button> ではなく <div> で描く。readonly の
フラグと on_change を別々に渡すと、読み取り専用なのにコールバックが
ある組み合わせが型の上で作れてしまうので Option 1つにまとめた。

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01F28qZZi3uLGGKV2azgKPX8
MSG
)"
```

---

### Task 2: コード名から押さえ方を引く関数

**Files:**
- Create: `chord_midi_web/src/chord_table.rs`
- Modify: `chord_midi_web/src/main.rs` (`mod chord_table;` を足す)

格子の定義と、コード名1つから `Shape` を引く関数だけを先に作る。
画面はまだ出さない。

- [ ] **Step 1: ファイルを作る**

`chord_midi_web/src/chord_table.rs`:

```rust
//! コード表タブ。ルート × 品質の格子ボタンを押すと、下の指板に
//! その押さえ方が出る。
//!
//! 押さえ方の探索は自前で持たない。コード名の文字列を組んで
//! `RechordImporter` → `resolve` → `chord_refs` → `shape_of` の
//! 既存経路をそのまま通す

use chord_midi::fingering::{chord_refs, shape_of, Shape};
use chord_midi::import::{Importer, RechordImporter};
use yew::prelude::*;

use crate::fretboard::board;

/// 格子の行。記法のルート名をそのまま使う
pub const ROOTS: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

/// 格子の列。`(見出し, 記法の接尾辞)`。メジャーは接尾辞が無いので
/// 見出しだけ `(maj)` と書く
pub const QUALITIES: [(&str, &str); 11] = [
    ("(maj)", ""),
    ("m", "m"),
    ("7", "7"),
    ("m7", "m7"),
    ("M7", "M7"),
    ("m7b5", "m7b5"),
    ("sus4", "sus4"),
    ("6", "6"),
    ("add9", "add9"),
    ("dim", "dim"),
    ("aug", "aug"),
];

/// コード名1つの押さえ方を引く。読めない名前や形が無い組み合わせは None。
///
/// 1小節だけの進行として読ませる。`resolve` を通すのは、度数表記が
/// 混ざっていなくても `chord_refs` が解決済みの木を要求するため
pub fn shape_for(name: &str) -> Option<Shape> {
    let ast = RechordImporter.import(&format!("{} |\n", name)).ok()?;
    let resolved = ast.resolve().ok()?;
    let refs = chord_refs(&resolved).ok()?;
    shape_of(refs.first()?)
}
```

`chord_midi_web/src/main.rs` の `mod` の並びに足す (`mod cursor;` の前):

```rust
mod chord_table;
```

- [ ] **Step 2: コンパイルを通す**

Run: `cd /Users/kmt/dev/chord-midi/chord_midi_web && cargo check --target wasm32-unknown-unknown 2>&1 | tail -10`
Expected: `ROOTS` / `QUALITIES` / `shape_for` が未使用という警告は出るが、エラーは無い。
`chord_refs` と `shape_of` と `Shape` はどれも `chord_midi::fingering` の
公開要素 (`fingering/mod.rs` の `pub fn` / `pub struct`)

- [ ] **Step 3: 引けることを目で確かめる**

`chord_midi_web/src/chord_table.rs` の末尾に、一時的な確認用のテストを足す:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shape_for_reads_every_cell_of_the_grid() {
        let mut found = 0;
        let mut missing: Vec<String> = Vec::new();
        for root in ROOTS {
            for (_, suffix) in QUALITIES {
                let name = format!("{}{}", root, suffix);
                match shape_for(&name) {
                    Some(_) => found += 1,
                    None => missing.push(name),
                }
            }
        }
        // 132通りのうち、いくつかは押さえ方が無い。全部無いとか全部あるとかに
        // なっていたら、記法の組み立てか経路のどこかが壊れている
        assert!(found > 60, "引けた数が少なすぎる: {} 件", found);
        assert!(
            missing.len() < 60,
            "引けない数が多すぎる: {:?}",
            missing
        );
    }

    #[test]
    fn shape_for_finds_a_known_chord() {
        let shape = shape_for("Am7").expect("Am7 は原典にある");
        assert_eq!(shape.frets, [None, Some(0), Some(2), Some(0), Some(1), None]);
        assert_eq!(shape.name, Some("Am7"));
    }

    #[test]
    fn shape_for_rejects_a_bad_name() {
        assert!(shape_for("Hm7").is_none());
    }
}
```

- [ ] **Step 4: テストを走らせる**

Run: `cd /Users/kmt/dev/chord-midi/chord_midi_web && cargo test 2>&1 | tail -10`
Expected: 3件 PASS。
(web クレートは wasm 向けだが `cargo test` はホストのターゲットでビルドできる。
この計画を書く前に空のテストで確認済み)

- [ ] **Step 5: 整形してコミット**

```bash
cd /Users/kmt/dev/chord-midi/chord_midi_web
cargo fmt --all
cargo test 2>&1 | tail -3
cd /Users/kmt/dev/chord-midi
git add -A
git commit -m "$(cat <<'MSG'
feat(web): コード名から押さえ方を引く shape_for を足す

探索は自前で持たず、コード名の文字列を組んで既存の経路
(RechordImporter -> resolve -> chord_refs -> shape_of) を通す。

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01F28qZZi3uLGGKV2azgKPX8
MSG
)"
```

---

### Task 3: コード表タブの画面

**Files:**
- Modify: `chord_midi_web/src/chord_table.rs` (コンポーネント本体)
- Modify: `chord_midi_web/src/main.rs` (`Tab::ChordTable`)

- [ ] **Step 1: コンポーネントを書く**

`chord_midi_web/src/chord_table.rs` の `shape_for` の下 (`#[cfg(test)]` の前) に足す:

```rust
/// 押している組み合わせ。`(ROOTS の添字, QUALITIES の添字)`
type Selected = Option<(usize, usize)>;

#[function_component(ChordTable)]
pub fn chord_table() -> Html {
    let selected = use_state(|| None as Selected);

    // 132通りを1度だけ引いて、押さえ方が無いボタンを薄く出す。
    // 押すまで有無が分からないと、空振りのボタンを何度も押すことになる。
    // 依存が `()` なので初回の1回だけ走る
    let available = use_memo((), |_| {
        let mut out = [[false; QUALITIES.len()]; ROOTS.len()];
        for (ri, root) in ROOTS.iter().enumerate() {
            for (qi, (_, suffix)) in QUALITIES.iter().enumerate() {
                out[ri][qi] = shape_for(&format!("{}{}", root, suffix)).is_some();
            }
        }
        out
    });

    let header = QUALITIES
        .iter()
        .map(|(label, _)| html! { <div class="chord-table-head">{*label}</div> })
        .collect::<Html>();

    let rows = ROOTS
        .iter()
        .enumerate()
        .map(|(ri, root)| {
            let cells = QUALITIES
                .iter()
                .enumerate()
                .map(|(qi, (_, suffix))| {
                    let onclick = {
                        let selected = selected.clone();
                        Callback::from(move |_| selected.set(Some((ri, qi))))
                    };
                    let is_active = *selected == Some((ri, qi));
                    let class = classes!(
                        "btn",
                        "chord-table-cell",
                        is_active.then_some("is-active"),
                        // 押さえ方が無い組み合わせは薄く出す。押せはする
                        (!available[ri][qi]).then_some("is-empty"),
                    );
                    html! {
                        <button type="button" {class} {onclick}>
                            {format!("{}{}", root, suffix)}
                        </button>
                    }
                })
                .collect::<Html>();
            html! {
                <>
                    <div class="chord-table-root">{*root}</div>
                    {cells}
                </>
            }
        })
        .collect::<Html>();

    let detail = match *selected {
        None => html! {
            <p class="empty">{"コードを押すと押さえ方が出ます。"}</p>
        },
        Some((ri, qi)) => {
            let name = format!("{}{}", ROOTS[ri], QUALITIES[qi].1);
            match shape_for(&name) {
                None => html! {
                    <>
                        <div class="chord-table-name">{name}</div>
                        <p class="empty">{"押さえ方データなし"}</p>
                    </>
                },
                Some(shape) => {
                    // 代替の押さえ方が選ばれたときだけ原典での名前を併記する。
                    // プレビューの図と同じ規則
                    let alias = match shape.name {
                        Some(n) if n != name => html! {
                            <span class="chord-alias">{format!(" (≒ {})", n)}</span>
                        },
                        _ => html! {},
                    };
                    html! {
                        <>
                            <div class="chord-table-name">{name}{alias}</div>
                            { board(shape.frets, None) }
                        </>
                    }
                }
            }
        }
    };

    html! {
        <div class="chord-table">
            <div class="chord-table-grid">
                // 左上の角。ルート名の列の見出しは無い
                <div class="chord-table-head"></div>
                {header}
                {rows}
            </div>
            <div class="chord-table-detail">{detail}</div>
        </div>
    }
}
```

- [ ] **Step 2: タブに並べる**

`chord_midi_web/src/main.rs` の `enum Tab` に足す:

```rust
enum Tab {
    Editor,
    Preview,
    ChordTable,
}
```

`on_preview_tab` の下に:

```rust
    let on_chord_table_tab = {
        let tab = tab.clone();
        Callback::from(move |_| tab.set(Tab::ChordTable))
    };
```

`preview` を組んでいる箇所の下に:

```rust
    let chord_table = html! { <ChordTable /> };
```

タブのボタン (`{"プレビュー"}` の `</button>` の後ろ) に:

```rust
                <button
                    class={classes!("tab", (*tab == Tab::ChordTable).then_some("is-active"))}
                    onclick={on_chord_table_tab}
                >
                    {"コード表"}
                </button>
```

`match *tab` に:

```rust
                    match *tab {
                        Tab::Editor => editor,
                        Tab::Preview => preview,
                        Tab::ChordTable => chord_table,
                    }
```

`use` に足す:

```rust
use chord_table::ChordTable;
```

- [ ] **Step 3: コンパイルを通す**

Run: `cd /Users/kmt/dev/chord-midi/chord_midi_web && cargo check --target wasm32-unknown-unknown 2>&1 | tail -10`
Expected: エラー無し

- [ ] **Step 4: 整形してコミット**

```bash
cd /Users/kmt/dev/chord-midi/chord_midi_web
cargo fmt --all
cargo check -q --target wasm32-unknown-unknown
cd /Users/kmt/dev/chord-midi
git add -A
git commit -m "$(cat <<'MSG'
feat(web): コード表タブを足す

12ルート x 11品質の格子ボタンを押すと、下の指板にその押さえ方が出る。
指板は表示専用で、逆引きはエディタタブの指板に残す。

132通りを初回に1度だけ引いて、押さえ方が無いボタンは薄く出す。
押すまで有無が分からないと空振りのボタンを何度も押すことになる。

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01F28qZZi3uLGGKV2azgKPX8
MSG
)"
```

---

### Task 4: 見た目

**Files:**
- Modify: `chord_midi_web/style.css`

- [ ] **Step 1: スタイルを足す**

`chord_midi_web/style.css` の末尾に:

```css
/* ---- コード表タブ ---- */

.chord-table {
    display: flex;
    flex-direction: column;
    gap: 20px;
}

/* ルート名の列 + 品質の列。横に長いので狭い画面ではスクロールさせる */
.chord-table-grid {
    display: grid;
    grid-template-columns: 44px repeat(11, minmax(56px, 1fr));
    gap: 4px;
    overflow-x: auto;
    padding-bottom: 4px;
}

.chord-table-head {
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--muted);
    font-size: 11px;
    font-weight: 600;
}

.chord-table-root {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    padding-right: 8px;
    color: var(--muted);
    font-size: 12px;
    font-weight: 600;
}

.chord-table-cell {
    padding: 6px 4px;
    font-size: 12px;
    white-space: nowrap;
}

.chord-table-cell.is-active {
    border-color: var(--accent);
    color: var(--accent);
    font-weight: 600;
}

/* 押さえ方データが無い組み合わせ。押せはするが、空振りだと分かるようにする */
.chord-table-cell.is-empty {
    opacity: 0.4;
}

.chord-table-detail {
    padding-top: 4px;
    border-top: 1px solid var(--line);
}

.chord-table-name {
    margin-bottom: 12px;
    font-size: 16px;
    font-weight: 600;
}
```

- [ ] **Step 2: 画面で確かめる**

```bash
cd /Users/kmt/dev/chord-midi/chord_midi_web
trunk serve --port 8099 --no-autoreload
```

`http://127.0.0.1:8099/` の「コード表」タブで:
1. 12行 × 11列の格子が出る。先頭行が `(maj) m 7 m7 …` の見出し
2. 押さえ方が無い組み合わせが薄い
3. `A` 行の `m7` を押す → 見出しに `Am7`、指板に 5弦開放 / 4弦2 / 3弦0 / 2弦1、
   6弦と1弦が ×
4. 指板のマスを押しても何も起きない (表示専用)
5. 薄いボタンを押す → 「押さえ方データなし」
6. 狭い幅 (430px) にすると格子が横スクロールする

確認したら `Ctrl-C` で止める。

- [ ] **Step 3: コミット**

```bash
cd /Users/kmt/dev/chord-midi
git add -A
git commit -m "$(cat <<'MSG'
style(web): コード表タブの格子と詳細のスタイル

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01F28qZZi3uLGGKV2azgKPX8
MSG
)"
```

---

### Task 5: README

**Files:**
- Modify: `README.md`

- [ ] **Step 1: 節を足す**

`README.md` の「## 逆引き（指板）」の節の**直後** (「## 押さえ方データ」の直前) に:

```markdown
## コード表タブ

コード名から押さえ方を見る。進行を書かずに「`C#m7` ってどう押さえるの？」を
引くための画面。

- 12ルート × 11品質 (`(maj) m 7 m7 M7 m7b5 sus4 6 add9 dim aug`) の格子
- ボタンを押すと下の指板にその押さえ方が出る
- 押さえ方データに無い組み合わせはボタンが薄い。押すと「押さえ方データなし」
- この指板は表示専用。クリックからの逆引きはエディタタブの指板にある
```

- [ ] **Step 2: コミット**

```bash
cd /Users/kmt/dev/chord-midi
git add -A
git commit -m "$(cat <<'MSG'
docs: README にコード表タブの節を足す

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01F28qZZi3uLGGKV2azgKPX8
MSG
)"
```

---

## 仕上げ

- [ ] **Step 1: 全部通す**

```bash
cd /Users/kmt/dev/chord-midi
cargo test -q --workspace 2>&1 | tail -3
cargo fmt --all --check
(cd chord_midi_web && cargo fmt --all --check && cargo test 2>&1 | tail -3 && cargo check -q --target wasm32-unknown-unknown)
```
Expected: 整形済み、workspace のテスト全通過、web のテスト3件通過、エラー無し

- [ ] **Step 2: 3つのタブを通しで確かめる**

```bash
cd /Users/kmt/dev/chord-midi/chord_midi_web
trunk serve --port 8099 --no-autoreload
```

1. **エディタ** — 指板をクリックして候補が出る。「挿入」で `tab(...)` が入る
2. **プレビュー** — 入力した進行の図が出る
3. **コード表** — ボタンで押さえ方が出る。指板はクリックできない

確認したら `Ctrl-C` で止める。

- [ ] **Step 3: PR を作る**

```bash
cd /Users/kmt/dev/chord-midi
git push -u origin feat/chord-table-tab
gh pr create --base feat/tab-notation --title "feat(web): コード名から押さえ方を引くコード表タブ" --body "stacked PR: base は #39 (tab 記法)。fingering::shape_of に依存する。

設計書: docs/superpowers/specs/2026-10-18-chord-table-tab-design.md"
```
