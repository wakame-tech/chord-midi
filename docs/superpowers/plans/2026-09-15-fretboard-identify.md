# 指板クリックからのコード逆引き Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** web に3つ目のタブ「運指」を足し、指板をクリックして作った押さえ方から、コード名候補と rechord 記法を逆引きできるようにする。

**Architecture:** 逆引きロジックは `chord_midi/src/fingering/identify.rs` に置く（`find()` の逆）。押さえ方データ `FINGERINGS` との完全一致照合と、12ルート × 品質テーブルの理論計算を併用する。記法は必ず `ChordNode` の `Display` から作るので、出した文字列は必ずパーサに通る。web 側は `chord_midi_web/src/fretboard.rs` に新しいコンポーネントを作り、`main.rs` にはタブ1つと state 1つだけ足す。

**Tech Stack:** Rust (workspace), Yew 0.21 (CSR), trunk, nom ベースの自作パーサ

**Spec:** `docs/superpowers/specs/2026-09-15-fretboard-identify-design.md`

---

## 前提知識（この plan を実行する人向け）

### リポジトリの構成

```
chord_midi/          コアのライブラリ。パーサ、AST、MIDI 出力、押さえ方
chord_midi_cli/      CLI。今回は触らない
chord_midi_web/      Yew の web アプリ
docs/                押さえ方の原データ (JSON) と設計文書
scripts/             JSON から Rust テーブルを生成する Python
```

### 弦とフレットの表し方

配列はどこでも **6弦→1弦** の順。`[Option<u8>; 6]` で、`None` = ミュート、
`Some(0)` = 開放、`Some(n)` = n フレット。

```rust
// C コード = x3201x （6弦ミュート、5弦3fr、4弦2fr、3弦開放、2弦1fr、1弦ミュート）
[None, Some(3), Some(2), Some(0), Some(1), None]
```

記法の `@N` だけは **1 = 1弦** で、`Fingering.lowest_string` は **0 = 6弦**。
`chord_ref()` が `6 - n` で変換している。逆向きに戻すときも `6 - n`。

### ピッチクラス

`u16` のビット集合。bit n が立っていればピッチクラス n（0 = C, 1 = C#, ... 11 = B）
を含む。`chord_ref()` が返す `ChordRef.pitch_classes` がこれ。

### 既存の使う関数

```rust
// chord_midi/src/fingering/mod.rs
pub const OPEN_STRINGS: [u8; 6] = [4, 9, 2, 7, 11, 4];  // 開放弦のピッチクラス
pub fn chord_ref(node: &ChordNode) -> Result<ChordRef>;  // コードから検索用の情報
pub fn find(chord: &ChordRef) -> Option<&'static Fingering>;  // コード -> 押さえ方
fn relative_semitones(mask: u16, root: u8) -> impl Iterator<Item = u8>;  // private
fn missing_cost(semitone: u8) -> u32;  // private
fn extra_cost(semitone: u8) -> u32;    // private
```

### テストの流し方

```bash
cargo test -p chord_midi                 # ライブラリのテスト全部
cargo test -p chord_midi identify::      # このモジュールだけ
cargo test                               # workspace 全部
```

web のビルド:

```bash
cd chord_midi_web && trunk build
```

`trunk` が入っていなければ `cargo install trunk` と
`rustup target add wasm32-unknown-unknown`。入っていなければ
`cargo check -p chord_midi_web --target wasm32-unknown-unknown` でも型は通せる。

---

## File Structure

| ファイル | 役割 |
| --- | --- |
| `chord_midi/src/fingering/mod.rs` | 既存。private な採点関数を `pub(crate)` に上げ、開放弦の MIDI 番号をテスト外に出し、`identify` を再輸出する |
| `chord_midi/src/fingering/identify.rs` | **新規**。逆引きのロジック一式。ここだけで完結させる |
| `chord_midi_web/src/diagram.rs` | 既存。`POSITION_MARKS` を `pub(crate)` に上げるだけ |
| `chord_midi_web/src/fretboard.rs` | **新規**。指板の描画・クリック処理・候補カード |
| `chord_midi_web/src/main.rs` | 既存。タブ1つ、state 1つ、`mod fretboard;` だけ足す |
| `chord_midi_web/style.css` | 既存。`.fretboard` 系を末尾寄りに足す |
| `README.md` | 既存。運指タブの説明 |

---

## Task 1: 既存の private 要素を共有できるようにする

逆引きは既存の採点関数と定数を使い回す。まずそれを隣のモジュールから見えるようにする。
振る舞いは変えないので、ここではテストを足さず「既存テストが全部通ったまま」を確認する。

**Files:**
- Modify: `chord_midi/src/fingering/mod.rs`

- [ ] **Step 1: 変更前のテストが通ることを確認する**

Run: `cargo test -p chord_midi`
Expected: PASS（`test result: ok.` が出る。件数を控えておく）

- [ ] **Step 2: 開放弦の MIDI ノート番号をテスト外に出す**

`chord_midi/src/fingering/mod.rs` の `mod tests` の中にある定数を消す。

消す行（`mod tests` の冒頭）:

```rust
    /// 開放弦の MIDI ノート番号。6弦→1弦
    const OPEN_STRING_PITCHES: [u8; 6] = [40, 45, 50, 55, 59, 64];
```

代わりに、ファイル上部の `OPEN_STRINGS` の定義の直後に足す:

```rust
/// 標準チューニングの開放弦のピッチクラス。6弦→1弦の順
pub const OPEN_STRINGS: [u8; 6] = [4, 9, 2, 7, 11, 4];

/// 標準チューニングの開放弦の MIDI ノート番号。6弦→1弦の順。
/// ピッチクラスだけでは弦をまたいだ音の高低が比べられないので併せて持つ
pub const OPEN_STRING_PITCHES: [u8; 6] = [40, 45, 50, 55, 59, 64];
```

- [ ] **Step 3: 採点関数を `pub(crate)` に上げる**

同じファイルの3つの関数のシグネチャだけ書き換える。中身は触らない。

```rust
/// ビット集合に立っているピッチクラスを、root からの半音数にして返す
pub(crate) fn relative_semitones(mask: u16, root: u8) -> impl Iterator<Item = u8> {
```

```rust
/// 構成音にあるのに押さえ方に無い音のコスト。ルートからの半音数で引く
pub(crate) fn missing_cost(semitone: u8) -> u32 {
```

