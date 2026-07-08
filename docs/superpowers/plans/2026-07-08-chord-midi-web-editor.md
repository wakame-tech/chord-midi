# chord_midi_web エディタ化 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `chord_midi_web` を、コード/ディグリー進行を書きながらリアルタイム再生・MIDI出力できるエディタにし、`arp(...)` アルペジオ記法を core に追加する。

**Architecture:** core ライブラリに `Node::Arpeggio(ChordNode)` を追加し、parser/exporter/transform/rechord-Display を対応させる。web(Yew) 側は textarea の `oninput` で即 parse→MIDI 再生成、BPM スライダーとループ再生を足す。

**Tech Stack:** Rust, nom (parser), midi_file (MIDI生成), Yew 0.21 + web-sys (WASM), Trunk (build), html-midi-player (再生).

---

## File Structure

- `chord_midi/src/model/ast.rs` — `Node` に `Arpeggio(ChordNode)` 追加
- `chord_midi/src/import/chord.rs` — `node_parser` に `arp(...)` 分岐
- `chord_midi/src/export/rechord.rs` — `Display for Node` に arm 追加
- `chord_midi/src/model/transform.rs` — `into_degree`/`into_pitch` で arp の inner key を transpose（共有ヘルパ化）
- `chord_midi/src/export/midi.rs` — `interpret_node` に Arpeggio 分岐（順次発音）
- `chord_midi_web/src/main.rs` — 全面書き換え（realtime + BPM + loop）

注: `import/sexp.rs` の `parse_node` は sexp 入力に対する分岐で網羅 match ではない（catch-all あり）。arp を sexp から作る要件はないので変更不要。

---

## Task 1: AST に Arpeggio バリアント + parser + Display + transform

新バリアント追加は網羅 match を壊すので、このタスクで parse・逆シリアライズ・transpose まで一括で通す。MIDI の interpret_node は暫定で block と同じ挙動にしておき（コンパイルを通す）、Task 2 で本実装する。

**Files:**
- Modify: `chord_midi/src/model/ast.rs`
- Modify: `chord_midi/src/import/chord.rs`
- Modify: `chord_midi/src/export/rechord.rs`
- Modify: `chord_midi/src/model/transform.rs`
- Modify: `chord_midi/src/export/midi.rs` (暫定 arm)

- [ ] **Step 1: 失敗するテストを書く（parser + roundtrip）**

`chord_midi/src/import/chord.rs` のテストモジュールに追加:

```rust
    #[test]
    fn test_arpeggio_parser() -> Result<()> {
        for code in ["arp(C)", "arp(Cm7)", "arp(C/E)"] {
            let span = span(code);
            let (res, node) = super::node_parser(span)?;
            assert_eq!(res.into_fragment(), "");
            assert!(matches!(node, crate::model::ast::Node::Arpeggio(_)));
        }
        Ok(())
    }
```

テストモジュール冒頭の `use super::chord_node_parser;` に加え `use super::node_parser;` は `super::node_parser` で参照済みなので追加不要。

- [ ] **Step 2: テストが失敗するのを確認**

Run: `cargo test -p chord_midi test_arpeggio_parser`
Expected: コンパイルエラー（`Node::Arpeggio` が存在しない）

- [ ] **Step 3: `Node` に Arpeggio を追加**

`chord_midi/src/model/ast.rs` の `enum Node`:

```rust
#[derive(Debug, PartialEq)]
pub enum Node {
    Chord(ChordNode),
    Arpeggio(ChordNode),
    Rest,
    Sustain,
    Repeat,
}
```

- [ ] **Step 4: parser に `arp(...)` 分岐を追加**

`chord_midi/src/import/chord.rs` の `node_parser`。`chord_node_parser` 分岐より**前**に置く（`arp` を先に食う）:

```rust
#[tracable_parser]
pub fn node_parser(s: Span) -> IResult<Span, Node> {
    alt((
        map(tag("="), |_| Node::Sustain),
        map(tag("_"), |_| Node::Rest),
        map(tag("%"), |_| Node::Repeat),
        map(tag("N.C."), |_| Node::Rest),
        map(
            delimited(tag("arp("), chord_node_parser, tag(")")),
            Node::Arpeggio,
        ),
        map(chord_node_parser, Node::Chord),
    ))(s)
}
```

`delimited` は既に `use nom::sequence::{delimited, preceded, tuple};` で import 済み。

- [ ] **Step 5: rechord `Display` に arm を追加**

`chord_midi/src/export/rechord.rs` の `Display for Node`:

```rust
impl Display for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Node::Chord(chord) => write!(f, "{}", chord),
            Node::Arpeggio(chord) => write!(f, "arp({})", chord),
            Node::Rest => write!(f, "N.C."),
            Node::Sustain => write!(f, "="),
            Node::Repeat => write!(f, "%"),
        }
    }
}
```

- [ ] **Step 6: transform で arp の inner key を transpose**

`chord_midi/src/model/transform.rs`。ChordNode の transpose を共有ヘルパにし、Chord/Arpeggio 両方で使う。`impl Ast` の前に private ヘルパを追加:

```rust
fn chordnode_into_degree(chord: ChordNode, key: Pitch) -> ChordNode {
    ChordNode {
        key: chord.key.into_degree(key),
        modifiers: chord.modifiers,
        on: chord.on.map(|on| on.into_degree(key)),
    }
}

fn chordnode_into_pitch(chord: ChordNode, pitch: Pitch) -> ChordNode {
    ChordNode {
        key: chord.key.into_pitch(pitch),
        modifiers: chord.modifiers,
        on: chord.on.map(|on| on.into_pitch(pitch)),
    }
}
```

`into_degree` の Measure map を差し替え:

```rust
                    .map(|node| match node {
                        Node::Chord(chord) => Node::Chord(chordnode_into_degree(chord, key)),
                        Node::Arpeggio(chord) => {
                            Node::Arpeggio(chordnode_into_degree(chord, key))
                        }
                        _ => node,
                    })
```

`into_pitch` の Measure map を差し替え:

```rust
                    .map(|node| match node {
                        Node::Chord(chord) => Node::Chord(chordnode_into_pitch(chord, pitch)),
                        Node::Arpeggio(chord) => {
                            Node::Arpeggio(chordnode_into_pitch(chord, pitch))
                        }
                        _ => node,
                    })
```

- [ ] **Step 7: midi.rs の interpret_node に暫定 arm を追加（コンパイル通し）**

`chord_midi/src/export/midi.rs` の `interpret_node` の match に、暫定で Chord と同じ挙動の arm を追加（Task 2 で置換）:

```rust
            Node::Chord(node) => {
                log::debug!("chord: {}: {:?}", node, node.modifiers);
                let chord = node.to_chord()?;
                self.pre = Some(chord.clone());
                self.sustain = dur;
            }
            Node::Arpeggio(node) => {
                // 暫定: block と同じ。Task 2 で順次発音に置換
                let chord = node.to_chord()?;
                self.pre = Some(chord.clone());
                self.sustain = dur;
            }
            Node::Repeat => {
```

- [ ] **Step 8: 逆シリアライズ往復テストを追加**

`chord_midi/src/export/rechord.rs` にテストモジュールが無ければ末尾に追加:

```rust
#[cfg(test)]
mod tests {
    use crate::import::{Importer, RechordImporter};

    #[test]
    fn test_arpeggio_roundtrip() {
        let ast = RechordImporter.import("arp(C) |\n").unwrap();
        let s = format!("{}", ast);
        assert!(s.contains("arp(C)"));
    }
}
```

- [ ] **Step 9: テストが通るのを確認**

Run: `cargo test -p chord_midi`
Expected: 全テスト PASS（既存 + `test_arpeggio_parser` + `test_arpeggio_roundtrip`）

- [ ] **Step 10: コミット**

