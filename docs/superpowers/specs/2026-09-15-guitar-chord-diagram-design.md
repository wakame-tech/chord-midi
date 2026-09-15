# 入力コードの押さえ方ダイアグラム表示 設計 (Issue #17)

日付: 2026-09-15
Issue: https://github.com/wakame-tech/chord-midi/issues/17

## 目的

`chord_midi_web` の入力欄に書いたコード進行について、各コードのギター押さえ方を HTML/CSS で描画したダイアグラムとして表示する。譜面を見ながらすぐ弾けるようにするのが狙い。

押さえ方データは daichan easy guitar「超簡単コード一覧」を元に `docs/easy_guiter_chords.json` として整備済み。67件。すべて「バレーなし / 小指を使わない」条件を満たす簡略ボイシングで、押さえるのは最大3本（`F#m7-5` と `G#m7-5` の2件だけ6弦を親指で押さえて計4本）。

## 決定事項 (ユーザー確定)

| 論点 | 決定 |
| --- | --- |
| 入力コードと押さえ方の照合 | **構成音（ピッチクラス集合）で照合**。コード名の文字列一致は使わない |
| データの供給 | **コンパイル時埋め込み**。JSON から生成した Rust const テーブルを commit する |
| ロジックの置き場所 | **`chord_midi` クレート**。host で `cargo test` できるようにする |
| 表示範囲 | 入力に現れるコードを**出現順に全件**並べる |
| 色分け | 出現するコードの種類ごとに色を割り当てる（同じコードは常に同じ色） |
| 未収録コード | 「データなし」プレースホルダを表示（枠は描く） |
| 運指 | **指名（人/中/薬/親）をドット内に白抜き文字で描く**。原典に指定が無いので規則で導出する |

serde / serde_json をコアに足す案（実行時に JSON をパース）は見送った。生成テーブルなら依存追加ゼロ・パースコストゼロ・データ不整合がコンパイル時に出る。再生成頻度が実質ゼロ（原典が固定の67件）なので生成ステップのコストは無視できる。

## アーキテクチャ

```
docs/easy_guiter_chords.json
        │  scripts/gen_fingerings.py  (just gen-fingerings)
        ▼
chord_midi/src/fingering/data.rs     生成物・commit する
        ▲
chord_midi/src/fingering/mod.rs      Fingering 型 / AST 走査 / 照合
        ▲
chord_midi_web/src/diagram.rs        Yew コンポーネント（描画のみ）
chord_midi_web/style.css             ダイアグラムの CSS
```

依存の向きは web → core の一方向のみ。core は web を知らない。

## 1. データ層 — `chord_midi/src/fingering/data.rs`

```rust
pub struct Fingering {
    /// 原典でのコード名。表示とデバッグ用で、照合には使わない
    pub name: &'static str,
    /// 原典のセクション名 ("Cコード系" など)
    pub group: &'static str,
    /// 6弦→1弦。"x3201x" 形式
    pub tab: &'static str,
    /// 6弦→1弦。None = ミュート、Some(0) = 開放
    pub frets: [Option<u8>; 6],
    /// 6弦→1弦。押さえる弦だけ Some。開放弦とミュート弦は None
    pub fingers: [Option<Finger>; 6],
    /// 鳴る音のピッチクラス集合。bit n が立っていればピッチクラス n を含む
    pub pitch_classes: u16,
    /// 最低音（鳴る弦のうち最も低い弦）のピッチクラス
    pub bass: u8,
}

pub static FINGERINGS: &[Fingering] = &[ /* 67件 */ ];
```

`pitch_classes` と `bass` は生成時に frets から計算して埋め込む。開放弦のピッチクラスは 6弦→1弦で `[4, 9, 2, 7, 11, 4]`（標準チューニング E A D G B E）。

```rust
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Finger { Index, Middle, Ring, Thumb }

impl Finger {
    /// 日本語1文字。"人" / "中" / "薬" / "親"
    pub fn label(&self) -> &'static str;
}
```

小指 (`Pinky`) は定義しない。原典が「小指は使わない」を条件にしており、67件すべてが人・中・薬・親で足りるため。

### 運指の導出

原典の図には指の指定が無い。記事本文にも大半のコードでは書かれていないので、以下の規則で導出する。

1. 押さえる音（フレット1以上）を **フレット昇順 → 弦番号 6弦→1弦** の順にソートする
2. 押さえる音が4つのとき、6弦の音を **親** に割り当て、残り3音を上記順で **人 → 中 → 薬**
3. 押さえる音が2〜3つのとき、上記順で **人 → 中 → 薬**
4. 押さえる音が1つだけのとき **中**
5. 開放弦・ミュート弦は `None`