```rust
/// 押さえ方にあるのに構成音に無い音のコスト。ルートからの半音数で引く
pub(crate) fn extra_cost(semitone: u8) -> u32 {
```

- [ ] **Step 4: テストが変わらず通ることを確認する**

Run: `cargo test -p chord_midi`
Expected: PASS。件数が Step 1 と同じ

- [ ] **Step 5: コミット**

```bash
git add chord_midi/src/fingering/mod.rs
git commit -m "refactor(fingering): 逆引きから使う定数と採点関数を共有する

OPEN_STRING_PITCHES をテスト内から外に出し、relative_semitones /
missing_cost / extra_cost を pub(crate) にする。振る舞いは変えない。

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01RMaNBbqQuecYx8C4Agmr27"
```

---

## Task 2: identify の骨組みと「鳴っている音」の取り出し

押さえ方から鳴っている音のピッチクラス集合と最低音を出すところまで作る。
全ミュートなら空を返す。

**Files:**
- Create: `chord_midi/src/fingering/identify.rs`
- Modify: `chord_midi/src/fingering/mod.rs`

- [ ] **Step 1: 失敗するテストを書く**

`chord_midi/src/fingering/identify.rs` を新規作成し、これだけ書く:

```rust
//! 押さえ方からコード名を引く。[`super::find`] の逆。

#[cfg(test)]
mod tests {
    use super::*;

    /// "x3201x" のような文字列を frets に直す。x = ミュート、それ以外は1桁の16進
    fn frets(s: &str) -> [Option<u8>; 6] {
        let mut out = [None; 6];
        for (i, c) in s.chars().enumerate() {
            out[i] = match c {
                'x' => None,
                c => Some(c.to_digit(16).expect("フレットは x か16進1桁") as u8),
            };
        }
        out
    }

    #[test]
    fn identify_returns_nothing_for_all_muted() {
        assert!(identify(frets("xxxxxx")).is_empty());
    }

    #[test]
    fn identify_returns_something_for_a_real_chord() {
        // C コード。名前が何になるかは次のタスクで詰める
        assert!(!identify(frets("x3201x")).is_empty());
    }
}
```

`chord_midi/src/fingering/mod.rs` の `mod data;` の下に足す:

```rust
mod data;
mod identify;

pub use data::FINGERINGS;
pub use identify::{identify, Candidate, Source};
```

- [ ] **Step 2: テストが失敗することを確認する**

Run: `cargo test -p chord_midi identify::`
Expected: FAIL。`cannot find function `identify`` などのコンパイルエラー

- [ ] **Step 3: 最小の実装を書く**

`chord_midi/src/fingering/identify.rs` の `#[cfg(test)]` より上に足す:

```rust
//! 押さえ方からコード名を引く。[`super::find`] の逆。

use super::{Fingering, OPEN_STRINGS, OPEN_STRING_PITCHES};

/// 候補がどこから来たか
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    /// 押さえ方データにこの形があり、記法もこの形を再現する。
    /// 中身の `name` は原典での呼び名（"A♭△7" など。rechord の記法ではない）
    Data(&'static Fingering),
    /// 鳴っている音から計算しただけ
    Theory,
}

/// 逆引きの結果1件
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    /// rechord のコード名。"C" や "C/E"
    pub name: String,
    /// そのまま入力に書ける記法。"C/E@6" や "pow(A@6)"
    pub notation: String,
    pub source: Source,
    /// `notation` を書いても押さえた形には戻らない（今のデータで再現できない形）
    pub unreachable: bool,
    /// 小さいほど良い。表示順に使う
    pub score: u32,
}

/// 鳴っている音
struct Played {
    /// ピッチクラス集合
    pitch_classes: u16,
    /// 最低音のピッチクラス
    bass: u8,
    /// 最低音が鳴っている弦。0 = 6弦
    bass_string: u8,
}

/// 押さえ方から鳴っている音を出す。1本も鳴っていなければ None。
///
/// 最低音は弦の番号ではなく実 MIDI ピッチの最小値から取る。`Fingering.bass` は
/// 「鳴る弦のうち一番低い弦」で定義されているが、これは原典データがロー
/// ポジションばかりだから成り立つ近似で、自由に押さえられる指板では破れる
/// （6弦12fr = E3(52) より 5弦開放 = A2(45) の方が低い）。
fn played(frets: &[Option<u8>; 6]) -> Option<Played> {
    let mut pitch_classes = 0u16;
    let mut lowest: Option<(u8, u8, u8)> = None; // (midi, ピッチクラス, 弦)
    for (s, fret) in frets.iter().enumerate() {
        let Some(fret) = fret else { continue };
        let midi = OPEN_STRING_PITCHES[s] + fret;
        let pc = (OPEN_STRINGS[s] + fret) % 12;
        pitch_classes |= 1u16 << pc;
        if lowest.is_none_or(|(m, _, _)| midi < m) {
            lowest = Some((midi, pc, s as u8));
        }
    }
    lowest.map(|(_, bass, bass_string)| Played {
        pitch_classes,
        bass,
        bass_string,
    })
}

/// 押さえ方からコード名候補を引く。frets は 6弦→1弦、None = ミュート
pub fn identify(frets: [Option<u8>; 6]) -> Vec<Candidate> {
    let Some(_played) = played(&frets) else {
        return Vec::new();
    };
    // 中身は次のタスクで入れる。今は「鳴っていれば1件は返す」だけ
    vec![Candidate {
        name: String::new(),
        notation: String::new(),
        source: Source::Theory,
        unreachable: true,
        score: u32::MAX,
    }]
}
```

`is_none_or` は Rust 1.82 以降。古い toolchain で通らなければ
`lowest.map_or(true, |(m, _, _)| midi < m)` に置き換える。

- [ ] **Step 4: テストが通ることを確認する**

Run: `cargo test -p chord_midi identify::`
Expected: PASS（2件）

- [ ] **Step 5: コミット**

```bash
git add chord_midi/src/fingering/identify.rs chord_midi/src/fingering/mod.rs
git commit -m "feat(fingering): 逆引きの骨組みと鳴っている音の取り出し

最低音は弦の番号ではなく実 MIDI ピッチの最小値から取る。

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01RMaNBbqQuecYx8C4Agmr27"
```

---

## Task 3: 品質テーブルと理論計算でコード名を出す

12ルート × 品質テーブルを総当たりして採点し、一番合う名前を返す。
ここではまだスラッシュも `@N` もデータ照合も無い。

