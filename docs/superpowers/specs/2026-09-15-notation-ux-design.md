# 記法 UX 改善: 定義・参照・リピートと GUI 補助

- 日付: 2026-09-15
- 対象: `chord_midi`, `chord_midi_web`, `chord_midi_cli`

## 背景と課題

現状の記法には次の問題がある。

1. **適用単位が2種混在** — ノード単位 (`arp(C)`, `blk(F)`) と小節列単位 (`pow(C | E D)`, `pat("oxox", ...)`) が同じ見た目で粒度が違う。
2. **ネストが読みにくい** — `pat("oxox", pow(C | E D))` は前置の括弧が小節グリッドを跨ぐため、`|` の縦揃いが崩れスコープを目で追えない。
3. **再利用ができない** — 同じリズムや進行を毎回ベタ書きする。曲構成 (Aメロ/サビ) を表現できない。進行プリセットは貼り付けるだけ。
4. **記法を覚えられない** — 関数名やパターン文字を思い出せず README を見に行く。GUI は textarea とプレビューのみで入力補助がない。

ユーザーへのヒアリングで、優先度は **3 (反復)** と **4 (記憶コスト)** と確定した。2 (ネスト) は今回の主目的ではないが、参照の導入で副次的に緩和される。

## 解決方針

- 記法に **定義 (`$name = ...`) / 参照 (`$name`) / リピート (`xN`, `( ) xN`)** を足す。
- ファイル全体を `main()` の本体とみなし、**定義は let 束縛、残り全体が戻り値 = 曲**とする。専用キーワード (`song:` 等) は追加しない。
- AST を平らな項目列から**式の木 (`Expr`)** に組み替える。進行全体が1つの `Expr` 値になる。
- GUI は **パレットと常設リファレンス**を足す。GUI は独自モデルを持たず、テキストを編集するだけ。

---

## 1. 記法仕様

### 1.1 定義

```
$name = <本体>
```

- 行頭のみ。本体は1行に収める。
- `$` は既存文法で未使用。弦指定の `@N` とも、`%` `=` `_` とも衝突しない。
- `name` は英数字 + `_` + 日本語 (`$Aメロ` を許す)。
- 値の型は2つだけ。**Pattern** (`"o.o.o.o."`) と **Measures** (小節列・関数適用・参照の並び)。型は代入時に確定する。
- 前方参照は禁止。使う前に定義する。循環参照はエラー。

```
$eight = "o.o.o.o."
$riff  = pow(C | E D)
$A     = pat($eight, C | Am | F | G)
$B     = $riff | $riff | pat($eight, F | G)
```

### 1.2 参照

`$name` は3か所に書ける。

| 位置 | 例 |
|---|---|
| 小節の位置 | `$riff \| C \|` |
| `pat` の第1引数 | `pat($eight, C \| Am)` |
| 関数の中 | `pow($riff)` |

型不一致 (`pat($riff, ...)` = Measures をリズム欄に渡す) はエラー。

### 1.3 曲構成: ファイル = `main()` 本体

専用キーワードは持たない。ファイル全体が `main()` の本体であり、**定義行以外のすべてが戻り値 = 曲**になる。

```
key: C

$eight = "o.o.o.o."
$riff  = pow(C | E D)
$A     = pat($eight, C | Am | F | G)
$B     = $riff x2 | pat($eight, F | G)

$A x2 | $B | $A
```

- 定義が0個のファイルは「本体だけ」なので、**既存ファイルはそのまま同じ意味**になる。
- 定義は 0 小節を返す単位元として評価される。したがって定義はファイルのどこに書いてもよい。
- ただし曲の途中に定義が挟まると読みにくいため、「定義より前に小節行を書くな」は **lint (警告)** として出す。文法規則にはしない。
- 定義だけで本体が無いファイルはエラーにせず「曲が空」として扱う。web は打鍵のたびにパースが走るため、エラー表示は邪魔になる。

