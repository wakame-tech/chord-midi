# tab 記法: 押さえ方を直接書く

2026-10-18

## 背景

指板パネルで押さえた形を記法に落とすとき、今はコード名 (`pow(G#@6)` や
`G#/C`) を経由する。コード名から押さえ方を引き直すのは `fingering::find`
で、原典の67形 + 可動形24形からの**近似**なので、元の形が戻るとは限らない。

戻らない例:

```
押さえた形   6弦8 / 5弦6 / 4弦6   C3  D#3 G#3   = G# のトライアド、C が最低音
判定         G#/C
図と MIDI    A♭△7 (6弦4/4弦5/3弦5) G#2 G3  C4
```

共通する実音はゼロ。5th が落ち M7 が増え、最低音も変わる。これは重み付けの
問題ではなく、**データに G# のトライアド形が無い**ことによる。候補カードの
「この形は今の押さえ方データでは再現できない」もこれが理由で出ている。

コード名を介さず押さえ方そのものを書ける記法があれば、この経路は消える。

## 記法

```
tab(8 6 6 x x x)
```

- 6弦→1弦の順。`x` = ミュート、`0` = 開放弦
- トークンはちょうど6個。過不足はパースエラー
- 各トークンは `x` か 0〜24 の10進。範囲外はパースエラー
- 区切りは空白1つ以上
- `node_parser` の枝として足す。コードが書ける場所ならどこにでも置ける

例:

```
key: C
C | Am | tab(8 6 6 x x x) | G |
pat("o.o.o.o.", tab(0 2 2 1 0 0)) |
$riff = tab(x 3 2 0 1 x) | tab(3 2 0 0 0 3) |
tab(10 12 12 x x x) x2 |
```

## AST

```rust
pub enum Node {
    Chord(ChordExpr),
    Arpeggio(ChordExpr),
    Brush(ChordExpr),
    Split(ChordExpr),
    /// `tab(...)`。6弦→1弦のフレット。2つ目はブラッシング
    Tab([Option<u8>; 6], bool),
    Rest,
    Sustain,
    Repeat,
}
```

`ChordExpr` には入れない。`ChordExpr::eval` が `ChordNode` を返す契約で、
コード名を持たない tab は型が合わないため。`Node` に置くことで、

| 書ける | 書けない (パースエラー) |
|---|---|
| `tab(...) \| C \|` | `pow(tab(...))` `oct(tab(...))` |
| `pat("o.o.", tab(...))` | `arp(tab(...))` `blk(tab(...))` |
| `$a = tab(...)` | `s25(tab(...))` `sub(tab(...))` `dom(tab(...))` |
| `tab(...) x2` | |

2つ目の `bool` はブラッシング。`Node::Brush` と同じく**パーサは常に false で
作り**、`pat` の `x` スロットだけが true にする。`pat("oxox", tab(...))` は
同じ押さえ方を弱いベロシティで鳴らす。

## 意味づけ

| | 今 (コード名経由) | tab |
|---|---|---|
| MIDI | `find` の形があればその実音、無ければ理論上の声部配置 | `OPEN_STRING_NOTES[i] + fret` をそのまま |
| 図 | `find` が返す原典の形 | 書かれたフレットそのまま。指番号はデータが無いので空のドット |
| 見出し | コード名 + 代替なら `(≒ 原典名)` | `identify` の先頭候補。該当なしなら記法そのもの。`≒` は出さない |

図の入力型を `&'static Fingering` から外す必要がある。tab の形は実行時の値で
`'static` ではないため。

```rust
/// 図に描ける押さえ方。原典の形と tab() の形を同じ型で扱う
pub struct Shape {
    pub frets: [Option<u8>; 6],
    pub fingers: [Option<Finger>; 6],
    /// 原典での呼び名。`tab()` 由来なら None → 「≒」を出さない
    pub name: Option<&'static str>,
}

/// `tab` があればそれを、無ければ `find` の結果を形にして返す
pub fn shape_of(c: &ChordRef) -> Option<Shape>;
```

`ChordRef` に `pub tab: Option<[Option<u8>; 6]>` を足し、`measure_refs` が
`Node::Tab` から詰める。web の `ChordDiagram` と CLI の図は `Shape` を受ける。

## 移調

`edit::shift_key` が tab の全弦に delta を足す。形を保ったまま平行移動する。
どれかの弦が 0 未満になるなら**何も変えずエラー**にする。

```rust
pub fn shift_key(text: &str, delta: i8) -> Result<String>   // String から変更
```

`-> String` のままだと失敗を黙って握り潰すしかないので戻り値を変える。
web は既存のエラー表示欄に出し、CLI は `?` で落とす。パースできない入力を
`key:` ディレクティブの書き換えにフォールバックする今の挙動は変えない。

`key:` / `scale:` / 度数表記は tab に関係しない。tab は絶対的な押さえ方で、
スケール上の度数を持たないため。

## 指板パネル

候補カードの「挿入」が入れる記法を `tab(...)` に変える。押さえた形がそのまま
戻るので、「この形は今の押さえ方データでは再現できない」の注記は原理的に
出なくなる。コード名の記法 (`pow(G#@6)` など) も併せて挿入できるようにするかは、
実装時に見た目を見て決める。

## 書き戻し

`Display` は `tab(8 6 6 x x x)` を出す。`x` とフレット番号、空白1つ区切り。
入力の余分な空白は正規化される。ブラッシングのフラグは出さない
(`Node::Brush` と同じく記法には無いもの)。

## 影響範囲

`Node` に変種が増えるので、`Node` を網羅する match を持つ箇所を全部通す。

- `import/chord.rs` — `node_parser` に枝を足す
- `import/sexp.rs` — S式の読み込み
- `export/midi.rs` — `interpret_node` と `apply_pattern` の beat 合成
- `export/rechord.rs` — `Display`
- `model/transform.rs` `model/expand.rs` — 素通し (`Rest` などと同じ扱い)
- `edit.rs` — `shift_node` でフレットを動かす
- `fingering/mod.rs` — `ChordRef.tab`、`Shape`、`shape_of`
- `reference.rs` — 記法表に1行
- `chord_midi_web/src/diagram.rs` — props を `Shape` に
- `chord_midi_web/src/fretboard.rs` — 挿入する記法
- `chord_midi_cli/src/diagram.rs` — 図の入力型

## テスト

- パーサ: 正常形 / トークン数の過不足 / 範囲外のフレット / 2桁 / `x` / 空白の揺れ
- MIDI: `tab(0 2 2 1 0 0)` が E メジャーの実音 (E2 B2 E3 G#3 B3 E4) になる
- `pat("oxox", tab(...))` でブラッシングのベロシティが乗る
- 移調: +1 で全弦が +1、開放弦を含む形の -1 がエラー
- 書き戻し: `tab(8 6 6 x x x)` が同じ文字列に戻る
- 逆引き: `Shape` の `name` が None のとき `≒` が出ない
- golden に tab のサンプルを1件足す

## やらないこと

- 6弦以外のチューニングや弦数
- 指番号の指定 (図は空のドットで描く)
- `tab` への `pow` / `arp` / `blk` などの適用
- 既存のコード名記法の置き換え。tab は選択肢を1つ増やすだけ