**Files:**
- Modify: `chord_midi/src/fingering/identify.rs`

- [ ] **Step 1: 失敗するテストを書く**

`mod tests` の中に足す。`identify_returns_something_for_a_real_chord` は
より強いテストに置き換わるので消す。

```rust
    #[test]
    fn identify_names_an_open_c() {
        let c = identify(frets("x3201x"));
        assert_eq!(c[0].name, "C");
    }

    #[test]
    fn identify_names_an_open_e() {
        let c = identify(frets("022100"));
        assert_eq!(c[0].name, "E");
    }

    #[test]
    fn identify_names_an_open_a_minor() {
        // x02210 = Am
        let c = identify(frets("x02210"));
        assert_eq!(c[0].name, "Am");
    }

    #[test]
    fn identify_names_a_seventh() {
        // 020100 = E7
        let c = identify(frets("020100"));
        assert_eq!(c[0].name, "E7");
    }

    #[test]
    fn identify_caps_the_number_of_candidates() {
        assert!(identify(frets("x3201x")).len() <= 5);
    }

    #[test]
    fn identify_has_no_duplicate_names() {
        let c = identify(frets("x3201x"));
        let mut names: Vec<_> = c.iter().map(|x| x.name.clone()).collect();
        names.sort();
        let before = names.len();
        names.dedup();
        assert_eq!(names.len(), before);
    }
```

- [ ] **Step 2: テストが失敗することを確認する**

Run: `cargo test -p chord_midi identify::`
Expected: FAIL。`assertion `left == right` failed: left: "", right: "C"`

- [ ] **Step 3: 実装を書く**

`identify.rs` の `use` を差し替える:

```rust
use super::{
    chord_ref, extra_cost, missing_cost, relative_semitones, ChordRef, Fingering, OPEN_STRINGS,
    OPEN_STRING_PITCHES,
};
use crate::model::{ast::ChordNode, key::Key, modifier::Modifier, pitch::Pitch};
use std::collections::BTreeSet;
```

`Played` の下に足す:

```rust
/// 返す候補の最大件数
const MAX_CANDIDATES: usize = 5;

/// 逆引きで当てにいくコードの品質。`Modifier::Major(5)` は `chord_node_parser`
/// と同じく常に種として入れるので、ここには書かない。上から「よくある」順で、
/// 同点のときはこの順が優先される。
///
/// 入れていない品質:
/// - m6: `Modifier::Minor(6)` はマイナースケールの6度 = b6 を足す。実際の m6
///   コードは長6度なので今の `Modifier` では正しく組めない
/// - oct: 構成音がルート1音だけなので、どの1音を押さえても完全一致して候補を
///   埋め尽くす。オクターブ形はデータ照合で拾う
const QUALITIES: &[&[Modifier]] = &[
    &[],                                      // C
    &[Modifier::Minor(5)],                    // Cm
    &[Modifier::Dominant(7)],                 // C7
    &[Modifier::Minor(7)],                    // Cm7
    &[Modifier::Major(7)],                    // Cmaj7
    &[Modifier::Sus4],                        // Csus4
    &[Modifier::Sus2],                        // Csus2
    &[Modifier::Dominant(6)],                 // C6
    &[Modifier::Add(9)],                      // Cadd9
    &[Modifier::Dominant(9)],                 // C9
    &[Modifier::Minor(9)],                    // Cm9
    &[Modifier::Major(9)],                    // Cmaj9
    &[Modifier::MinorMajaor7],                // CmM7
    &[Modifier::Minor(7), Modifier::Flat5th], // Cm7b5
    &[Modifier::Dim],                         // Cdim
    &[Modifier::Dim7],                        // Cdim7
    &[Modifier::Aug],                         // Caug
    &[Modifier::Power],                       // C5 (記法は pow(C)。Task 6 で包む)
];

/// 品質から `ChordNode` を作る。`chord_node_parser` と同じく `Major(5)` を
/// 種にしてから修飾を足す。種が無いと `Sus4` のような「既存の3rdを消して4thを
/// 入れる」修飾が土台の無いところに当たって壊れる
fn node(root: Pitch, quality: &[Modifier], on: Option<Pitch>) -> ChordNode {
    let mut modifiers = BTreeSet::from([Modifier::Major(5)]);
    modifiers.extend(quality.iter().cloned());
    ChordNode {
        key: Key::Absolute(root),
        modifiers,
        on: on.map(Key::Absolute),
        root_string: None,
    }
}

/// 押さえた音とコードのずれを点にする。小さいほど良い。
/// 押さえた音のうちコードで説明できない音が2つ以上なら候補にしない
fn score(played: &Played, chord: &ChordRef) -> Option<u32> {
    let extra_mask = played.pitch_classes & !chord.pitch_classes;
    if extra_mask.count_ones() >= 2 {
        return None;
    }
    let extra: u32 = relative_semitones(extra_mask, chord.root)
        .map(extra_cost)
        .sum();
    let missing: u32 = relative_semitones(chord.pitch_classes & !played.pitch_classes, chord.root)
        .map(missing_cost)
        .sum();
    // ルートが鳴っていない呼び方は疑わしい
    let root_silent = if (played.pitch_classes >> chord.root) & 1 == 0 {
        3
    } else {
        0
    };
    // 最低音がルートでないのにスラッシュで書いていない
    let bass_mismatch = if chord.bass.is_none() && played.bass != chord.root {
        2
    } else {
        0
    };
    Some(extra + missing + root_silent + bass_mismatch)
}
```

`identify` を書き換える:

```rust
/// 押さえ方からコード名候補を引く。frets は 6弦→1弦、None = ミュート
pub fn identify(frets: [Option<u8>; 6]) -> Vec<Candidate> {
    let Some(played) = played(&frets) else {
        return Vec::new();
    };

    // (score, QUALITIES での並び順, ノード, 検索用の情報)
    let mut scored: Vec<(u32, usize, ChordNode, ChordRef)> = Vec::new();
    for root in 0..12u8 {
        let Ok(root_pitch) = Pitch::try_from(root) else {
            continue;
        };
        for (qi, quality) in QUALITIES.iter().enumerate() {
            let node = node(root_pitch, quality, None);
            let Ok(r) = chord_ref(&node) else { continue };
            let Some(s) = score(&played, &r) else { continue };
            scored.push((s, qi, node, r));
        }
    }
    scored.sort_by_key(|(s, qi, _, _)| (*s, *qi));

    let mut out: Vec<Candidate> = Vec::new();
    for (s, _, node, r) in scored {
        if out.len() >= MAX_CANDIDATES {
            break;
        }
        if out.iter().any(|c| c.name == r.label) {
            continue;
        }
        out.push(Candidate {
            name: r.label.clone(),
            notation: node.to_string(),
            source: Source::Theory,
            unreachable: false,
            score: s,
        });
    }
    out
}
```