### 1.4 リピート

- `xN` は直前の**1項目** (小節 / `$参照` / 関数適用 / グループ) を N 回繰り返す。`N` は 2〜32。`x1` と `x0` は意味がないのでエラーにする。
- `( ... )` でグループを作り、リピート範囲を明示できる。

```
$A x2 | $B            → A A B
C | Am x3 |           → C, Am, Am, Am   (直前の小節だけ)
(C | Am | F | G) x4   → 4小節を4回
```

- `(` の曖昧さは、関数名タグ (`pow(`, `oct(`, `pat(`) を先に試すことで解消する。

---

## 2. AST とパイプライン

### 2.1 型

```rust
pub enum Expr {
    Measure(Vec<Node>, bool),   // 小節 + 後ろが改行か
    Seq(Vec<Expr>),             // `|` / 改行の連結
    Apply(Func, Box<Expr>),     // pow / oct / pat
    Ref(String),                // $name
    Repeat(Box<Expr>, u8),      // xN
    Pattern(Pattern),           // "o.ox"
    Define(String, Box<Expr>),  // $name = ...
    Keyed(Pitch, Box<Expr>),    // key: 以降を包む
    Scaled(Scale, Box<Expr>),   // scale: 以降を包む
    Comment(String),
}

pub enum Func {
    Pow,
    Oct,
    Pat(Box<Expr>),   // 引数は Pattern か Ref
}
```

- `Ast` は廃止し `Expr` 1本にする。`Apply` の `Vec<Box<Ast>>` は `Box<Expr>` (中身は `Seq`) に畳む。
- **`Group` は型に持たない。** `( )` は Display 側で導出する。`Repeat(Seq[2個以上], n)` は `(...) xn`、`Repeat(Measure, n)` は `C xn` と出す。決定的なので round-trip する。
- `Define` / `Comment` は 0 小節の Measures (単位元) として評価される。現状の `resolve()` が Directive/Comment を filter で落としているのと同じ意味。

### 2.2 評価パイプライン

```
parse        → Expr    構文のみ。env は参照しない
expand(env)  → Expr    Define を env に収容して Seq から除去
                       Ref を値に置換、Repeat を n 回に展開
                       型検査 (Pat の引数が Pattern か) と循環検出もここ
into_pitch   → Expr    Keyed/Scaled をスコープとして再帰下降
desugar      → Expr    pow/oct を潰す (現状の desugar_shapes)
→ midi export / fingering
```

`expand` 通過後は `Ref` / `Define` / `Repeat` が消えている。下流は今と同じ形だけを見る。

**`into_pitch` の挙動変更**: 現状は全体を先にスキャンして最後の `key:` を採用しており、途中転調を表現できない。`Keyed` 導入後は再帰下降で「そのスコープの tonic」を渡すため、ファイル途中の `key:` がそこから後ろに効くようになる。これは意図した挙動変更。

### 2.3 エラー

`expand` が返す4種。すべて `anyhow::Err` として既存のエラー表示に載せる。web は打鍵ごとにパースが走るため、パニックさせない方針は現状どおり維持する。

- 未定義参照 `$x`
- 循環参照 (env に解決中スタックを持って検出)
- 型不一致 (Measures をリズム欄に渡した)
- `xN` が範囲外 (2〜32 の外)

### 2.4 Display / round-trip

- `Keyed(C, body)` は **`key: C` 行 + 本体**というフラットな形に戻す。入れ子は出力しない。
- `Define` は `$name = <本体>` 行として出す。
- 定義の並び順は `Seq` の順序がそのまま保持する。

### 2.5 ファイル別の影響

