# MIDI を押さえ方に合わせる Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 押さえ方が見つかったコードは、その押さえ方を実際に弾いたときの音を MIDI で鳴らす。プレビューの図と鳴っている音を一致させる。

**Architecture:** `export/midi.rs` に「`ChordNode` → MIDI ノート番号」を `voicing()` として1箇所に集約し、`fingering::find` の結果があれば開放弦 + フレットから実音を作る。見つからなければ従来の `into_note_numbers` に落とす。あわせて `Score::pre` を `Option<Chord>` から `Option<Vec<NoteNumber>>`（解決済みの声部）に変え、`Chord` / `Arpeggio` / `Brush` / `Transform` / `%` / `=` が同じ経路を通るようにする。

**Tech Stack:** Rust 2021 / midi_file 0.0.3

**Spec:** `docs/superpowers/specs/2026-09-15-midi-follows-fingering-design.md`

**Issue:** https://github.com/wakame-tech/chord-midi/issues/26

---

## File Structure

| ファイル | 変更 |
| --- | --- |
| `chord_midi/src/fingering/mod.rs` | `chord_ref` を `pub` にする |
| `chord_midi/src/export/midi.rs` | `Score::pre` の型変更、`voicing()` の追加、押さえ方の実音を使う分岐 |

---

## 前提知識（このリポジトリを知らない人向け）

- **ワークスペース**: ルートの `Cargo.toml` のメンバーは `chord_midi` と `chord_midi_cli`。`chord_midi_web` は `exclude` されており、`cd chord_midi_web` して別ビルドする。
- **`chord_midi_cli` は現在ビルドできない**（依存の `simplelog` → `time 0.3.34` が現行ツールチェインで `error[E0282]`）。このブランチとは無関係な既知の壊れ。テストは必ず `cargo test -p chord_midi` で走らせること。`cargo test`（ワークスペース全体）は使わない。
- **開放弦の MIDI ノート番号**: 6弦→1弦で `[40, 45, 50, 55, 59, 64]`（E2 A2 D3 G3 B3 E4）。これは**実音**。ギターは実音より1オクターブ高く記譜する楽器だが、ここで扱うのは鳴る音そのもの。
- **`Fingering.frets`**: `[Option<u8>; 6]` で 6弦→1弦。`None` がミュート、`Some(0)` が開放弦。
- **`NoteNumber`**: `midi_file::core::NoteNumber`。`NoteNumber::new(u8)` で作り、`.get()` で中の `u8` が取れる。
- **`Score::interpret_node` の作り**: コードを見たら即座に音を積むのではなく、`self.pre` に声部を覚えて `self.sustain` に尺をためる。次のノードが来たとき（または Score の末尾）に `Note` として吐き出す。`=`（Sustain）は尺を足し、`%`（Repeat）は `pre` をもう一度鳴らす。
- **テストヘルパ**: `notes_of(code)` は `resolve()` を通さずに解釈する。`resolved_notes_of(code)` は `resolve()` を通す。どちらも末尾に改行を足すので、呼ぶときは `"C |"` のように改行なしで渡す。
- **`pow(...)` は小節ではない**ので後ろに `|` を付けるとパースエラーになる。テスト入力は `"pow(C)"` と書く。

---

## Task 1: `Score::pre` を解決済みの声部にする

振る舞いを変えないリファクタ。`voicing()` を導入するが、中身はまだ従来どおり。**各ステップの後でテストが緑のままであることを確認する**。

**Files:**
- Modify: `chord_midi/src/export/midi.rs`

- [ ] **Step 1: `voicing` を足す**

`chord_midi/src/export/midi.rs` の `into_note_numbers` の下に足す。

```rust
/// `ChordNode` を鳴らす音（MIDI ノート番号）に直す。低い方から並ぶ
fn voicing(node: &ChordNode) -> Result<Vec<NoteNumber>> {
    into_note_numbers(&node.to_chord()?)
}
```

同じファイルの `use` に `ChordNode` を足す。

```rust
use crate::model::ast::{Ast, Beat, ChordNode, Func, Node, Pattern};
```