`Fingering` がまだ使われず警告になるが、Task 5 で使うので
`use` はそのまま残す。警告が邪魔なら Task 5 まで `Fingering` を
`use` から外し、Task 5 で戻す。

- [ ] **Step 4: テストが通ることを確認する**

Run: `cargo test -p chord_midi identify::`
Expected: PASS（7件）

失敗したら、`cargo test -p chord_midi identify:: -- --nocapture` で実際に出た名前を
出して QUALITIES の並び順とコストを見直す。`E7` が `E` に負けるなら、押さえた
b7 が `E` 側で extra 1点しか付いていないのが原因なので、`extra_cost` ではなく
足切り（`count_ones() >= 2`）の閾値ではなく `missing` 側を疑うこと。

- [ ] **Step 5: コミット**

```bash
git add chord_midi/src/fingering/identify.rs
git commit -m "feat(fingering): 鳴っている音からコード名を計算する

12ルート x 品質テーブルを総当たりし、既存の missing_cost / extra_cost で
採点する。品質テーブルは chord_node_parser と同じく Major(5) を種にした
追加の修飾だけを持つ。

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01RMaNBbqQuecYx8C4Agmr27"
```

---

## Task 4: スラッシュコードの候補を出す

最低音がルートと違うとき、`C/E` のような呼び方も候補に入れる。

**Files:**
- Modify: `chord_midi/src/fingering/identify.rs`

- [ ] **Step 1: 失敗するテストを書く**

`mod tests` に足す:

```rust
    #[test]
    fn identify_offers_a_slash_chord_when_the_bass_is_not_the_root() {
        // x32010 は 5弦3fr(C) が最低音なので C のまま。1弦を開放にした
        // "332010" は 6弦3fr = G が最低音になる C/G
        let c = identify(frets("332010"));
        assert!(
            c.iter().any(|x| x.name == "C/G"),
            "C/G が候補に無い: {:?}",
            c.iter().map(|x| &x.name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn identify_prefers_the_slash_name_when_the_bass_is_not_the_root() {
        // xx0232 は D。4弦開放(D) が最低音なのでスラッシュにはならない
        let d = identify(frets("xx0232"));
        assert_eq!(d[0].name, "D");
    }
```

- [ ] **Step 2: テストが失敗することを確認する**

Run: `cargo test -p chord_midi identify::`
Expected: `identify_offers_a_slash_chord_when_the_bass_is_not_the_root` が FAIL

- [ ] **Step 3: 実装を書く**

`identify` の内側のループを書き換える。`quality` のループの中身を差し替える:

```rust
        for (qi, quality) in QUALITIES.iter().enumerate() {
            // `on` 無しと、最低音がルートと違うときの `on` 付きを両方試す。
            // `C/E` と `C` はどちらも正しい呼び方なので、どちらが上に来るかは
            // 採点に任せる
            let mut ons: Vec<Option<Pitch>> = vec![None];
            if played.bass != root {
                if let Ok(bass) = Pitch::try_from(played.bass) {
                    ons.push(Some(bass));
                }
            }
            for on in ons {
                let node = node(root_pitch, quality, on);
                let Ok(r) = chord_ref(&node) else { continue };
                let Some(s) = score(&played, &r) else { continue };
                scored.push((s, qi, node, r));
            }
        }
```

- [ ] **Step 4: テストが通ることを確認する**

Run: `cargo test -p chord_midi identify::`
Expected: PASS（9件）

- [ ] **Step 5: コミット**

```bash
git add chord_midi/src/fingering/identify.rs
git commit -m "feat(fingering): 最低音がルートと違うときスラッシュ名も候補にする

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01RMaNBbqQuecYx8C4Agmr27"
```

---

## Task 5: データ照合と @N の往復検証

押さえ方データに同じ形があれば `Source::Data` を立てる。記法は「書いたら元の形に
戻るか」を `find()` への往復で確かめ、戻らなければ `@N` を足して再試行する。

`Fingering.name` は原典表記（`A♭△7` `Am7-5` `D7(9)`）で rechord のパーサには
通らないので、**名前と記法は必ず段2の `ChordNode` から作る**。データ照合は
「原典に載っているか」の判定にだけ使う。

**Files:**
- Modify: `chord_midi/src/fingering/identify.rs`

- [ ] **Step 1: 失敗するテストを書く**

`mod tests` に足す:

```rust
    #[test]
    fn identify_marks_a_shape_that_exists_in_the_data() {
        let c = identify(frets("x3201x"));
        assert_eq!(c[0].name, "C");
        assert_eq!(c[0].notation, "C");
        assert!(matches!(c[0].source, Source::Data(_)));
        assert!(!c[0].unreachable);
    }

    #[test]
    fn identify_marks_a_shape_that_is_not_in_the_data() {
        // x3555x は C だが、この形は押さえ方データに無い
        let c = identify(frets("x3555x"));
        assert_eq!(c[0].name, "C");
        assert_eq!(c[0].source, Source::Theory);
        assert!(c[0].unreachable);
    }

    /// 記法を読み直して押さえ方検索用の情報にする。`node_parser` は private
    /// モジュールにあり Span の組み立ても面倒なので、公開 API の
    /// RechordImporter を通す。これなら `pow(A@6)` のような関数も読める
    fn reparse(notation: &str) -> Vec<ChordRef> {
        use crate::import::{Importer, RechordImporter};
        let ast = RechordImporter
            .import(&format!("{} |\n", notation))
            .unwrap_or_else(|e| panic!("{} がパースできない: {}", notation, e))
            .resolve();
        chord_refs(&ast).unwrap()
    }

    /// データの全要素について、逆引きが1件は返し、Data と印が付いた候補の記法を
    /// 読み直して find() に流すと元の形が戻る
    #[test]
    fn every_shape_in_the_data_round_trips() {
        // 可動形は Task 6 まで扱わないので、ここでは原典の押さえ方だけ見る
        for f in FINGERINGS.iter().filter(|f| f.kind == ShapeKind::Normal) {
            let got = identify(f.frets);
            assert!(!got.is_empty(), "{} ({:?}) の候補が空", f.name, f.frets);
            for c in got.iter().filter(|c| matches!(c.source, Source::Data(_))) {
                let refs = reparse(&c.notation);
                assert_eq!(refs.len(), 1, "{} が1コードにならない", c.notation);
                let back = find(&refs[0])
                    .unwrap_or_else(|| panic!("{} の押さえ方が引けない", c.notation));
                assert_eq!(
                    back.frets, f.frets,
                    "{} -> {} で形が変わった",
                    f.name, c.notation
                );
            }
        }
    }
```

