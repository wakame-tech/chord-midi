# Issue #12 フェーズ3: web (キー上下・スケール選択・プリセット) 実装計画

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) syntax.

**Goal:** web エディタに (1) キー上下ボタン、(2) スケール選択、(3) 進行プリセット挿入を追加する。すべて textarea のテキスト(`key:`/`scale:` ディレクティブ, degree 進行)を編集する形。

**Architecture:** テキスト編集ロジックは純関数として core (`chord_midi::edit`) に置き、そこでユニットテスト。web(`chord_midi_web/src/main.rs`)は UI とイベントで core の純関数を呼び input state を更新する。プリセットは静的配列。

**Tech Stack:** Rust, Yew 0.21, web-sys, chord_midi. base ブランチ `feat/issue-12-phase2-transforms`。

参照 spec: `docs/superpowers/specs/2026-07-08-progression-presets-and-transforms-design.md` の B。

**注意:** `chord_midi_web` は wasm ターゲットの bin クレートで native `cargo test` はできない。よって編集ロジックは core に置き `cargo test -p chord_midi` で検証する。web は `cd chord_midi_web && cargo build --target wasm32-unknown-unknown` で確認。

---

## Task 1: core `edit` モジュール (純関数 + テスト)

**Files:**
- Create: `chord_midi/src/edit.rs`
- Modify: `chord_midi/src/lib.rs`

`key:` 行のピッチを半音シフト、`scale:` 行を設定する純関数。テキストは複数行、ディレクティブは行頭 `key:` / `scale:`。

- [ ] **Step 1: 失敗するテストを書く**

`chord_midi/src/edit.rs` を作成(テスト含む):

```rust
use crate::model::pitch::Pitch;
use crate::model::scale::Scale;
use std::str::FromStr;

/// `key:` 行のピッチを `delta` 半音シフトして返す。`key:` 行が無ければ
/// `key: <C を delta シフトした音>` を先頭に挿入する。
pub fn shift_key(text: &str, delta: i8) -> String {
    let shift = |p: Pitch| -> Pitch {
        let n = (p as i8 + delta).rem_euclid(12) as u8;
        Pitch::try_from(n).unwrap()
    };
    let mut lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
    if let Some(line) = lines.iter_mut().find(|l| l.trim_start().starts_with("key:")) {
        if let Some(rest) = line.split_once("key:") {
            if let Ok(p) = Pitch::from_str(rest.1.trim()) {
                *line = format!("key: {}", shift(p));
            }
        }
        lines.join("\n")
    } else {
        // no key line: insert one (starting from C)
        let mut out = vec![format!("key: {}", shift(Pitch::C))];
        out.extend(lines);
        out.join("\n")
    }
}

/// `scale:` 行を `scale` に設定して返す。無ければ `key:` 行の直後(無ければ先頭)に挿入。
pub fn set_scale(text: &str, scale: Scale) -> String {
    let mut lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
    if let Some(line) = lines.iter_mut().find(|l| l.trim_start().starts_with("scale:")) {
        *line = format!("scale: {}", scale);
        lines.join("\n")
    } else {
        let insert_at = lines
            .iter()
            .position(|l| l.trim_start().starts_with("key:"))
            .map(|i| i + 1)
            .unwrap_or(0);
        lines.insert(insert_at, format!("scale: {}", scale));
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shift_key_existing() {
        assert_eq!(shift_key("key: C\nI |", 1), "key: C#\nI |");
        assert_eq!(shift_key("key: C\nI |", -1), "key: B\nI |");
        assert_eq!(shift_key("key: B\nI |", 1), "key: C\nI |"); // wrap
    }

    #[test]
    fn test_shift_key_absent_inserts() {
        assert_eq!(shift_key("I | IV |", 2), "key: D\nI | IV |");
    }

    #[test]
    fn test_set_scale_existing() {
        assert_eq!(set_scale("key: C\nscale: major\nI |", Scale::Dorian), "key: C\nscale: dorian\nI |");
    }

    #[test]
    fn test_set_scale_absent_after_key() {
        assert_eq!(set_scale("key: C\nI |", Scale::Lydian), "key: C\nscale: lydian\nI |");
    }
}
```

- [ ] **Step 2: `lib.rs` に登録**

`chord_midi/src/lib.rs`:
```rust
pub mod edit;
pub mod export;
pub mod import;
pub mod model;
```

- [ ] **Step 3: テスト実行**

Run: `cargo test -p chord_midi edit > /tmp/t.txt 2>&1; grep -E "test result|FAILED" /tmp/t.txt`
Expected: pass。`text.lines()` は末尾改行を落とすため join で再構成する点に注意(テスト期待値は改行正規化後)。もし末尾改行の有無で web の使い勝手に差が出るなら、web 側で対処(このタスクはテキスト→テキストの純変換に限定)。

- [ ] **Step 4: 全体テスト & コミット**

Run: `cargo test -p chord_midi > /tmp/t.txt 2>&1; grep "test result" /tmp/t.txt`(全 pass)
```bash
git add chord_midi/src/edit.rs chord_midi/src/lib.rs
git commit -m "feat: add chord_midi::edit text helpers (shift_key, set_scale)"
```