（既存の `use crate::model::ast::{...};` の行に `ChordNode` を足す）

- [ ] **Step 2: `pre` の型を変える**

同じファイルの `struct Score` を差し替える。

```rust
#[derive(Debug)]
struct Score {
    notes: Vec<Note>,
    sustain: u32,
    rest: u32,
    /// 直前に鳴らした声部。`=` と `%` が伸ばす対象
    pre: Option<Vec<NoteNumber>>,
}
```

同じファイルの `inspect` を差し替える。

```rust
    fn inspect(&self) {
        log::debug!(
            "pre={} sus={}/{}, rest={}/{}",
            self.pre
                .as_ref()
                .map(|notes| format!("{:?}", notes))
                .unwrap_or("None".to_string()),
            self.sustain,
            MEASURE_LENGTH,
            self.rest,
            MEASURE_LENGTH
        );
    }
```

- [ ] **Step 3: 吐き出しと各ノードの経路を差し替える**

同じファイルの `interpret_node` の先頭、サステインを吐き出すところを差し替える。

```rust
    fn interpret_node(&mut self, node: Node, dur: u32) -> Result<()> {
        self.inspect();
        if !matches!(node, Node::Sustain) && self.sustain != 0 {
            self.notes.push(Note::new(self.pre.clone(), self.sustain));
            self.sustain = 0;
        }
```

同じ関数の `Node::Chord` の腕を差し替える。

```rust
            Node::Chord(node) => {
                log::debug!("chord: {}: {:?}", node, node.modifiers);
                self.pre = Some(voicing(&node)?);
                self.sustain = dur;
            }
```

同じ関数の `Node::Arpeggio` の腕の最初の2行を差し替える。

```rust
            Node::Arpeggio(node) => {
                let all = voicing(&node)?;
```

（`let chord = node.to_chord()?;` と `let all = into_note_numbers(&chord)?;` の2行をこの1行にする。それ以降の `k` の計算とループはそのまま）

同じ関数の `Node::Brush` の腕を差し替える。

```rust
            Node::Brush(node) => {
                let notes = voicing(&node)?;
                self.pre = Some(notes.clone());
                self.notes.push(Note::brushed(Some(notes), dur));
            }
```

同じファイルの `interpret` の `Ast::Score` の腕、末尾の吐き出しを差し替える。

```rust
            Ast::Score(score) => {
                for node in score.into_iter() {
                    self.interpret(*node)?
                }
                if self.sustain != 0 {
                    self.notes.push(Note::new(self.pre.clone(), self.sustain));
                    self.sustain = 0;
                }
                Ok(())
            }
```

- [ ] **Step 4: テストが通ることを確認する**

Run: `cargo test -p chord_midi`
Expected: PASS。108件。振る舞いを変えていないので件数も結果も変わらない

- [ ] **Step 5: コミット**

```bash
git add chord_midi/src/export/midi.rs
git commit -m "refactor(midi): 声部の解決を voicing に集約する

Score::pre が Chord を持って鳴らす直前に音程へ変換していたのを、
解決済みの Vec<NoteNumber> を持つように変える。Chord / Arpeggio /
Brush / Transform / % / = が同じ経路を通るようになり、音程の決め方が
voicing の1箇所に収まる。振る舞いは変えていない。"
```

---

## Task 2: 押さえ方の実音を鳴らす

**Files:**
- Modify: `chord_midi/src/fingering/mod.rs`
- Modify: `chord_midi/src/export/midi.rs`

- [ ] **Step 1: 失敗するテストを書く**

`chord_midi/src/export/midi.rs` の `mod tests` に足す。