`mod tests` の `use super::*;` の下に足す:

```rust
    use crate::fingering::chord_refs;
```

`FINGERINGS` `find` `chord_ref` `ShapeKind` `ChordRef` は `use super::*;` で
入ってくる（子モジュールは親の private な `use` も見える）。

- [ ] **Step 2: テストが失敗することを確認する**

Run: `cargo test -p chord_midi identify::`
Expected: まず `ShapeKind` と `find` が未 import のコンパイルエラー。
Step 3 の `use` の差し替えだけ先に当てると、
`identify_marks_a_shape_that_exists_in_the_data` が
「`source` が `Theory` のまま」で FAIL する

- [ ] **Step 3: 実装を書く**

`use` に `find` と `ShapeKind` を足す:

```rust
use super::{
    chord_ref, extra_cost, find, missing_cost, relative_semitones, ChordRef, Fingering, ShapeKind,
    FINGERINGS, OPEN_STRINGS, OPEN_STRING_PITCHES,
};
```

`score` の下に足す:

```rust
/// 記法を決める。`@N` 無しで元の形に戻るならそのまま、戻らなければ `@N` を
/// 足してもう一度試す。それでも戻らなければ `@N` 無しの名前を返し、
/// 「再現できない」と伝える。返り値は (記法, 元の形を再現するか)
fn notation_for(node: &ChordNode, r: &ChordRef, played: &Played, frets: &[Option<u8>; 6]) -> (String, bool) {
    if find(r).map(|f| f.frets) == Some(*frets) {
        return (node.to_string(), true);
    }
    let mut with_string = node.clone();
    // 記法は 1 = 1弦、Played.bass_string は 0 = 6弦
    with_string.root_string = Some(6 - played.bass_string);
    if let Ok(r2) = chord_ref(&with_string) {
        if find(&r2).map(|f| f.frets) == Some(*frets) {
            return (with_string.to_string(), true);
        }
    }
    (node.to_string(), false)
}
```

`identify` の候補を作るループを書き換える:

```rust
    // 同じ形が押さえ方データにあるか。名前はここから取らない（原典表記で
    // rechord のパーサに通らないため）。使うのは「載っているか」と kind だけ
    let data = FINGERINGS.iter().find(|f| f.frets == frets);

    let mut out: Vec<Candidate> = Vec::new();
    for (s, _, node, r) in scored {
        if out.len() >= MAX_CANDIDATES {
            break;
        }
        if out.iter().any(|c| c.name == r.label) {
            continue;
        }
        let (notation, reproduces) = notation_for(&node, &r, &played, &frets);
        let source = match data {
            Some(f) if reproduces && f.kind == ShapeKind::Normal => Source::Data(f),
            _ => Source::Theory,
        };
        out.push(Candidate {
            name: r.label.clone(),
            notation,
            source,
            unreachable: !reproduces,
            score: s,
        });
    }
    out
```

- [ ] **Step 4: テストが通ることを確認する**

Run: `cargo test -p chord_midi identify::`
Expected: PASS（12件）

`every_shape_in_the_data_round_trips` は Step 1 の通り原典の押さえ方だけを見る。
可動形は Task 6 でフィルタを外す。

- [ ] **Step 5: コミット**

```bash
git add chord_midi/src/fingering/identify.rs
git commit -m "feat(fingering): データ照合と @N の往復検証

記法を書いて find() に流し、元の形に戻るかで @N の要否を決める。戻らない
形は unreachable を立てる。原典名は表記が違うので記法には使わない。

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01RMaNBbqQuecYx8C4Agmr27"
```

---

## Task 6: 可動形を pow() / oct() で包む

`C5` と書いても `Modifier::Dominant(5)` に読まれてただのメジャートライアドになる
（`Modifier::Power` は `pow()` からしか作れない）。可動形の記法は関数で包む。

**Files:**
- Modify: `chord_midi/src/fingering/identify.rs`

- [ ] **Step 1: 失敗するテストを書く**

`mod tests` に足す:

```rust
    #[test]
    fn identify_names_a_power_shape() {
        // 57xxxx = 6弦5fr + 5弦7fr の A パワーコード
        let c = identify(frets("57xxxx"));
        assert_eq!(c[0].name, "A5");
        assert_eq!(c[0].notation, "pow(A@6)");
        assert!(matches!(c[0].source, Source::Data(_)));
        assert!(!c[0].unreachable);
    }

    #[test]
    fn identify_names_an_octave_shape() {
        // 5x7xxx = 6弦5fr + 4弦7fr の A オクターブ形
        let c = identify(frets("5x7xxx"));
        assert_eq!(c[0].name, "Aoct");
        assert_eq!(c[0].notation, "oct(A@6)");
        assert!(matches!(c[0].source, Source::Data(_)));
    }

    #[test]
    fn identify_names_a_five_string_power_shape() {
        // x02xxx = 5弦開放 + 4弦2fr の A パワーコード
        let c = identify(frets("x02xxx"));
        assert_eq!(c[0].notation, "pow(A@5)");
    }
```

`every_shape_in_the_data_round_trips` のフィルタを外して全データにする:

```rust
        for f in FINGERINGS.iter() {
```

`reparse` は `RechordImporter` を通すので `pow(A@6)` / `oct(A@6)` も読める。
可動形を外す必要はない。

- [ ] **Step 2: テストが失敗することを確認する**

Run: `cargo test -p chord_midi identify::`
Expected: `identify_names_a_power_shape` が FAIL（`name` が `"A5"` にならない、
または `notation` が `"A5"` のまま）

- [ ] **Step 3: 実装を書く**

`notation_for` の下に足す:

```rust
/// 可動形（パワーコード / オクターブ形）の候補。
///
/// `C5` と書くと `Modifier::Dominant(5)` に読まれてただのメジャートライアドに
/// なってしまう（`Modifier::Power` は `pow()` からしか作れない）ので、記法は
/// 関数で包む。どの弦で取るかが形そのものなので `@N` は常に付ける
fn movable_candidate(f: &'static Fingering) -> Option<Candidate> {
    let (func, suffix) = match f.kind {
        ShapeKind::Power => ("pow", "5"),
        ShapeKind::Octave => ("oct", "oct"),
        ShapeKind::Normal => return None,
    };
    let root = Pitch::try_from(f.bass).ok()?;
    // Fingering.lowest_string は 0 = 6弦、記法の @N は 1 = 1弦
    let string = 6 - f.lowest_string;
    Some(Candidate {
        name: format!("{}{}", root, suffix),
        notation: format!("{}({}@{})", func, root, string),
        source: Source::Data(f),
        unreachable: false,
        score: 0,
    })
}
```

`identify` の `let mut out` の直後に足す:

```rust
    let mut out: Vec<Candidate> = Vec::new();
    // 可動形は段2が出せない（Power は記法が違い、Octave は1音なので何にでも
    // 一致する）ので、データに当たったときだけここで先頭に足す
    if let Some(c) = data.and_then(movable_candidate) {
        out.push(c);
    }
```

- [ ] **Step 4: テストが通ることを確認する**

Run: `cargo test -p chord_midi identify::`
Expected: PASS（15件）

- [ ] **Step 5: workspace 全部のテストを流す**

Run: `cargo test`
Expected: PASS

- [ ] **Step 6: コミット**

```bash
git add chord_midi/src/fingering/identify.rs
git commit -m "feat(fingering): 可動形の記法を pow() / oct() で包む

C5 と書くと Dominant(5) に読まれてメジャートライアドになるため、パワー
コードとオクターブ形は関数の記法で出す。@N は形そのものなので常に付ける。

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01RMaNBbqQuecYx8C4Agmr27"
```

---

## Task 7: 指板コンポーネントを描く

クリックできる指板だけ作る。候補の表示は次のタスク。

**Files:**
- Create: `chord_midi_web/src/fretboard.rs`
- Modify: `chord_midi_web/src/diagram.rs:9`
- Modify: `chord_midi_web/src/main.rs`

- [ ] **Step 1: `POSITION_MARKS` を共有できるようにする**

`chord_midi_web/src/diagram.rs` の9行目あたり:

```rust
/// 実ギターのポジションマークが入るフレット。指板の下に番号を出す
pub(crate) const POSITION_MARKS: [u8; 8] = [3, 5, 7, 9, 12, 15, 17, 19];
```

- [ ] **Step 2: `fretboard.rs` を作る**

`chord_midi_web/src/fretboard.rs` を新規作成:

```rust
use chord_midi::fingering::identify;
use yew::prelude::*;

use crate::diagram::POSITION_MARKS;

/// 描く一番上のフレット。0（開放）から数える
const MAX_FRET: u8 = 12;

#[derive(Properties, PartialEq)]
pub struct FretboardProps {
    /// 6弦→1弦。None = ミュート、Some(0) = 開放
    pub frets: [Option<u8>; 6],
    pub on_change: Callback<[Option<u8>; 6]>,
    /// 記法をエディタに挿入する
    pub on_insert: Callback<String>,
}

#[function_component(Fretboard)]
pub fn fretboard(props: &FretboardProps) -> Html {
    let on_clear = {
        let on_change = props.on_change.clone();
        Callback::from(move |_| on_change.emit([None; 6]))
    };
    html! {
        <div class="fretboard-tab">
            <p class="fretboard-help">
                {"指板を押すとその弦の音が決まる。同じところをもう一度押すとミュート。"}
            </p>
            { board(props) }
            <div class="block">
                <button class="btn" onclick={on_clear}>{"クリア"}</button>
            </div>
        </div>
    }
}

fn board(props: &FretboardProps) -> Html {
    // 1弦を上に描く。frets は 6弦→1弦 の順なので逆から回す
    let rows = (0..6usize)
        .rev()
        .map(|s| {
            let value = match props.frets[s] {
                None => "×".to_string(),
                Some(0) => "○".to_string(),
                Some(n) => n.to_string(),
            };
            let cells = (0..=MAX_FRET)
                .map(|fret| {
                    let onclick = {
                        let frets = props.frets;
                        let on_change = props.on_change.clone();
                        Callback::from(move |_| {
                            let mut next = frets;
                            // 同じところをもう一度押したらその弦をミュートにする
                            next[s] = if next[s] == Some(fret) { None } else { Some(fret) };
                            on_change.emit(next);
                        })
                    };
                    let on = props.frets[s] == Some(fret);
                    // 0列は開放弦。ナットの左側なので弦だけ描いてフレット線は描かない
                    let class = classes!(
                        "fb-cell",
                        (fret == 0).then_some("is-open"),
                        (fret == 1).then_some("is-nut"),
                        on.then_some("is-on"),
                    );
                    let dot = if on {
                        html! { <span class="fb-dot"></span> }
                    } else {
                        Html::default()
                    };
                    html! { <div {class} {onclick}>{dot}</div> }
                })
                .collect::<Html>();
            html! {
                <>
                    <div class="fb-label">
                        {format!("{}弦", 6 - s)}
                        <span class="fb-value">{value}</span>
                    </div>
                    {cells}
                </>
            }
        })
        .collect::<Html>();

    // 指板の下のフレット番号。実ギターのポジションマークと同じ位置
    let numbers = (0..=MAX_FRET)
        .map(|fret| {
            let text = if POSITION_MARKS.contains(&fret) {
                fret.to_string()
            } else {
                String::new()
            };
            html! { <div class="fb-number">{text}</div> }
        })
        .collect::<Html>();

    html! {
        <div class="fretboard-scroll">
            <div class="fb-grid">{rows}</div>
            <div class="fb-numbers">
                // 左端は弦名の列のぶん
                <div></div>
                {numbers}
            </div>
        </div>
    }
}
```

`identify` はまだ使わないので、この時点では `use chord_midi::fingering::identify;`
を消しておく。Task 8 で戻す。

- [ ] **Step 3: `main.rs` にタブと state を足す**

`mod diagram;` の下:

```rust
mod diagram;
mod fretboard;
```

`use` に足す:

```rust
use fretboard::Fretboard;
```

`Tab` enum:

```rust
/// 画面のタブ。プレイヤーはタブの外に置くので、切り替えても再生は止まらない
#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Editor,
    Preview,
    Fretboard,
}
```

