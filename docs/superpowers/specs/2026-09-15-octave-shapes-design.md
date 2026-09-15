# オクターブ奏法の関数 oct() と押さえ方 設計 (Issue #28)

日付: 2026-09-15
Issue: https://github.com/wakame-tech/chord-midi/issues/28

## 目的

6弦と4弦（5弦を飛ばす）で同じ音を1オクターブ差で鳴らす**オクターブ奏法**の形が、今の記法では書けない。

```
6弦5fr + 4弦7fr  ->  A2 + A3   (12半音 = オクターブ)
6弦4fr + 4弦6fr  ->  G#2 + G#3
6弦9fr + 4弦11fr ->  C#3 + C#4
```

`pow()` はルート + 5th を出すのでこの形にはならず、押さえ方データにもオクターブ形が無い。`pow()` と対称な `oct()` を足す。

## 決定事項 (ユーザー確定)

| 論点 | 決定 |
| --- | --- |
| 記法 | `pow()` と対称な `oct( <小節列> )` |
| 構成音 | **ルート1音だけ** |
| 入れる形 | 6弦ルート12キー + 5弦ルート12キー = 24形 |
| 運指 | ルート = 人差し指 / オクターブ = 薬指（`pow` と同じ） |
| データファイル | `docs/power_chords.json` を `docs/movable_shapes.json` に改名し `kind` を持たせる |
| `Fingering` の型 | `is_power: bool` を `kind: ShapeKind` に変える |

## 記法

```
oct( <小節列> )                  例: oct(G#@6 | A@6 | C#@6)
oct(A@6)                        6弦5fr + 4弦7fr
pat("o.oo..oo", oct(A@6))       ネストは pow と同じ
```

`@N` は既存どおり「最低音をこの弦で取る」。`oct(A@6)` は6弦ルート、`oct(A@5)` は5弦ルートの形になる。

## AST

`Func` に `Oct` を足す。

```rust
pub enum Func {
    Pow,
    Oct,
    Pat(Pattern),
}
```

`Display` は `oct(...)` を再出力する。`pow` と同じく `Ast::Apply` の3つ目の `bool`（後ろで改行されるか）も持つ。

## 構成音

`Modifier::Octave` を足す。

```rust
pub enum Modifier {
    // ... 既存 ...
    Power,
    /// オクターブ奏法。ルートだけを鳴らす
    Octave,
}
```

`Chord::modify` で `Modifier::Octave` は `semitones` を `{0}` に**置き換える**（`clear()` してから挿入）。enum の**末尾**（`Power` の後ろ）に置き、`BTreeSet` の順で最後に適用させて他の修飾を上書きする。

`Display` は `"oct"` とする。押さえ方データのオクターブ形の `name` も `"Aoct"` の形にして、カードのラベルと一致させる。一致していないと「≒」の併記が誤って出る（`@N` で同じ問題を踏んだ）。

`resolve()` での脱糖は `pow` と同じ経路。`Apply(Func::Pow, ..)` を潰して `Modifier::Power` を足す処理を、`Func::Oct` なら `Modifier::Octave` を足すように一般化する。

## 押さえ方データ

オクターブ形24件。

```
6弦ルート : 6弦 n fr + 4弦 (n+2) fr      40+n と 52+n  -> 12半音
5弦ルート : 5弦 n fr + 3弦 (n+2) fr      45+n と 57+n  -> 12半音
```

`n` は `(ルートのピッチクラス - 開放弦のピッチクラス) mod 12` で 0〜11。相方は 2〜13フレット。

間の弦（6弦ルートなら5弦）はミュートなので、図では `×` が出る。実際の奏法でもそこは指の腹で触れて止める。

### データファイルの一般化

`docs/power_chords.json` を `docs/movable_shapes.json` に改名し、各形に `kind` を持たせる。

```json
{
  "shapes": [
    { "kind": "power",  "root": "A", "string": 6, "frets": [5, 7, null, null, null, null] },
    { "kind": "octave", "root": "A", "string": 6, "frets": [5, null, 7, null, null, null] }
  ]
}
```

ファイルを増やすより1つにまとめる。`scripts/gen_fingerings.py` が読むのは `easy_guiter_chords.json` とこの2つのまま。

## Fingering の型