```rust
    /// 最初に鳴る和音の MIDI ノート番号
    fn pitches_of(code: &str) -> Vec<u8> {
        resolved_notes_of(code)
            .iter()
            .find(|n| n.chord.is_some())
            .expect("no sounding note")
            .chord
            .as_ref()
            .unwrap()
            .iter()
            .map(|n| n.get())
            .collect()
    }

    #[test]
    fn test_chord_sounds_the_fingering() {
        // C の押さえ方は x-3-2-0-1-x。
        // 5弦3fr=C3(48) 4弦2fr=E3(52) 3弦開放=G3(55) 2弦1fr=C4(60)
        assert_eq!(pitches_of("C |"), vec![48, 52, 55, 60]);
    }

    #[test]
    fn test_power_chord_sounds_the_fingering() {
        // 6弦5fr=A2(45) + 5弦7fr=E3(52)
        assert_eq!(pitches_of("pow(A@6)"), vec![45, 52]);
        // 5弦5fr=D3(50) + 4弦7fr=A3(57)
        assert_eq!(pitches_of("pow(D@5)"), vec![50, 57]);
        // 5弦7fr=E3(52) + 4弦9fr=B3(59)
        assert_eq!(pitches_of("pow(E@5)"), vec![52, 59]);
    }

    #[test]
    fn test_falls_back_when_no_fingering_matches() {
        // ルート1音だけの和音に合う押さえ方は無い (どの押さえ方も2音以上鳴る)
        // ので、従来の声部配置に落ちる
        let p = pitches_of("C#omit3omit5 |");
        assert_eq!(p.len(), 1, "{:?}", p);
    }

    #[test]
    fn test_voicing_is_ordered_low_to_high() {
        for code in ["C |", "Am7 |", "F |", "pow(A@6)"] {
            let p = pitches_of(code);
            assert!(p.windows(2).all(|w| w[0] < w[1]), "{}: {:?}", code, p);
        }
    }
```

- [ ] **Step 2: テストが失敗することを確認する**

Run: `cargo test -p chord_midi export::midi`
Expected: FAIL。`test_chord_sounds_the_fingering` が `assertion `left == right` failed: left: [60, 64, 67], right: [48, 52, 55, 60]`（従来の C4 E4 G4 のまま）

- [ ] **Step 3: `chord_ref` を公開する**

`chord_midi/src/fingering/mod.rs` の `chord_ref` に `pub` を付ける。ドキュメントコメントも足す。

```rust
/// `ChordNode` から押さえ方検索用の情報を取り出す。
/// 呼ぶ前に [`Ast::resolve`] を通しておくこと
pub fn chord_ref(node: &ChordNode) -> Result<ChordRef> {
```

- [ ] **Step 4: `voicing` で押さえ方の実音を使う**

`chord_midi/src/export/midi.rs` の `voicing` を差し替え、その上に定数を足す。

```rust
/// 開放弦の MIDI ノート番号。6弦→1弦 (E2 A2 D3 G3 B3 E4)
const OPEN_STRING_NOTES: [u8; 6] = [40, 45, 50, 55, 59, 64];

/// `ChordNode` を鳴らす音（MIDI ノート番号）に直す。低い方から並ぶ。
///
/// 押さえ方が見つかればその実音を鳴らす。プレビューが出している図と
/// 鳴っている音を一致させるため。見つからなければ従来の声部配置に落とす
fn voicing(node: &ChordNode) -> Result<Vec<NoteNumber>> {
    if let Some(f) = crate::fingering::find(&crate::fingering::chord_ref(node)?) {
        return Ok(f
            .frets
            .iter()
            .enumerate()
            .filter_map(|(i, fret)| fret.map(|n| NoteNumber::new(OPEN_STRING_NOTES[i] + n)))
            .collect());
    }
    into_note_numbers(&node.to_chord()?)
}
```

- [ ] **Step 5: 音数が変わる既存テストを直す**

押さえ方を使うようになると、`C` は3音（C4 E4 G4）ではなく `x-3-2-0-1-x` の4音になる。音数を見ている3つのアサーションを直す。

`chord_midi/src/export/midi.rs` の `test_block_chord_is_simultaneous` を差し替える。

```rust
    #[test]
    fn test_block_chord_is_simultaneous() {
        let notes = notes_of("C");
        assert_eq!(notes.len(), 1);
        // C の押さえ方 x-3-2-0-1-x は4本鳴る
        assert_eq!(notes[0].chord.as_ref().unwrap().len(), 4);
    }
```

