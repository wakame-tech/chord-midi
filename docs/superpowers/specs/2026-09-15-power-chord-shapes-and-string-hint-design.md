# 可動パワーコードの押さえ方と弦指定記法 @N 設計 (Issue #24)

日付: 2026-09-15
Issue: https://github.com/wakame-tech/chord-midi/issues/24

## 目的

`pow(A)` と書いてもプレビューに出るのは開放弦を使ったローポジションの代用形で、ギターで実際に使う可動パワーコード形が出ない。押さえ方データが daichan の超簡単コード67件しか無く、可動形が入っていないため。

同じ `A5` でも 6弦ルート（5fr）と 5弦ルート（開放）で押さえ方が違う。どちらで取るかを指定する手段も無い。

この2つを埋める。

## 決定事項 (ユーザー確定)

| 論点 | 決定 |
| --- | --- |
| 入れる形 | **6弦ルート12キー + 5弦ルート12キー = 24形**。4弦ルートは入れない |
| 弦の指定方法 | **rechord 記法に `@N` を足す**。Rust 側で動的生成する案は採らない |
| パワーコードの運指 | **人（ルート）+ 薬（5th）** の特例にする |
| データの置き場所 | `docs/power_chords.json` を新設し、生成時に合流させる |

## 記法

`@N`（N = 1〜6）をコードの末尾に付ける。**最低音をこの弦で取る**という押さえ方への指示で、音程には影響しない。

```
pow(A@6 | D@5 | E@5)
C@5 |
A/E@6 |
```

`/on` より後ろに置く。`A@6/E` ではなく `A/E@6`。

`@N` は押さえ方検索にしか効かない。MIDI の音程は今までどおりコードの構成音だけで決まる。

「ルートの弦」ではなく「最低音の弦」と定義する理由: `Fingering` はコードのルートを知らない（`name` は表示用で照合には使わない）が、最低音が鳴る弦は `frets` から機械的に求まる。パワーコードではルート = 最低音なので、この定義で意図どおりに動く。

## AST とパーサ

```rust
pub struct ChordNode {
    pub key: Key,
    pub modifiers: BTreeSet<Modifier>,
    pub on: Option<Key>,
    /// `@N` で指定された「最低音を取る弦」。1 = 1弦, 6 = 6弦
    pub root_string: Option<u8>,
}
```

`chord_node_parser` の `opt(preceded(tag("/"), key_parser))` の後ろに `opt(preceded(tag("@"), one_of("123456")))` を足す。

`ChordNode` の `Display` は `on` の後ろに `@6` を出す。これでキー移調がテキストを書き戻しても `@N` が消えない。

## データ

`docs/power_chords.json` を新設する。

```json
{
  "description": "可動パワーコード形。ルートの隣の弦の2フレット上が5th。",
  "shapes": [
    { "root": "A", "string": 6, "frets": [5, 7, null, null, null, null] }
  ]
}
```

12キー × 2弦 = 24件。ルート弦のフレットは `(root_pc - 開放弦の pc) mod 12` で、0 になるキー（6弦なら E、5弦なら A）は開放弦の形になる。

`scripts/gen_fingerings.py` が `easy_guiter_chords.json` と `power_chords.json` の両方を読んで `data.rs` を作る。`easy_guiter_chords.json` は daichan からの抽出結果のまま保ち、手書きのデータを混ぜない。

## Fingering に足すフィールド

```rust
pub struct Fingering {
    // ... 既存 ...
    /// 最低音が鳴る弦。0 = 6弦, 5 = 1弦
    pub lowest_string: u8,
    /// 可動パワーコード形かどうか
    pub is_power: bool,
}
```

`lowest_string` は既存67件も `frets` から算出して埋める（最初に `Some` になる添字）。

## find のガード

可動パワーコード形は2音しか無いので、無条件に候補へ入れると3rd や7th が欠けた形が本来のコードを押しのける。12ルート × よく使う10種類 = 120コードで実測したところ、ガード無しでは **9件が劣化**した。

```
D#m7   x3x34x (Cm7 の形)  ->  x68xxx (D#5)     3rd も b7 も無くなる
D#m    x3x34x             ->  x68xxx (D#5)
C#sus4 x1x12x             ->  x46xxx (C#5)     4th が無くなる
```

そこで2段のガードを入れる。

1. `is_power` の形は、**ターゲットの構成音が root と 5th ちょうど**（`pitch_classes == (1 << root) | (1 << ((root + 7) % 12))`）か、**`@N` 指定がある**ときだけ候補にする
2. `@N` 指定があるときは、`lowest_string` が一致する形だけに絞る

このガードを入れると既存120コードの結果は **1件も変わらない**ことを実測で確認済み。