`let tab = use_state(|| Tab::Editor);` の下に state を足す。タブは条件
レンダリングでアンマウントされるので、押さえた形は `App` 側で持つ:

```rust
    let tab = use_state(|| Tab::Editor);
    // 運指タブで押さえた形。タブを離れても消さないので App が持つ
    let picked = use_state(|| [None; 6] as [Option<u8>; 6]);
```

`on_preview_tab` の下にコールバックを足す:

```rust
    let on_fretboard_tab = {
        let tab = tab.clone();
        Callback::from(move |_| tab.set(Tab::Fretboard))
    };
    let on_pick = {
        let picked = picked.clone();
        Callback::from(move |next: [Option<u8>; 6]| picked.set(next))
    };
    // 記法をエディタ末尾に足す。進行プリセットのボタンと同じ挙動
    let on_insert = {
        let input = input.clone();
        Callback::from(move |notation: String| {
            let cur = (*input).clone();
            let next = if cur.trim().is_empty() {
                format!("{} |", notation)
            } else {
                format!("{} {} |", cur.trim_end(), notation)
            };
            input.set(next);
        })
    };
```

`let preview = ...` の下に足す:

```rust
    let fretboard = html! {
        <Fretboard frets={*picked} on_change={on_pick} on_insert={on_insert} />
    };
```

タブのボタンを足す（`プレビュー` のボタンの下）:

```rust
                <button
                    class={classes!("tab", (*tab == Tab::Fretboard).then_some("is-active"))}
                    onclick={on_fretboard_tab}
                >
                    {"運指"}
                </button>
```

`main` の中身を3分岐にする:

```rust
            <main class="panel">
                {
                    match *tab {
                        Tab::Editor => editor,
                        Tab::Preview => preview,
                        Tab::Fretboard => fretboard,
                    }
                }
            </main>
```

- [ ] **Step 4: 型が通ることを確認する**

Run: `cd chord_midi_web && trunk build`
Expected: 成功。`trunk` が無ければ
`cargo check -p chord_midi_web --target wasm32-unknown-unknown`

`[Option<u8>; 6]` が `Properties` の `PartialEq` を満たすことと、
`Callback<[Option<u8>; 6]>` が使えることを確認する。通らなければ
`#[derive(Properties, PartialEq)]` のままで型を `Vec<Option<u8>>` に
落とさず、配列のまま直すこと。

- [ ] **Step 5: コミット**

```bash
git add chord_midi_web/src/fretboard.rs chord_midi_web/src/diagram.rs chord_midi_web/src/main.rs
git commit -m "feat(web): 運指タブとクリックできる指板

6弦 x 0-12フレット。1弦1音で、同じところをもう一度押すとミュート。
押さえた形はタブを離れても消えないよう App が持つ。

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01RMaNBbqQuecYx8C4Agmr27"
```

---

## Task 8: 候補カードを出す

**Files:**
- Modify: `chord_midi_web/src/fretboard.rs`

- [ ] **Step 1: `identify` を呼んでカードを描く**

`chord_midi_web/src/fretboard.rs` の `use` を戻す:

```rust
use chord_midi::fingering::{identify, Candidate, Source};
```

`fretboard` 関数の中身を書き換える:

```rust
#[function_component(Fretboard)]
pub fn fretboard(props: &FretboardProps) -> Html {
    let candidates = identify(props.frets);
    let on_clear = {
        let on_change = props.on_change.clone();
        Callback::from(move |_| on_change.emit([None; 6]))
    };
    html! {
        <div class="fretboard-tab">
            <p class="fretboard-help">
                {"指板を押すとその弦の音が決まる。同じところをもう一度押すとミュート。"}
            </p>
            { board(props) }
            <div class="block">
                <button class="btn" onclick={on_clear}>{"クリア"}</button>
            </div>
            <div class="block">
                <span class="block-label">{"コードの候補"}</span>
                { results(&candidates, &props.on_insert) }
            </div>
        </div>
    }
}
```

`board` の下に足す:

```rust
fn results(candidates: &[Candidate], on_insert: &Callback<String>) -> Html {
    if candidates.is_empty() {
        return html! { <p class="empty">{"該当なし"}</p> };
    }
    let cards = candidates
        .iter()
        .map(|c| {
            let onclick = {
                let on_insert = on_insert.clone();
                let notation = c.notation.clone();
                Callback::from(move |_| on_insert.emit(notation.clone()))
            };
            let (badge, badge_class) = match c.source {
                Source::Data(_) => ("原典", "fb-badge is-data"),
                Source::Theory => ("計算", "fb-badge"),
            };
            // 原典での呼び名が rechord の名前と違うときだけ併記する。
            // diagram.rs の「≒」と同じ考え方
            let alias = match c.source {
                Source::Data(f) if f.name != c.name => html! {
                    <div class="fb-alias">{format!("原典では {}", f.name)}</div>
                },
                _ => Html::default(),
            };
            let note = if c.unreachable {
                html! {
                    <div class="fb-note">{"この形は今の押さえ方データでは再現できない"}</div>
                }
            } else {
                Html::default()
            };
            html! {
                <div class="fb-card">
                    <div class="fb-card-head">
                        <span class="fb-card-name">{&c.name}</span>
                        <span class={badge_class}>{badge}</span>
                    </div>
                    {alias}
                    <div class="fb-card-body">
                        <code class="fb-notation">{&c.notation}</code>
                        <button class="btn" {onclick}>{"挿入"}</button>
                    </div>
                    {note}
                </div>
            }
        })
        .collect::<Html>();
    html! { <div class="fb-cards">{cards}</div> }
}
```

- [ ] **Step 2: ビルドが通ることを確認する**

Run: `cd chord_midi_web && trunk build`
Expected: 成功

- [ ] **Step 3: コミット**

```bash
git add chord_midi_web/src/fretboard.rs
git commit -m "feat(web): 逆引きの候補カードと記法の挿入

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01RMaNBbqQuecYx8C4Agmr27"
```

---

## Task 9: CSS

**Files:**
- Modify: `chord_midi_web/style.css`

- [ ] **Step 1: `.diagram-empty` のルールの後ろに足す**

既存の変数（`--panel` `--line` `--ink` `--muted` `--accent` `--radius` `--mono`）
を使う。既存クラスは触らない。