---

## Task 2: web プリセット + UI

**Files:**
- Create: `chord_midi_web/src/presets.rs`
- Modify: `chord_midi_web/src/main.rs`

- [ ] **Step 1: プリセット定義**

`chord_midi_web/src/presets.rs`:
```rust
/// (name, degree-notation progression). Curated staple progressions.
pub const PRESETS: &[(&str, &str)] = &[
    ("王道進行", "IVM7 | V7 | IIIm7 | VIm7 |"),
    ("カノン", "I | V | VIm | IIIm | IV | I | IV | V |"),
    ("小室進行", "VIm | IV | V | I |"),
    ("ツーファイブワン", "IIm7 | V7 | IM7 | IM7 |"),
    ("Just the Two of Us", "IVM7 | IIIm7 VI7 | IIm7 | V7 |"),
];
```

- [ ] **Step 2: main.rs に mod 追加とハンドラ**

`chord_midi_web/src/main.rs`:
- `mod presets;` を追加。
- `use chord_midi::edit::{shift_key, set_scale};` と `use chord_midi::model::scale::Scale;`。
- キー上下ボタン2つ。onclick で `input.set(shift_key(&input, +1|-1))`。
- スケール `<select>`(7旋法)。onchange で `input.set(set_scale(&input, Scale::from_str(value)))`。
- プリセットボタン群。`presets::PRESETS` を map してボタン化。onclick で現在の input 末尾にプリセット進行を append(改行区切り): `input.set(format!("{}\n{}", current.trim_end(), degrees))`(空なら degrees のみ)。

具体的な差分(既存 main.rs の html! に追記する UI ブロック):
```rust
            // key up/down
            <div>
                <button onclick={on_key_down}>{"♭ key"}</button>
                <button onclick={on_key_up}>{"key ♯"}</button>
            </div>
            // scale select
            <div>
                <label>{"scale: "}</label>
                <select onchange={on_scale}>
                    { for ["major","minor","dorian","phrygian","lydian","mixolydian","locrian"]
                        .iter().map(|s| html!{ <option value={*s}>{*s}</option> }) }
                </select>
            </div>
            // presets
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
```

ハンドラ実装(`app()` 内、既存 on_input 等の近く):
```rust
    let on_key_up = {
        let input = input.clone();
        Callback::from(move |_| input.set(chord_midi::edit::shift_key(&input, 1)))
    };
    let on_key_down = {
        let input = input.clone();
        Callback::from(move |_| input.set(chord_midi::edit::shift_key(&input, -1)))
    };
    let on_scale = {
        let input = input.clone();
        Callback::from(move |e: web_sys::Event| {
            let el: web_sys::HtmlSelectElement = e.target_unchecked_into();
            if let Ok(scale) = chord_midi::model::scale::Scale::from_str(&el.value()) {
                input.set(chord_midi::edit::set_scale(&input, scale));
            }
        })
    };
```
`use std::str::FromStr;`、`use web_sys::HtmlSelectElement;` を追加。`e.target_unchecked_into()` は `yew::prelude::*`(TargetCast)で解決。`input.set(...)` は `use_state` ハンドラ。`&input` は `&*input`(Deref)で `&str` になる(`shift_key(text: &str)` が `&String` を受けるには `&input` が `&String`→`&str` に coerce。`shift_key(&input, ..)` で `input: UseStateHandle<String>` → `&input` は `&UseStateHandle`。Deref で `&String`。関数は `&str` を取るので `shift_key(&input, ..)` が通らなければ `shift_key(input.as_str(), ..)` を使う)。実装時にコンパイルが通る形へ調整。

注: input が変わると既存の再計算 effect が走り parse→MIDI 再生成されるので、ボタン操作の結果が即再生に反映される。

- [ ] **Step 3: web ビルド**

Run: `cd chord_midi_web && cargo build --target wasm32-unknown-unknown 2>&1 | tail -5`
Expected: 成功。コンパイルエラー(Deref/型)は上記の注に沿って最小修正。`input.as_str()` 等。

- [ ] **Step 4: コミット**

```bash
cd /Users/kmt/dev/chord-midi && git add chord_midi_web/src/main.rs chord_midi_web/src/presets.rs
git commit -m "feat: web key up/down, scale select, and progression presets"
```

---

## Self-Review メモ

- **Spec coverage:** キー上下(B-1)、scale 選択(B-2)、プリセット(B-3、静的配列)。
- **Type consistency:** `edit::shift_key(&str, i8) -> String`、`edit::set_scale(&str, Scale) -> String`。web はこれらを呼び input state 更新。
- **テスト:** 編集ロジックは core でユニットテスト(web は native テスト不可のため)。web は wasm ビルドで検証。
- **非破壊:** main.rs の既存 realtime/BPM/loop/保存ボタンは維持。UI 追加のみ。
- **リスク:** `text.lines()` が末尾改行を落とす → web の `compile` は `format!("{}\n", input)` で改行付与するので実害なし。ボタン操作で input が置換されても oninput 相当の再計算 effect((input,bpm) 依存)が走る。
