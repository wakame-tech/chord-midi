# ノードに掛かる関数の型付けと入れ子

- 日付: 2026-09-16
- 対象: `chord_midi`, `chord_midi_web`, `chord_midi_cli`

## 背景

`arp(blk(I))` が書けない。

```rust
map(delimited(tag("arp("), chord_node_parser, tag(")")), Node::Arpeggio)
```

`arp(` の中身が `chord_node_parser`(裸のコード)で、`node_parser` を再帰していない。AST も
`Node::Arpeggio(ChordNode)` / `Node::Transform(Transform, ChordNode)` と、`Node` ではなく
`ChordNode` を持つので、パーサを直しても型が入れ子を受けられない。

一方で `interpret_node` と `collect_node` は既に `Node::Transform` の腕で自分を呼び直しており、
展開そのものは再帰的に動く。詰まっているのはパーサと型の2点だけ。

## 型で決まる

ノードに掛かる関数は、返すものが2種類ある。

| | 返すもの |
|---|---|
| `blk` `sub` `dom` | コード1つ |
| `s25` | **コード2つ** (`Dm7 G7`) |
| `arp` | 演奏法 (コードではない) |

今は4つとも `Transform::expand(ChordNode) -> Vec<Node>` で一緒くたになっていて、「1つ返すことも
あれば2つ返すこともある」曖昧な型になっている。これを型で分ける。

```rust
/// コードを返す式。入れ子にできる
pub enum ChordExpr {
    /// 素のコード
    Chord(ChordNode),
    /// コード1つを別のコード1つに変える
    Map(ChordMap, Box<ChordExpr>),
}

/// Chord -> Chord
pub enum ChordMap {
    /// ブラックアダーコード
    Blk,
    /// 裏コード化
    Sub,
    /// ドミナント化
    Dom,
}

pub enum Node {
    /// 鳴らす
    Chord(ChordExpr),
    /// アルペジオで鳴らす
    Arpeggio(ChordExpr),
    /// ブラッシング。`pat` の `x` から作られる (パーサは作らない)
    Brush(ChordExpr),
    /// ツーファイブ分割。自分の尺を2つに割る
    Split(ChordExpr),
    Rest,
    Sustain,
    Repeat,
}
```

`arp` と `s25` は `Node` を返すので、`ChordExpr` の位置に置けない。**入れ子の可否が型から落ちる。**

| 書ける | 書けない |
|---|---|
| `arp(blk(I))` | `arp(s25(V7))` — `s25` は `Node` |
| `blk(sub(V7))` | `blk(arp(I))` — `arp` は `Node` |
| `s25(blk(I))` | `s25(s25(V7))` |
| `arp(blk(sub(V7)))` | `blk(s25(V7))` |

**すべてパースエラーになる。** 実行時検査ではなく、パーサが組み立てられない。`arp` が最も外側にしか
来ないのも型の帰結であって、特別扱いではない。

## `s25` を `Node` に残す理由

`s25` を「パース時に2ノードへ脱糖する」形にはできない。尺の配分が変わる。

```
C s25(G7) |   ->  C が 1/2、s25 の 1/2 が Dm7(1/4) G7(1/4) に割れる
C Dm7 G7 |    ->  3 等分
```

実測でこの2つは別の MIDI になる。`s25` は「自分の尺を2つに割る」ノードなので、
`Node::Split` として残す。

## 既存の記法への影響

**記法は変わらない。** `arp(C)` / `blk(F)` / `s25(G7)` / `sub(G7)` / `dom(Dm7)` はそのまま書ける。
書けるものが増えるだけで、減るものは無い。

**MIDI 出力は変わらない。** ゴールデン27件が1バイトも変わらないことが受け入れ条件。

**JSON の形は変わる。** `Node::Transform(Blk, chord)` が `Node::Chord(Map(Blk, Chord(chord)))` に
なる。web の「解釈結果」の『構文』『展開後』の見た目が変わるが、これは意図した変更。

## 波及範囲

| ファイル | 変更 |
|---|---|
| `chord_midi/src/model/ast.rs` | `ChordExpr` / `ChordMap` の追加、`Node` の作り替え。`Transform` は廃止 |
| `chord_midi/src/import/chord.rs` | `node_parser` の再帰、`chord_expr_parser` の新設 |
| `chord_midi/src/export/rechord.rs` | `ChordExpr` の Display |
| `chord_midi/src/export/midi.rs` | `interpret_node` の腕、`interpret_patterned_measure` の事前展開 |
| `chord_midi/src/fingering/mod.rs` | `collect_node` の腕 |
| `chord_midi/src/model/transform.rs` | `into_pitch` / `into_degree` / `mark` が `ChordExpr` へ潜る。`Transform::expand` を `ChordMap::apply` と `Split` の展開に分ける |
| `chord_midi/src/edit.rs` | `shift_node` が `ChordExpr` へ潜る |
| `chord_midi/src/reference.rs` | 記法表に入れ子の例を足す |
| `chord_midi_web` | `MeasureRef` 経由なので原則無変更。`cargo check` で確認 |

## テスト

**ゴールデン (最重要)**: 既存27件が1バイトも変わらないこと。記法も MIDI も変えない変更なので、
1件でも動いたら壊している。

**等価テスト**: `blk(F)` が `Baug/F` とバイト同一であることを利用して、入れ子の結果を手で書いた形と
突き合わせる。Phase 2 の `defines_match_hand_expansion` と同じ検証。

| 入れ子 | 手で書いた形 |
|---|---|
| `arp(blk(F))` | `arp(Baug/F)` |
| `blk(sub(G7))` | `sub(G7)` の結果 (Db7) に blk を掛けた形 |
| `arp(blk(sub(G7)))` | 同上を arp |
| `s25(blk(I))` | `blk(I)` の結果を s25 した形 |

**型エラー**: `arp(s25(V7))` / `blk(arp(I))` / `s25(s25(V7))` / `blk(s25(V7))` が
パースエラーになること。

**記法表**: `every_example_parses` が入れ子の例を含めて通ること。

## やらないこと

- **`pow` / `oct` は触らない。** 小節列に掛かる別の層で、今回の型付けの対象外
- **`arp` の入れ子** (`arp(arp(I))`)。`arp` は `Node` を返すので型で弾かれる。特別扱いしない
- **新しい変換関数の追加**