```css
/* ---- 運指タブ（逆引き） ---- */

.fretboard-help {
    color: var(--muted);
    font-size: 12px;
    margin: 0 0 12px;
}

/* 12フレットぶんは横に長いので、狭い画面では横スクロールさせる */
.fretboard-scroll {
    overflow-x: auto;
    padding-bottom: 4px;
}

.fb-grid {
    display: grid;
    grid-template-columns: 56px repeat(13, 34px);
    grid-auto-rows: 28px;
    width: max-content;
}

.fb-label {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding-right: 8px;
    color: var(--muted);
    font-size: 11px;
    line-height: 1;
}

.fb-value {
    color: var(--ink);
    font-family: var(--mono);
    font-size: 12px;
}

/* 弦はセルの中央を横切る線、フレットはセルの右の縦線 */
.fb-cell {
    position: relative;
    cursor: pointer;
    border-right: 1px solid #8a9099;
    background: linear-gradient(#8a9099, #8a9099) 0 50% / 100% 1px no-repeat;
}

.fb-cell:hover {
    background-color: var(--hover);
}

/* 0列は開放弦。ナットの左なのでフレット線は引かない */
.fb-cell.is-open {
    border-right: none;
}

.fb-cell.is-nut {
    border-left: 3px solid var(--ink);
}

.fb-dot {
    position: absolute;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: var(--accent);
}

.fb-numbers {
    display: grid;
    grid-template-columns: 56px repeat(13, 34px);
    width: max-content;
    margin-top: 3px;
}

.fb-number {
    color: var(--muted);
    font-size: 10px;
    line-height: 1.2;
    text-align: center;
    font-variant-numeric: tabular-nums;
}

.fb-cards {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
}

.fb-card {
    min-width: 200px;
    padding: 8px 10px;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: var(--radius);
}

.fb-card-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
}

.fb-card-name {
    font-weight: 700;
    font-size: 15px;
}

.fb-badge {
    padding: 1px 6px;
    border-radius: 999px;
    background: var(--active);
    color: var(--muted);
    font-size: 10px;
    line-height: 1.6;
}

.fb-badge.is-data {
    background: var(--accent-soft);
    color: var(--accent);
}

.fb-alias {
    color: var(--muted);
    font-size: 11px;
}

.fb-card-body {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin-top: 6px;
}

.fb-notation {
    font-family: var(--mono);
    font-size: 13px;
}

.fb-note {
    margin-top: 4px;
    color: var(--danger);
    font-size: 11px;
}
```

- [ ] **Step 2: 見た目を確認する**

Run: `cd chord_midi_web && trunk serve`

ブラウザで開き、運指タブで次を確かめる:

- 指板が6行 × 13列で出る。1弦が上
- セルを押すと点が付き、左の弦名の横に数字が出る
- 同じセルをもう一度押すと `×` に戻る
- `x3201x`（5弦3fr / 4弦2fr / 3弦開放 / 2弦1fr）で候補の先頭が `C`、バッジが「原典」
- 「挿入」を押すとエディタタブの入力に `C |` が足されている
- ウィンドウを狭めても指板が横スクロールするだけでレイアウトが崩れない

- [ ] **Step 3: コミット**

```bash
git add chord_midi_web/style.css
git commit -m "style(web): 運指タブの指板と候補カード

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01RMaNBbqQuecYx8C4Agmr27"
```

---

## Task 10: README

**Files:**
- Modify: `README.md`

- [ ] **Step 1: 「押さえ方データ」の節の前に節を足す**

```markdown
## 逆引き（運指タブ）

web 版の「運指」タブでは、指板をクリックして作った押さえ方から、それが何の
コードで rechord 記法ではどう書くのかを引ける。

- 指板を押すとその弦の音が決まる。1弦につき1音で、同じところをもう一度押すと
  その弦はミュートになる
- 候補は確度の高い順に最大5件。バッジの「原典」は押さえ方データに同じ形がある
  もの、「計算」は鳴っている音から名前を出しただけのもの
- 「挿入」を押すと、その記法がエディタの入力末尾に足される

候補の記法を書いてもプレビュータブで同じ形にならないときは、カードにその旨が
出る。押さえ方データに無い形は記法で名指しできないため。

逆引きは CLI にはまだ無い。
```

- [ ] **Step 2: コミット**

```bash
git add README.md
git commit -m "docs: 運指タブ（逆引き）の説明

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01RMaNBbqQuecYx8C4Agmr27"
```

---

## Task 11: 仕上げの確認

- [ ] **Step 1: workspace のテストを全部流す**

Run: `cargo test`
Expected: PASS

- [ ] **Step 2: フォーマットと lint**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings`
Expected: 差分なし、警告なし

clippy が既存コードで落ちるなら、今回触ったファイルの警告だけ直して、
既存の警告は放置する（無関係な修正を混ぜない）。

- [ ] **Step 3: web のリリースビルド**

Run: `cd chord_midi_web && trunk build --release`
Expected: 成功

- [ ] **Step 4: 差分を見直す**

Run: `git log --oneline main..HEAD` と `git diff main...HEAD --stat`

`chord_midi/src/fingering/data.rs` に差分が無いこと（生成物なので手で触らない）。

- [ ] **Step 5: 直しがあればコミット**

```bash
git add -A
git commit -m "chore: fmt と clippy

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01RMaNBbqQuecYx8C4Agmr27"
```

---

## つまずきそうなところ

**`Modifier` が `Copy` でない。** `quality.iter().cloned()` を使う。

**`BTreeSet` の順が記法の順を決める。** `Modifier` の enum 宣言順がそのまま
`Display` の並び順になる。`Minor(7)` は `Flat5th` より前なので `Cm7b5` と出る。
並びが変なら `Modifier` の宣言順を疑うが、**既存の順は変えないこと**（`Power` と
`Octave` が末尾にあるのは「他の修飾を上書きする」ためで、動かすと壊れる）。

**`Dominant(5)` はパワーコードではない。** `[1,3,5]` のメジャートライアド。
パワーコードの記法は `pow(X)` だけ。

**`chord_ref()` はラベルから `@N` を落とす。** `Candidate.name` に `@N` は
入らない。記法 (`notation`) にだけ入る。

**`find()` の候補は「余計な音1つまで」。** 往復検証が思ったより通らないときは、
`find()` が別の押さえ方を選んでいる。`find()` の採点は変えないこと。変えると
プレビュータブの表示が全部変わる。

**`identify` は毎レンダリングで走る。** 216通り × ビット演算なので問題ないが、
`use_memo` を足したくなったら `props.frets` を鍵にすること。