```bash
git add chord_midi/src/model/ast.rs chord_midi/src/import/chord.rs chord_midi/src/export/rechord.rs chord_midi/src/model/transform.rs chord_midi/src/export/midi.rs
git commit -m "feat: add arp(...) arpeggio notation to AST/parser/transform"
```

---

## Task 2: MIDI exporter で arp を順次発音にする

`arp(C)` を、構成音を dur で等分して1音ずつ昇順に鳴らす。block コード（同時発音）と対比。

**Files:**
- Modify: `chord_midi/src/export/midi.rs`
- Test: `chord_midi/src/export/midi.rs`（同ファイル内 `#[cfg(test)] mod tests`）

- [ ] **Step 1: 失敗するテストを書く**

`chord_midi/src/export/midi.rs` 末尾に追加。private な `Score`/`Note` は同ファイル内テストから参照可能:

```rust
#[cfg(test)]
mod tests {
    use super::{Note, Score};
    use crate::import::{Importer, RechordImporter};

    fn notes_of(code: &str) -> Vec<Note> {
        let ast = RechordImporter.import(&format!("{}\n", code)).unwrap();
        let mut score = Score::new();
        score.interpret(ast).unwrap();
        score.notes
    }

    #[test]
    fn test_block_chord_is_simultaneous() {
        let notes = notes_of("C");
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].chord.as_ref().unwrap().len(), 3); // C E G
    }

    #[test]
    fn test_arpeggio_is_sequential() {
        let notes = notes_of("arp(C)");
        assert_eq!(notes.len(), 3); // 3音が順次
        assert!(notes.iter().all(|n| n.chord.as_ref().unwrap().len() == 1));
    }
}
```

- [ ] **Step 2: テストが失敗するのを確認**

Run: `cargo test -p chord_midi test_arpeggio_is_sequential`
Expected: FAIL（暫定実装では notes.len() == 1、chord に3音まとまっている）

`Note`/`Score` が private でテストから見えずコンパイルエラーになる場合は、`struct Note` を `pub(crate) struct Note`、`struct Score` を `pub(crate) struct Score` に変更（フィールド `notes`/`chord`/`duration` は既に `pub`）。同ファイル内 `mod tests` からは通常 private でも参照可能なので、まずそのまま試し、コンパイルエラー時のみ可視性を上げる。

- [ ] **Step 3: Arpeggio の本実装に置換**

`chord_midi/src/export/midi.rs` の `interpret_node` の暫定 Arpeggio arm を置換:

```rust
            Node::Arpeggio(node) => {
                let chord = node.to_chord()?;
                let notes = into_note_numbers(&chord)?;
                let k = notes.len().max(1) as u32;
                let each = (dur / k).max(1);
                let rem = dur % k;
                for (i, n) in notes.into_iter().enumerate() {
                    let d = each + if i as u32 == k - 1 { rem } else { 0 };
                    self.notes.push(Note::new(Some(vec![n]), d));
                }
            }
```

注: この arm に入る前に、`interpret_node` 冒頭の pending flush（`self.sustain != 0` の block、`self.rest != 0` の block）が先に走る。Arpeggio は `Node::Sustain`/`Node::Rest` ではないので pending は正しく flush され、arp 音は独立して push される。arp は `pre`/`sustain` を変更しないため、直後の `=`(Sustain) は arp を延長せず、それ以前の block chord に作用する既存挙動を保つ。

- [ ] **Step 4: テストが通るのを確認**

Run: `cargo test -p chord_midi`
Expected: 全テスト PASS

- [ ] **Step 5: CLI 回帰確認（実 MIDI 生成）**

`arp(...)` を含む入力で MIDI が生成できることを確認。テスト用ファイルを作成:

```bash
printf 'arp(C) | F | arp(G) | C |\n' > /tmp/arp_test.rechord
cargo run -p chord_midi_cli -- -i /tmp/arp_test.rechord --bpm 120 -o /tmp/arp_test.midi
```

Expected: エラーなく `/tmp/arp_test.midi` が生成される（ファイルサイズ > 0）

- [ ] **Step 6: コミット**