`test_arpeggio_is_sequential` を差し替える。

```rust
    #[test]
    fn test_arpeggio_is_sequential() {
        let notes = notes_of("arp(C)");
        assert_eq!(notes.len(), 4); // 押さえ方の4音が順次
        assert!(notes.iter().all(|n| n.chord.as_ref().unwrap().len() == 1));
    }
```

`test_arpeggio_durations_sum_to_slot` を差し替える。

```rust
    #[test]
    fn test_arpeggio_durations_sum_to_slot() {
        // single-node measure: dur = MEASURE_LENGTH = 16, C の押さえ方は4音
        let notes = notes_of("arp(C)");
        let durs: Vec<u32> = notes.iter().map(|n| n.duration).collect();
        assert_eq!(durs, vec![4, 4, 4, 4]);
        assert_eq!(durs.iter().sum::<u32>(), 16);
    }
```

- [ ] **Step 6: テストが通ることを確認する**

Run: `cargo test -p chord_midi`
Expected: PASS。108 + 4 = 112件

- [ ] **Step 7: コミット**

```bash
git add chord_midi/src/fingering/mod.rs chord_midi/src/export/midi.rs
git commit -m "fix(midi): 押さえ方の実音を鳴らす

押さえ方プレビューが出している図と MIDI で鳴っている音が違っていた。
find はピッチクラスだけで照合しオクターブも転回も見ないのに対し、
MIDI 側は match_pitches が基準音に近いオクターブと転回を選ぶため。
pow(A@6) は A2+E3 と表示しながら E4+A4 (4度・+19半音) を鳴らしていた。

押さえ方が見つかったら、開放弦 + フレットから作った実音を鳴らす。
見つからなければ従来の声部配置に落とす。

これで全入力の MIDI がギターの声部配置になり、音域も下がる。C は
C4 E4 G4 の3音から x-3-2-0-1-x の C3 E3 G3 C4 の4音になる。
音数を見ていた既存テスト3件の期待値を直した。"
```

---

## Task 3: 仕上げ

**Files:**
- なし（確認と PR のみ）

- [ ] **Step 1: 全部のビルドとテストを流す**

Run: `cargo test -p chord_midi`
Expected: 112件 PASS

Run: `cd chord_midi_web && cargo check --target wasm32-unknown-unknown`
Expected: error ゼロ

Run: `cd chord_midi_web && trunk build --release --public-url /chord-midi/`
Expected: `✅ success`（CI と同じコマンド。`trunk` は `~/.cargo/bin/trunk` にある）

- [ ] **Step 2: ブラウザで音を確かめる**

Run: `cd chord_midi_web && trunk serve --port 8088 --no-autoreload`

`http://127.0.0.1:8088/` を開いて次を入力する。

```
pat("o.oo..oo", pow(A@6 | D@5 | E@5))
```

ブラウザのコンソールで、鳴っている音が押さえ方と一致するか確かめる。

```js
const ns = document.querySelector('#visualizer').noteSequence;
const byStart = {};
for (const n of ns.notes) (byStart[+n.startTime.toFixed(3)] ||= []).push(n.pitch);
[0, 2, 4].map(t => byStart[t].slice().sort((a, b) => a - b));
```

Expected: `[[45, 52], [50, 57], [52, 59]]`

- 45 = A2, 52 = E3 → 6弦5fr + 5弦7fr
- 50 = D3, 57 = A3 → 5弦5fr + 4弦7fr
- 52 = E3, 59 = B3 → 5弦7fr + 4弦9fr

確認する点:
1. 3小節とも押さえ方どおりの実音が鳴ること
2. 音程が完全5度（7半音）で、転回していないこと
3. 再生して音域が下がっていること（今までより低く聞こえる）
4. `C | Am7 | F | G |` を入れて、エラーが出ず鳴ること

- [ ] **Step 3: push して PR を出す**

```bash
git push -u origin feat/midi-follows-fingering
gh pr create --fill
```

PR 本文には `Closes #26` を入れる。
