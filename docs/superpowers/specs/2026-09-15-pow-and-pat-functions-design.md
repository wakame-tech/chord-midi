# パワーコード関数 pow() とリズム関数 pat() 設計 (Issue #20)

日付: 2026-09-15
Issue: https://github.com/wakame-tech/chord-midi/issues/20

## 目的

rechord 記法に、小節をまたいで適用できる関数を2つ足す。

```
pow( <小節列> )                   例: pow(C | E D)   pow(C)
pat( "<パターン>", <小節列> )      例: pat("o.o.oxox", C | E D)
pat("o.o.oxox", pow(C | E D))     ネスト可
```

`pow()` は中のコードをパワーコードにする。`pat()` は中の小節にリズムを与える。

## 既存との違い

既存の関数 `arp()` / `blk()` / `s25()` / `sub()` / `dom()` は `Node::Transform(Transform, ChordNode)` で、**1コードだけ**を引数に取り、小節の1スロットの中で展開される。

今回の2つは `pow(C | E D)` のように `|` を含む小節列を引数に取るので、`Node` の下には収まらない。AST に新しいレベルが要る。

## 決定事項 (ユーザー確定)

| 論点 | 決定 |
| --- | --- |
| `pow` の構成音 | **ルート + 5th の2音**。元の 3rd / 7th / テンションは全て捨てる |
| `x` の意味 | **ブラッシング**（ミュートした空振り）。ベロシティを下げて表現する |
| `pat` の文字数 | **文字数 = 1小節の分割数**。1/2/4/8/16 のみ許す |
| 1小節に複数コードがあるとき | **パターンは小節全体に1回**。各スロットの時刻に鳴っているコードを弾く |
| AST の持ち方 | **`Ast::Apply` を足す**。パース時の脱糖（糖衣構文）は採らない |

パース時に脱糖する案は見送った。`shift_key` がテキストを書き戻すようになったため、キーを上下した瞬間に `pow(C | E D)` が `C5 | E5 D5` へ化けてしまう。AST に関数を残せば `Display` がそのまま再出力できる。

`Ast::Measure` に修飾フラグを持たせる案も見送った。書き戻すとき連続する同じフラグをまとめ直す必要があり、`pow(C) pow(E D)` と `pow(C | E D)` の区別が復元できない。

## AST

```rust
pub enum Ast {
    Comment(String),
    Directive(Directive),
    Measure(Vec<Node>, bool),
    /// 小節列に関数を適用する
    Apply(Func, Vec<Box<Ast>>),
    Score(Vec<Box<Ast>>),
}

/// 小節列に掛ける関数
#[derive(Debug, Clone, PartialEq)]
pub enum Func {
    Pow,
    Pat(Pattern),
}

/// リズムパターン。長さは 1/2/4/8/16 のいずれか
#[derive(Debug, Clone, PartialEq)]
pub struct Pattern(pub Vec<Beat>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Beat {
    /// `o` 打弦
    Hit,
    /// `.` 休符
    Rest,
    /// `x` ブラッシング
    Brush,
}
```

`Apply` の中身は `Ast::Measure` と `Ast::Apply`（ネスト）だけを想定する。`Directive` や `Comment` は文法上入らない。

## 責務の分け方

`pow` と `pat` は関心が違うので、脱糖する場所を分ける。

**`pow` は音程の話** — `resolve()` で AST を書き換える。`Apply(Func::Pow, items)` を中身の `items` に潰し、含まれる全 `ChordNode` の `modifiers` に `Modifier::Power` を足す。

MIDI 出力と押さえ方プレビューの両方が `resolve()` 後の AST を見るので、両方に自動で効く。

**`pat` はリズムの話** — `resolve()` では展開せず、`Apply(Func::Pat(_), _)` の形のまま残す。MIDI の `Score::interpret` がこれを処理する。押さえ方の収集側 (`fingering::collect_ast`) は `Apply` を素通りして中の小節だけを見る。

こうしないと `pat("o.o.oxox", C)` が脱糖後に1小節8ノードになり、**押さえ方カードが同じコード `C` で8枚並ぶ**。リズムを MIDI 側だけの関心にしておけばプレビューは `C` 1枚のままになる。

## pow の意味

```rust
pub enum Modifier {
    // ... 既存 ...
    /// パワーコード。ルートと5thだけを鳴らす
    Power,
}
```

`Chord::modify` で `Modifier::Power` は `semitones` を `{0, 7}` に**置き換える**（`clear()` してから挿入する）。

`Modifier` は `BTreeSet` に入っており、derive した `Ord` は enum の宣言順なので、`Power` を**末尾に足す**ことで必ず最後に適用され、他の修飾を上書きできる。`Cm7` は `{Minor(7), Power}` となり `{0, 7}` になる。

スラッシュコードの `on` は触らない。`pow(C/E)` はベースに E が残る。

`Modifier::Power` の `Display` は `"5"` とする。これは `Modifier::Dominant(5)` の表示と衝突するが、脱糖後の AST をテキストに戻す経路が無い（web の `<pre>` も `shift_key` も脱糖前の AST を使う）ので実害は無い。デバッグ用と割り切る。

## pat の展開

