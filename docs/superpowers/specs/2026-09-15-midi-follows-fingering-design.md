# MIDI を押さえ方に合わせる 設計 (Issue #26)

日付: 2026-09-15
Issue: https://github.com/wakame-tech/chord-midi/issues/26

## 目的

押さえ方プレビューが表示している形と、MIDI で鳴っている音が一致していない。

`pat("o.oo..oo", pow(A@6 | D@5 | E@5))` を入れて実測した結果。

| | ギターで鳴る音 | MIDI で鳴っている音 | ずれ |
| --- | --- | --- | --- |
| `A5@6` (6弦5fr + 5弦7fr) | A2 + E3 | E4 + A4 | +19半音、しかも転回（5度 → 4度） |
| `D5@5` (5弦5fr + 4弦7fr) | D3 + A3 | D4 + A4 | +12半音 |
| `E5@5` (5弦7fr + 4弦9fr) | E3 + B3 | B3 + E4 | +7半音、転回 |

「この形で押さえろ」と表示しておきながら、鳴らしているのは別の声部配置になっている。ギター向けのアプリとして筋が通らない。

## 原因

押さえ方の照合と MIDI の音程が別々に決まっている。

- `fingering::find` は**ピッチクラスだけ**で照合する。オクターブも転回も見ていない（意図してそう設計した）
- MIDI 側は `Chord::octave` と `match_pitches` が「基準音に一番近いオクターブ・転回」を選ぶ。ギターのことは何も知らない

## 決定事項 (ユーザー確定)

押さえ方が見つかった場合は、**その押さえ方の実音を鳴らす**。見つからなければ従来の声部配置に落とす。

検討して見送った案:

| 案 | 見送った理由 |
| --- | --- |
| `pow` のときだけ転回を止める | 4度→5度は直るがオクターブのずれが残る |
| 全体をギターの音域に下げる | 声部配置は相変わらず一致しない |
| 現状のまま仕様として書く | 表示と音が違うことがこのアプリで一番まずい |

## 変更の中心

`chord_midi/src/export/midi.rs` に「`ChordNode` → MIDI ノート番号」を1箇所に集約する。

```rust
/// 開放弦の MIDI ノート番号。6弦→1弦 (E2 A2 D3 G3 B3 E4)
const OPEN_STRING_NOTES: [u8; 6] = [40, 45, 50, 55, 59, 64];

/// 押さえ方が見つかればその実音を鳴らす。見つからなければ従来の声部配置に落とす
fn voicing(node: &ChordNode) -> Result<Vec<NoteNumber>> {
    if let Some(f) = fingering::find(&fingering::chord_ref(node)?) {
        return Ok(f
            .frets
            .iter()
            .enumerate()
            .filter_map(|(i, v)| v.map(|fret| NoteNumber::new(OPEN_STRING_NOTES[i] + fret)))
            .collect());
    }
    into_note_numbers(&node.to_chord()?)
}
```

`fingering::chord_ref` を `pub` にする。`export` → `fingering` の依存が増えるが、同じクレート内なので循環しない。

音は 6弦→1弦 の順に並ぶ（低い方から）。アルペジオはこの順で鳴らす。

## `Score::pre` の型を変える

今は `Option<Chord>` を持ち、鳴らす直前に `into_note_numbers` で音程へ変換している。これを `Option<Vec<NoteNumber>>`（解決済みの声部）に変える。

```rust
struct Score {
    notes: Vec<Note>,
    sustain: u32,
    rest: u32,
    /// 直前に鳴らした声部。`=` と `%` が伸ばす対象
    pre: Option<Vec<NoteNumber>>,
}
```

こうすると `Node::Chord` / `Arpeggio` / `Brush` / `Transform` / `Repeat` / `Sustain` が全部同じ経路になり、`voicing` の呼び出しが `interpret_node` の中の数箇所に収まる。サステインの吐き出し（`interpret_node` の先頭と `interpret` の Score 末尾）は `pre.clone()` を使うだけになる。

## 期待する結果

| 入力 | 今 | 変更後 |
| --- | --- | --- |
| `pow(A@6)` | E4 + A4（4度・+19半音） | **A2 + E3**（押さえ方どおり） |
| `C` | C4 E4 G4（3音） | **C3 E3 G3 C4**（`x3201x` の4音） |
| `arp(C)` | 3音を順に | 6弦側から4音を順に |

## 意図した副作用

- **全入力の MIDI 出力が変わる**。押さえ方のカバレッジがほぼ100%なので、実質すべてがギターの声部配置になる
- **音域が下がる**。ギターの実音は記譜より1オクターブ低いので、今まで C4 中心だったのが C3 中心になる
- スラッシュコードのベースは押さえ方の最低音がそのまま使われる。`find` がベース一致を優先するので大抵合うが、一致しない押さえ方が選ばれることもある。その場合も「その形を押さえたときに鳴る音」なので表示とは一致する

## 既存テストへの影響

`export/midi.rs` のテストは音程ではなく音数と尺をアサートしている。壊れるのは1件。

- `test_block_chord_is_simultaneous` が `C` を3音と期待している。`x3201x` は4音になるので期待値を直す

`test_arpeggio_durations_sum_to_slot` と `test_arpeggio_dense_measure_no_overrun` は音数に依存するが、尺の合計が `dur` になる作りは変わらないので通る。

## テスト

**押さえ方どおりに鳴ること**
- `C` が `[48, 52, 55, 60]`（C3 E3 G3 C4 = `x3201x`）になる
- `pow(A@6)` が `[45, 52]`（A2 E3 = 6弦5fr + 5弦7fr）になる
- `pow(D@5)` が `[50, 57]`（D3 A3 = 5弦5fr + 4弦7fr）になる
- `pow(E@5)` が `[52, 59]`（E3 B3 = 5弦7fr + 4弦9fr）になる
- 音が低い方から順に並ぶこと

**フォールバック**
- `into_note_numbers` が従来どおり動くこと（押さえ方が無いコード用に残す）

**既存の振る舞いが保たれること**
- `arp(C)` が1音ずつ鳴り、尺の合計が小節1つぶんのまま
- `=` と `%` が直前の声部を伸ばす
- `pat` のリズムとブラッシングのベロシティが変わらない

## スコープ外 (YAGNI)

- ストローク（弦を順にずらして鳴らす）。全弦同時発音のまま
- 押さえ方を使うかどうかの切り替えフラグ。このアプリはギター向けなので常に使う
- ミュートした弦のブラッシング音