| ファイル | 変更 |
|---|---|
| `model/ast.rs` | `Ast` → `Expr`。`Func::Pat` の引数を `Box<Expr>` に |
| `model/expand.rs` (新規) | env、`expand`、循環検出、型検査 |
| `model/transform.rs` | `into_pitch` を `Keyed`/`Scaled` スコープ再帰に。`desugar_shapes` は腕の形だけ追従 |
| `import/rechord.rs` | `define_parser` / `ref_parser` / `repeat_parser` / `group_parser` を追加 |
| `import/sexp.rs` | `keyed` が `Expr::Keyed` に直接対応する |
| `export/rechord.rs` | `Define` / `Ref` / `Repeat` / `Keyed` の Display |
| `export/midi.rs` | 展開済みだけ見る。`Ref`/`Define`/`Repeat` の腕はエラー |
| `fingering/mod.rs` | 同上 + `MeasureRef` に `section: Option<String>` を追加 |
| `edit.rs` | `shift_ast` に `Define`/`Keyed`/`Repeat` の腕を足す。定義本体も移調する |
| `chord_midi_cli` | 型名の追従のみ |

---

## 3. GUI 補助

**原則: GUI は独自モデルを持たない。すべて「テキストを編集する」だけ。** 双方向同期を作らない。

### 3.1 配置

エディタタブの右 (狭い画面では下、`<details>` で畳む) に記法パネルを置く。

### 3.2 部品

**`RhythmGrid` — リズム入力**
- 分割数セレクタ (4 / 8 / 16) とマスの並び。各マスは `o` → `.` → `x` の3状態トグル。記法自体は 1 と 2 も許すが、グリッドでは扱わない (手で書ける)。
- 挿入ボタンでカーソル位置に `pat("o.o.oxox", )` を入れる。
- カーソルが既存の `pat("...")` の中にあるときは、パターン文字列だけを置換する。

**`FuncPalette` — 関数ボタン**
- `pow` `oct` `pat` / `arp` `blk` `s25` `sub` `dom`。
- 選択範囲があれば包む (`C | E D` を選択して `pow` → `pow(C | E D)`)。無ければ雛形を挿入する。
- `xN` と `( )` もボタン化し、選択範囲を `( ) x2` で包めるようにする。

**`SyntaxReference` — 記法表**
- 記号 (`|` `=` `%` `_` `N.C.` `@N` `$` `xN` `( )`) と関数の一覧を常設表示する。
- **`chord_midi/src/reference.rs` に静的テーブルを置き、web と CLI で共有する。** `presets.rs` と同じ流儀。CLI は `--help-syntax` で同じ表を出す。README と二重管理しない。

**`DefineChips` — 定義の一覧**
- 現在のテキストから定義名を抽出してボタン化する。クリックでカーソル位置に `$name` を挿入する。
- **AST に依存せず、行頭 `$name =` を正規表現で拾う。** 打鍵途中でパースが落ちていてもチップが消えない。
- 既存の「進行プリセット」の上位互換。プリセットを `$` 定義として貼る形に変える案はあるが、今回はやらない。

### 3.3 共通処理

`insert_at_cursor(textarea, text, wrap: Option<(&str, &str)>)` を1つ作り、全部品から呼ぶ。`selection_start` / `selection_end` を読み、挿入後にカーソルを括弧の内側へ移す。

### 3.4 プレビュー

コードカードに `MeasureRef.section` のラベルを出す。`$A` 由来の小節がどこまでかが図で分かるようにする。

---

## 4. AST 表示

現状、web の「解釈結果」も CLI の `--print-ast` も `Display` (rechord テキストの再出力) を出しているだけで、構造が見えない。両方を JSON 表示に変える。

- `chord_midi` に `serde = { version = "1", features = ["derive"] }` と `serde_json = "1"` を追加する (現状 workspace に serde はない)。
- `Expr` / `Node` / `Func` / `Pattern` / `Beat` / `ChordNode` / `Key` / `Pitch` / `Modifier` / `Degree` / `Scale` / `Transform` に `#[derive(Serialize)]` を付ける。
- web は「解釈結果」に `<pre>` で pretty JSON を出す。CLI は `--print-ast` が JSON を出す。
- 木の整形コードは書かない。折り畳みが欲しくなったら後で JSON ビューアを足す。