`is_power: bool` を `kind: ShapeKind` に置き換える。

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeKind {
    /// 原典の押さえ方
    Normal,
    /// 可動パワーコード形
    Power,
    /// オクターブ形
    Octave,
}
```

`bool` を2つ並べるより素直で、`find` のガードも読みやすい。

## find のガード

### ピッチクラスからの推測をやめる

今の `find` は「ターゲットの構成音がルート + 5th ちょうどか」を見てパワーコード形を解放している。つまり**書かれた関数ではなくピッチクラスから推測**している。オクターブ形を足すとこれが破綻する。

`oct(A/E)` を考える。構成音は `{A}` だがスラッシュのベース E が `pitch_classes` に足されるので `{A, E}` になり、これは**パワーコードのターゲット条件と同じ**になってしまう。結果、オクターブ形ではなくパワーコード形が選ばれる。

そこで推測をやめ、**書かれた関数をそのまま持ち回す**。`Modifier::Power` / `Modifier::Octave` が `ChordNode` に付いているので、`chord_ref` がそれを読んで `ChordRef` に持たせる。

```rust
pub struct ChordRef {
    pub label: String,
    pub root: u8,
    pub pitch_classes: u16,
    pub bass: Option<u8>,
    pub root_string: Option<u8>,
    /// `pow()` / `oct()` で明示された押さえ方の種類
    pub wanted: Option<ShapeKind>,
}
```

```rust
let wanted = if node.modifiers.contains(&Modifier::Octave) {
    Some(ShapeKind::Octave)
} else if node.modifiers.contains(&Modifier::Power) {
    Some(ShapeKind::Power)
} else {
    None
};
```

### ガードの規則

```rust
// Normal は常に候補。可動形 (Power / Octave) は、書かれた関数で種類が
// 明示されていればその種類だけ、明示が無ければ @N の指定があるときだけ
let allowed = |kind| match kind {
    ShapeKind::Normal => true,
    other => match chord.wanted {
        Some(wanted) => wanted == other,
        None => chord.root_string.is_some(),
    },
};
```

`wanted` は「この種類の押さえ方が欲しい」という明示なので、`@N` より優先する。`||` で繋ぐと弦指定を付けた瞬間に種類の指定が無効になり、`oct(A/E@6)` の構成音 `{A, E}` がパワーコード形と完全一致するためパワー形が選ばれてしまう。

`Normal` を常に許すのは既存どおり。`pow(C)` が原典の2音の形（`x-3-x-0-1-x` = C + G）を引くのはこの経路で、可動形より低いポジションなので妥当な結果として受け入れている。

関数の指定が無く `@N` だけがあるときは Power と Octave の両方が候補になるが、コストで自然に分かれる。

| 入力 | パワー形のコスト | オクターブ形のコスト | 選ばれる |
| --- | --- | --- | --- |
| `pow(A@6)` = {A, E} | 欠け0 余分0 = **0** | 5thが欠けて 1 | パワー形 |
| `oct(A@6)` = {A} | 5thが余分で 2 | 欠け0 余分0 = **0** | オクターブ形 |

### 既存の振る舞いへの影響

ピッチクラスからの推測をやめることで、`pow()` と書いていないのに構成音がたまたまルート + 5th になるコード（例 `Comit3`）は、可動パワーコード形ではなく原典の形を引くようになる。押さえ方としては同じ音が鳴るローポジションの形なので実害は無い。

既存のテストはすべて `pow(...)` と書いた入力か、押さえ方が明示されていない通常のコードなので、結果は変わらない。

## MIDI

Issue #26 で押さえ方の実音を鳴らすようにしたので、`oct(A@6)` は自動で A2 + A3 が鳴る。MIDI 側の追加変更は要らない。

押さえ方が見つからなかった場合のフォールバックは `into_note_numbers` で、構成音が `{0}` なので1音だけ鳴る。

## 既存コードで触る箇所

| ファイル | 変更 |
| --- | --- |
| `chord_midi/src/model/ast.rs` | `Func::Oct` |
| `chord_midi/src/model/modifier.rs` | `Modifier::Octave`、`Display` |
| `chord_midi/src/model/chord.rs` | `modify` に `Octave` の腕 |
| `chord_midi/src/import/rechord.rs` | `oct(` のパーサ |
| `chord_midi/src/export/rechord.rs` | `Ast::Apply` の `Display` に `Func::Oct` |
| `chord_midi/src/model/transform.rs` | `resolve` の脱糖を `Pow` / `Oct` 共通にする |
| `chord_midi/src/export/midi.rs` | `Ast::Apply` の match に `Func::Oct` |
| `chord_midi/src/fingering/mod.rs` | `ShapeKind`、`ChordRef` に `wanted`、`find` のガード |
| `docs/movable_shapes.json` | `power_chords.json` から改名、`kind` 追加、オクターブ形24件追加 |
| `scripts/gen_fingerings.py` | 新ファイル名、`kind` の出力、オクターブ形の運指 |
| `README.md` | `oct()` の記法 |

## テスト

**パーサと Display**
- `oct(A)` / `oct(A@6 | D@5)` / `pat("oxox", oct(A@6))` が読める
- `oct(A@6 | D@5)` をパースして `Display` すると同じ文字列に戻る
- `shift_key("oct(A@6)", 1)` が `oct(A#@6)` になり `-1` で戻る

**構成音**
- `oct(A)` の `semitones` が `{0}`
- `oct(Am7)` も `{0}`（元の修飾を上書きすること）
- `oct(A/E)` でベースの E が残ること

**データ**
- `FINGERINGS` の件数が 67 + 24 + 24 = 115
- オクターブ形24件すべてで、鳴る音が1ピッチクラス・2音・音程がちょうど12半音
- オクターブ形の運指が 人 + 薬（開放ルートの形は薬のみ）

**find**
- `oct(A@6)` が 6弦5fr + 4弦7fr、`oct(A@5)` が 5弦0fr + 3弦2fr
- `oct(G#@6)` が 6弦4fr + 4弦6fr、`oct(C#@6)` が 6弦9fr + 4弦11fr
- `pow(A@6)` は従来どおりパワー形が選ばれること（オクターブ形に取られない）
- `oct(A/E)` でオクターブ形が選ばれること（ベースが足された構成音がパワーコードの条件と一致してしまう問題の回帰テスト）
- **既存の結果が変わらないこと**: `C` `Am7` `D#m7` `C#sus4` `G#m`

**MIDI**
- `oct(A@6)` が `[45, 57]`（A2 + A3）になる
- `oct(G#@6)` が `[44, 56]`、`oct(C#@6)` が `[49, 61]`

## スコープ外 (YAGNI)

- 4弦ルートのオクターブ形。2弦とのペアは +3 フレットになり規則が変わる
- 3本目を足したオクターブ形（ルート + オクターブ + 5th など）
- ミュートする弦のブラッシング音