根拠と検証:

- 4音は `F#m7-5` (`2x221x`) と `G#m7-5` (`4x443x`) の2件のみで、どちらも6弦を押さえる。原典の F コード系解説に「6弦は親指を上から被せるように押さえて、残りを人、中、薬指で押さえます」と明記があり、規則2はこれに一致する。
- 規則1は標準的な運指と一致する。`C` (`x3201x`) → 人=2弦1F / 中=4弦2F / 薬=5弦3F、`D` (`xx0232`) → 人=3弦2F / 中=1弦2F / 薬=2弦3F、`E` (`022100`) → 人=3弦1F / 中=5弦2F / 薬=4弦2F、`A` (`x0222x`) → 人=4弦2F / 中=3弦2F / 薬=2弦2F。
- 規則4は、原典が `G` (`3x000x`) を「薬指または中指で押さえる」とし、人差し指の付け根を1弦・5弦のミュートに使うと説明していることに合わせた。1本指のコードでは人差し指を空ける。

### 生成スクリプト

`scripts/gen_fingerings.py` が唯一の導出箇所になる。`docs/easy_guiter_chords.json` を読んで運指を計算し、3つを書き出す。

- `chord_midi/src/fingering/data.rs`（生成物・commit する。先頭に「手で編集しない」旨のコメント）
- `docs/easy_guiter_chords.json` に `fingers` フィールドを追加（HTML 表示用データとして完結させる）
- `docs/easy_guiter_chords.md` に運指の列を追加

`Justfile` に `gen-fingerings` レシピを追加する。

## 2. 照合層 — `chord_midi/src/fingering/mod.rs`

### AST からコード列を取り出す

```rust
pub struct ChordRef {
    /// 表示用のコード名
    pub label: String,
    pub pitch_classes: u16,
    /// スラッシュコードのベース。無ければ None
    pub bass: Option<u8>,
}

pub fn chord_refs(ast: &Ast) -> Vec<ChordRef>;
```

- 呼び出し側が `Ast::resolve()` 済みの AST を渡す。`resolve()` は既存 API で、degree 表記を絶対音に直し、ディレクティブとコメントを落とす。
- `Ast` は `Clone` を持たず `resolve()` も `export()` も self を consume するので、web の `compile()` は
  `import` → `format!("{}", ast)`（`<pre>` 用、degree 表記のまま）→ `ast.resolve()` → `chord_refs(&resolved)` → `export(resolved)`
  の順に一度だけ流す。パースをやり直したり `Ast` に `Clone` を足したりはしない。
  `MidiExporter::export` は内部でもう一度 `resolve()` を呼ぶが、解決済み AST に対しては何も変えない（冪等）。
- `Ast::Score` → `Ast::Measure` → `Node` の順に走査する。
- `Node::Chord` / `Node::Arpeggio` は `to_chord()` で `Chord` にし、`key` のルート音と `semitones`（ルート相対）からピッチクラス集合を組む。`on` があればベースに入れる。
- `Node::Transform(t, c)` は `t.expand(...)` した結果のノードを展開してから同じ処理をする。`resolve()` 後は key が Absolute なので tonic/scale は無視される（既存の midi export と同じ扱い）。
- `Node::Rest` / `Node::Sustain` / `Node::Repeat` はスキップする。同じコードが連続しても、入力に書かれた分だけカードが出る。

### 照合

```rust
pub fn find(chord: &ChordRef) -> Option<&'static Fingering>;
```

1. 候補を絞る: `fingering.pitch_classes & !chord.pitch_classes == 0`。つまり押さえ方が鳴らす音が、コードの構成音の**部分集合**であるものだけ。構成音に無い音を鳴らす押さえ方は除外する。
2. スコア = `(chord.pitch_classes & !fingering.pitch_classes).count_ones()`。つまり**構成音のうち押さえ方でカバーされていない音の数**。小さいほど良い。完全一致は 0 になり自動的に最優先される。
3. 同点の tie-break:
   1. `chord.bass` が Some のとき、`fingering.bass` が一致するものを優先
   2. 押さえるフレットの最大値が小さいもの（ローポジション優先）
   3. `FINGERINGS` 中の出現順（原典の掲載順 = 易しい順）
4. 候補が無ければ `None`。

補足: データ側の押さえ方は簡略ボイシングなので、5thや root が省かれているものが普通にある（例 `Am7/G` は G-E-C で A を含まない）。ステップ1の部分集合判定はこれを許容する。逆に構成音に無い音（例えば `C` に対して 9th を含む押さえ方）は鳴らさない、という方針。