表示は3つから選べるようにする。

| 表示 | 中身 | 用途 |
|---|---|---|
| **構文** (既定) | `parse` 直後。`Define` / `Ref` / `Repeat` / `Keyed` がそのまま見える | 自分の書いた構造の確認 |
| **展開後** | `expand` + `into_pitch` 済み。度数は実音に、`$` は消え、`xN` は並ぶ | 実際に鳴る中身の確認 |
| **テキスト** | 現行の Display 再出力 | round-trip の確認 |

web はラジオ2つ + 既存表示、CLI は `--print-ast` (構文) / `--print-ast=expanded` / `--print-ast=text` とする。CLI 側は現状 `bool` のフラグなので、値を省略できる引数 (clap の `num_args(0..=1)` + `default_missing_value`) に変える。

---

## 5. テスト

**パーサ** (`import/rechord.rs`)
- `$name = ...` の定義、`$name` 参照、`xN`、`( ) xN`
- `pat($eight, C)` — 参照をリズム欄に渡す
- `(` の曖昧性: `pow(C)` は関数、`(C | D) x2` はグループ
- 日本語の定義名 `$Aメロ`

**expand** (`model/expand.rs`)
- 参照が展開される / `xN` が n 個に並ぶ / ネストした参照が解決される
- エラー4種: 未定義参照、循環参照、型不一致、`xN` 範囲外
- 展開後に `Ref` / `Define` / `Repeat` が残っていないこと

**round-trip** (`export/rechord.rs`)
- 定義行の順序が保持される
- `Keyed` が `key: X` 行に戻る
- `( ) xN` の括弧が導出で正しく付く / `C x4` に余計な括弧が付かない
- 既存の round-trip テストがすべて通る

**転調** (`edit.rs`)
- 定義本体の中のコードも移調される
- `Keyed` の pitch が移調される
- 途中転調 (`key:` が2回) で両方ずれる

**押さえ方** (`fingering/mod.rs`)
- `MeasureRef.section` に `$A` 由来の名前が入る
- `pat` のリズムが参照経由でも伝わる

**後方互換 (最重要)**
- README とプリセットの既存の進行が、すべて今までと同じ MIDI を出す
- 既存テストを1つも書き換えずに通す。書き換えが必要なら挙動変更なので明示する

---

## 6. 段階分け

PR は3本に分ける。

**Phase 1: `Ast` → `Expr` 組み替え + `Keyed` + JSON 表示**
- 記法の機能追加はゼロ。型の組み替えと `key:` のスコープ化だけ。
- `key:` の挙動変更 (最後の1つが全体 → 以降に効く) がここで入る。既存テストへの影響をここで見切る。
- serde derive と JSON 表示もここに含める。以降のデバッグが楽になるため先に入れる。

**Phase 2: 定義・参照・リピート**
- `$name =` / `$name` / `xN` / `( )`、`expand`、エラー4種。
- Display / 転調 / `MeasureRef.section` の追従。
- 「定義より前に小節行」の lint。

**Phase 3: GUI**
- `reference.rs`、記法パネル、`RhythmGrid`、`FuncPalette`、`DefineChips`、`insert_at_cursor`。
- CLI の `--help-syntax`。

Phase 1 は最もリスクが高く、機能的な見返りがゼロ。単独で先に通して既存挙動を固めてから Phase 2 に進む。

---

## 7. 今回やらないこと

- **パイプライン後置記法** (`C | Am >> pat "o.o." >> pow`) — ネストの読みにくさは参照の導入で緩和されるため、様子を見る。
- **セクション見出しブロック** (`A:` でインデント) — `$A = ...` の糖衣として後から足せる。
- **進行プリセットの `$` 定義化** — GUI の `DefineChips` が入ってから判断する。
- **入れ子スコープ** (`$A = { $x = ...; ... }`) — 必要になっていない。