`Apply(Func::Pat(pattern), items)` を MIDI で解釈するとき、`items` の中の各 `Ast::Measure` を次のように読み替える。

1. パターン長を `n` とする。小節を `n` スロットに割り、スロット `i` が占める尺は `MEASURE_LENGTH / n`
2. スロット `i` の時刻は `i * MEASURE_LENGTH / n`
3. その時刻に元の小節で鳴っているノードを引く。元の小節のノード数を `m` とすると、既存の `measure_unit_size(m)` が1ノードあたりの尺 `dur` を返すので、ノード `j` は時刻 `[j * dur, (j+1) * dur)` を占める
4. 引いたノードとパターンの文字から、そのスロットで鳴らすものを決める

| 元のノード | `o` | `.` | `x` |
| --- | --- | --- | --- |
| `Chord` / `Arpeggio` / `Transform` | そのコードを打弦 | 休符 | ブラッシング |
| `Rest` | 休符 | 休符 | 休符 |
| `Sustain` / `Repeat` | 直前のコードを打弦 | 休符 | 直前のコードをブラッシング |

`E D` に `"o.o.oxox"` を掛けると、前半4スロットが E、後半4スロットが D になる。

`items` の中に `Ast::Apply` がネストしている場合は、内側を先に解釈した結果の小節に対してパターンを掛ける。実装上は再帰で処理する。

## ブラッシングの MIDI 表現

`Note` に `velocity` を足す。

```rust
struct Note {
    chord: Option<Vec<NoteNumber>>,
    duration: u32,
    velocity: Velocity,
}
```

通常の打弦は `Velocity::default()`、ブラッシングは `Velocity::new(40)` とする。1小節が 16 tick しかないため、音の長さを短くしてブラッシングを表現することはできない。ベロシティで差を付ける。

`write_notes` と `dump_notes` に velocity を通す。既存の呼び出しは `Velocity::default()` を渡すので出力は変わらない。

## パースとエラー

- `pow(` / `pat(` を関数名として読み、閉じ括弧まで小節列をパースする
- `pat` の第1引数はダブルクォートで囲んだパターン文字列。`,` で区切って第2引数に小節列
- パターン長が 1/2/4/8/16 以外 → パースエラー
- パターンに `o` `.` `x` 以外の文字 → パースエラー
- パースエラーは既存どおり `import` が `Err` を返し、web はエラー表示に落ちる

## 既存コードで触る箇所

| ファイル | 変更 |
| --- | --- |
| `model/ast.rs` | `Ast::Apply`、`Func`、`Pattern`、`Beat` を追加 |
| `model/modifier.rs` | `Modifier::Power` を末尾に追加、`Display` |
| `model/chord.rs` | `modify` に `Power` の腕を追加 |
| `import/rechord.rs` | `pow(...)` / `pat("...", ...)` のパーサ |
| `export/rechord.rs` | `Ast::Apply` の `Display` |
| `model/transform.rs` | `into_pitch` / `into_degree` に `Apply` の腕、`resolve` で `pow` を脱糖 |
| `export/midi.rs` | `interpret` に `Apply` の腕、`Note` に velocity |
| `fingering/mod.rs` | `collect_ast` に `Apply` の腕（素通り） |
| `edit.rs` | `shift_ast` に `Apply` の腕 |

## テスト

**パーサ**
- `pow(C)` / `pow(C | E D)` / `pat("o.o.", C)` / `pat("o.o.oxox", C | E D)` / ネスト `pat("oxox", pow(C))` が読める
- パターン長 3 や 5 はエラー
- `o . x` 以外の文字はエラー

**Display 往復**
- `pow(C | E D)` をパースして `Display` すると同じ文字列に戻る
- `pat("o.o.oxox", C | E D)` も同様

**pow**
- `pow(C)` の `semitones` が `{0, 7}`
- `pow(Cm7)` も `{0, 7}`（元の修飾を上書きすること）
- `pow(C/E)` でベースの E が残ること

**pat**
- `pat("o.o.", C)` が 4 ノード（打弦・休符・打弦・休符）になり、総尺が `MEASURE_LENGTH` のまま
- `pat("oxox", C)` のブラッシング位置のベロシティが低いこと
- `pat("oooo", E D)` で前半2つが E、後半2つが D になること
- `pat("o...", _)` を含む進行の総尺が、パターン無しのときと一致すること

**移調**
- `shift_key("pow(C | E D)", 1)` が `pow(C# | F D#)` になり、`-1` で戻ること
- `shift_key("pat(\"o.o.\", C)", 1)` が `pat("o.o.", C#)` になること

**押さえ方**
- `pow(C)` の `ChordRef` の構成音が `{C, G}` になり、`find` が押さえ方を返すこと
- `pat("o.o.oxox", C)` の `measure_refs` が 1 小節 1 コードのまま（リズムでカードが増えないこと）

## スコープ外 (YAGNI)

- `C5` という記法の意味変更。今も `Dominant(5)`（メジャートライアド）として解釈される。今回は `pow()` だけを入れる
- ベロシティをユーザーが指定する記法
- パターンの長さを小節をまたいで繰り返す（2小節で1パターンなど）
- ストロークの上下（ダウン/アップ）の区別
- 押さえ方プレビューへのリズム表示