```bash
git add chord_midi/src/export/midi.rs
git commit -m "feat: render arp(...) as sequential arpeggio in MIDI export"
```

---

## Task 3: web エディタ（realtime parse + BPM + loop）

textarea を `oninput` で即再計算、BPM スライダー、ループ再生チェックボックスを追加。submit ボタン廃止。`loop` は Rust キーワードのため `<midi-player>` の属性は NodeRef 経由で set/remove する。

**Files:**
- Modify (全面書き換え): `chord_midi_web/src/main.rs`

- [ ] **Step 1: `main.rs` を書き換え**

`chord_midi_web/src/main.rs` を以下で置き換え:

```rust
use chord_midi::{
    export::{Exporter, MidiExporter},
    import::{Importer, RechordImporter},
};
use std::io::BufWriter;
use web_sys::{wasm_bindgen::JsCast, Blob, HtmlInputElement, HtmlTextAreaElement};
use yew::prelude::*;

fn new_blob(arr: &[u8], mime_type: &str) -> Blob {
    let u8_arr = unsafe { web_sys::js_sys::Uint8Array::view(arr) };
    let parts = web_sys::js_sys::Array::new();
    parts.push(&u8_arr);
    let mut options = web_sys::BlobPropertyBag::new();
    options.type_(mime_type);
    Blob::new_with_blob_sequence_and_options(&parts, &options).unwrap()
}

fn compile(input: &str, bpm: u8) -> anyhow::Result<(String, Vec<u8>)> {
    let ast = RechordImporter.import(&format!("{}\n", input))?;
    let ast_str = format!("{}", ast);
    let mut writer = BufWriter::new(Vec::new());
    MidiExporter { bpm }.export(&mut writer, ast)?;
    let bytes = writer.into_inner()?;
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
                Err(e) => error.set(e.to_string()),
            }
        });
    }

    // url / loop_on が変わるたび midi-player の属性を更新（loop は予約語なので属性直接操作）
    {
        let player_ref = player_ref.clone();
        use_effect_with(((*url).clone(), *loop_on), move |(url, loop_on)| {
            if let Some(el) = player_ref.cast::<web_sys::Element>() {
                if !url.is_empty() {
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
            <textarea rows=8 cols=50 id="input" oninput={on_input}
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
```

- [ ] **Step 2: WASM ビルドが通るのを確認**

Run: `cd chord_midi_web && trunk build`
Expected: `dist/` が生成され、コンパイルエラーなし

ビルドでエラーが出た場合の想定対処:
- `use_effect_with` のクロージャが `()` を返す必要 → 上記はクロージャ末尾で暗黙に `()` を返しているので追加不要。もし `must return teardown` 系エラーが出たら末尾に `|| ()` teardown を返す形へ変更（`move |deps| { ...; || () }`）。
- `target_unchecked_into` が見つからない → `use yew::TargetCast;` を追加。

- [ ] **Step 3: 手動確認**

Run: `cd chord_midi_web && trunk serve`
ブラウザで開き、textarea に `C | Am | F | arp(G) |` を入力 → 打鍵ごとに AST 表示更新・再生可能、BPM スライダーで速度変化、loop チェックで反復再生を確認。

- [ ] **Step 4: コミット**

```bash
git add chord_midi_web/src/main.rs
git commit -m "feat: realtime editor with BPM slider and loop playback"
```

---

## Self-Review メモ

- **Spec coverage:** (1)realtime parse+error=Task3, (2)BPM slider=Task3, (3)loop=Task3, (4)`arp(...)`=Task1+2。全項目カバー。
- **Type consistency:** `Node::Arpeggio(ChordNode)` を全 match 箇所（ast/parser/rechord/transform/midi）で統一。`compile(&str,u8)->Result<(String,Vec<u8>)>` を web で一貫使用。
- **既存挙動の非破壊:** transform は共有ヘルパ抽出で Chord 挙動不変。midi の block chord 経路は無変更。sexp は catch-all のため変更不要。
```