### テスト (host, `cargo test -p chord_midi`)

- `C` → `x3201x`、`Am7` → `x0201x`、`Em7` → `020000` が選ばれる
- `C△7` に対して `C` (x3201x) ではなく `C△7` (x3x000) が選ばれる（完全一致優先）
- degree 表記 `IVM7` を key:C で resolve → F系の押さえ方が引ける
- スラッシュコード `D/F#` でベース一致の押さえ方が選ばれる
- 未収録コード（構成音に合う候補が無いもの）で `None` が返る
- データ整合: 全件で `tab` と `frets` が一致し、`pitch_classes` / `bass` が `frets` から再計算した値と一致する
- 運指: 押さえる弦すべてに指が割り当てられ、ミュート弦と開放弦は `None`。同じ指が同一コード内で2回使われない
- 運指の既知ケース: `C` → 人=2弦1F / 中=4弦2F / 薬=5弦3F、`F#m7-5` → 6弦が親で残りが人中薬、`G` (1本指) → 中

## 3. 描画層 — `chord_midi_web`

### コンポーネント `src/diagram.rs`

```rust
#[derive(Properties, PartialEq)]
pub struct ChordDiagramProps {
    pub label: String,
    pub fingering: Option<&'static Fingering>,
    /// CSS カスタムプロパティに流し込む色
    pub color: String,
}

#[function_component(ChordDiagram)]
pub fn chord_diagram(props: &ChordDiagramProps) -> Html;
```

- 1枚のカード = ヘッダ（コード名）+ ダイアグラム本体。
- ダイアグラムは CSS grid で 6行（弦）× 5列（フレット）。罫線はセルの border で引き、ナットは左端の太いボーダー。
- ドットは各セルの子 div（`border-radius: 50%`、黒）。中に指名を白抜き1文字（人 / 中 / 薬 / 親）で入れる。
  ドット径に対して文字が詰まらないよう `font-size` はドット径の約 0.6 倍にし、`line-height: 1` で中央に置く。
- 各行の左外側にミュート `×` / 開放 `○` のマーカー列を置く。押さえる弦は空欄。
- **表示フレット窓**: 押さえるフレットの最大値が 5 以下なら 1〜5フレットを描く。超える場合は最小フレットから5フレット分を描き、窓の開始フレット番号（`4fr` など）をダイアグラム脇に出す。
- `fingering` が `None` のときは同じ寸法の空の枠を描き、本体に「データなし」と出す。カードの幅が揃うので並びが崩れない。

### 色分け

- `App` 側で、表示するコード列から `label` のユニーク集合を出現順に作り、固定パレット（8色）を順に割り当てる。9種類目以降は先頭に戻る。
- 色はカードの枠線とヘッダ帯に使う。ドットと罫線は黒のままにする（視認性を落とさないため）。
- 同じ `label` は常に同じ色になる。

### 配置と CSS

- `App` の描画に `<div class="fingerings">` を追加し、プリセットボタン群と `<pre>`（AST 表示）の間に置く。
- `display: flex; flex-wrap: wrap;` で入力順に横並び、幅が足りなければ折り返す。
- CSS は `chord_midi_web/style.css` に新規作成し、`index.html` に `<link data-trunk rel="css" href="style.css" />` を足して Trunk のアセットパイプラインに乗せる。
- カードごとの色は `style="--chord-color: ..."` でインライン指定し、CSS 側は `var(--chord-color)` を参照する。

### エラー時の挙動

- 入力がパースエラーのとき（既存の `error` state が非空）は、ダイアグラム領域は直前の成功時の内容を出さず空にする。既存の `result` / MIDI と同じ扱いに揃える。
- 入力が空のときも空にする。

## スコープ外 (YAGNI)

- 運指の最適化（同じ押さえ方に対する代替運指の提示、コード間の移動しやすさを考えた指の選び直し）
- フレットボード探索による押さえ方の自動生成（未収録コードはプレースホルダのまま）
- 押さえ方の複数候補表示・切り替え UI
- カポ対応、変則チューニング
- ダイアグラムの画像書き出し

## 作業手順

1. `scripts/gen_fingerings.py` と `Justfile` のレシピを追加、`data.rs` を生成
2. `chord_midi/src/fingering/mod.rs` を実装、テストを書く
3. `chord_midi_web` に `diagram.rs` / `style.css` を追加し `main.rs` から繋ぐ
4. `trunk build` が通ることと、ブラウザでの見た目を確認
5. PR を出す