`@N` を指定したのに一致する形が無い場合は `None` を返し、プレビューは「データなし」になる。

## ChordRef

```rust
pub struct ChordRef {
    pub label: String,
    pub root: u8,
    pub pitch_classes: u16,
    pub bass: Option<u8>,
    /// `@N` の指定。0 = 6弦, 5 = 1弦 に正規化して持つ
    pub root_string: Option<u8>,
}
```

記法は 1 = 1弦 だが、`Fingering.lowest_string` は配列の添字（0 = 6弦）に合わせてある。`chord_ref` で `6 - n` に変換して持つ。

## 運指

パワーコード形は **人（ルート）+ 薬（5th）** にする。既存の「押さえる音をフレット昇順 → 弦番号 6弦→1弦 の順に並べ、人 → 中 → 薬」という規則だと2音のときに 人 + 中 になり、実際の運指とずれる。

`gen_fingerings.py` の運指導出に「可動パワーコード形は 人 + 薬」という分岐を足す。開放弦がルートになる形（E5 の `02xxxx`、A5 の `x02xxx`）は押さえる音が1つなので、既存の「1音だけなら中指」ではなく **薬指** にする（人差し指はミュートに使うという原典の説明と、パワーコードの運指の両方に合う）。

## 期待する結果

```
A@6  ->  57xxxx   6弦5fr(人) + 5弦7fr(薬)
D@5  ->  x57xxx   5弦5fr(人) + 4弦7fr(薬)
E@5  ->  x79xxx   5弦7fr(人) + 4弦9fr(薬)
E@6  ->  02xxxx   6弦開放 + 5弦2fr(薬)
```

弦指定なしの `pow(...)` では、コストが同じならローポジション優先の既存 tie-break により開放弦を使う形が選ばれる（`E5` → `02xxxx`、`A5` → `x02xxx`）。

daichan のデータにも2音だけの形がいくつかあり（`C` の `x3x01x` = C+G、`D` の `xx023x` = D+A、`G` の `3x003x` = G+D）、これらは `is_power` ではないのでガードに関係なく候補に残る。`pow(C)` はこの `x3x01x` を引くのでカードに `C5 (≒ C)` と出る。可動形より低いポジションで鳴る音も同じなので、これは妥当な結果として受け入れる。

## 既存コードで触る箇所

| ファイル | 変更 |
| --- | --- |
| `chord_midi/src/model/ast.rs` | `ChordNode` に `root_string` |
| `chord_midi/src/import/chord.rs` | `@N` のパーサ |
| `chord_midi/src/export/rechord.rs` | `ChordNode` の `Display` に `@N` |
| `chord_midi/src/fingering/mod.rs` | `Fingering` に2フィールド、`ChordRef` に `root_string`、`find` のガード |
| `chord_midi/src/model/transform.rs` | `ChordNode` を組み立てている箇所（`transform_chord`）に `root_string: None` |
| `docs/power_chords.json` | 新規 |
| `scripts/gen_fingerings.py` | 両方の JSON を読む、運指の特例、新フィールドの出力 |

`ChordNode` にフィールドを足すので、構造体リテラルで組み立てている箇所すべてに `root_string: None` が要る。

## テスト

**パーサと Display**
- `A@6` / `A/E@6` / `pow(A@6 | D@5)` が読める
- `@0` と `@7` はエラー
- `A@6` をパースして `Display` すると `A@6` に戻る
- `shift_key("pow(A@6)", 1)` が `pow(A#@6)` になり、`-1` で戻る

**データ**
- `FINGERINGS` の件数が 67 + 24 = 91
- 可動パワーコード形24件すべてで、鳴る音が `{root, 5th}` の2音
- 可動パワーコード形の運指が 人 + 薬（開放ルートの形は薬のみ）
- 全件で `lowest_string` が `frets` の最初の `Some` の添字と一致

**find**
- `A@6` → `57xxxx`、`D@5` → `x57xxx`、`E@5` → `x79xxx`
- 弦指定なしの `pow(E)` → `02xxxx`、`pow(A)` → `x02xxx`
- **既存の結果が変わらないこと**: `C` → `x3201x`、`Am7` → `x0201x`、`D#m7` → `x3x34x`、`C#sus4` → `x1x12x`
- `@N` に一致する形が無いとき `None`（例: `C@1`）

## スコープ外 (YAGNI)

- 4弦ルートのパワーコード形
- パワーコード以外での `@N` の活用（3和音のポジション違いを持つ）。記法としては書けるが、データに同じコードの別ポジションが無いので実質パワーコードにしか効かない
- オクターブを足した3音のパワーコード形
- `A5` という記法をパワーコードとして解釈すること（今も `Dominant(5)` = メジャートライアド）
