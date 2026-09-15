# Phase 2: 定義・参照・リピート 実装計画

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** rechord 記法に `$name = ...` の定義、`$name` の参照、`xN` のリピート、`( )` のグループを足す。ファイル全体を `main()` の本体とみなし、定義は 0 小節を返す単位元、残り全部が曲になる。

**Architecture:** `Expr` に `Define` / `Ref` / `Repeat` / `Pattern` を足す。パーサはこれらを構文のまま組み立て、新設する `model/expand.rs` が環境 (env) を持って `Define` を収容し `Ref` を値に置換し `Repeat` を展開する。`resolve()` のパイプラインは `expand` → `into_pitch` → `desugar_shapes` → `strip_comments` になる。`expand` 通過後は `Define`/`Ref`/`Repeat` が消えるので、MIDI 書き出しと押さえ方抽出は今と同じ形だけを見る。

**Tech Stack:** Rust 2021, nom 7 + nom_locate + nom-tracable (パーサ), serde + serde_json (AST 表示), yew 0.21 (web), clap 4 (CLI)

**設計書:** `docs/superpowers/specs/2026-09-15-notation-ux-design.md`
**前提:** Phase 1 (`docs/superpowers/plans/2026-09-15-expr-ast-phase1.md`) がマージ済み

---

## Phase 1 から引き継ぐ約束

1. **ゴールデン MIDI (`chord_midi/tests/golden/*.mid`) を1バイトも変えない。** 既存の進行の出力が変わったら後方互換の破壊。新しい記法のサンプルは**追加**してよい
2. **既存テストのアサーションの意味を変えない。** 型変更に伴う機械的な書き換えは可。期待値を変えたくなったら手を止めて報告する
3. **個別タスクで `cargo fmt` を走らせない。** 整形は最後に1本のコミットで当てる。ただし Phase 1 で CI に `fmt --check` を入れたので、**最後のタスクまでに必ず fmt を当てること**
4. **macOS の BSD sed は `\b` を解釈せず、エラーなしで0件置換になる。** 一括置換は `perl -pi -e`
5. `chord_midi_web` は workspace から exclude。チェックは `(cd chord_midi_web && cargo check --target wasm32-unknown-unknown)`、ビルドは `trunk build`

---

## 記法仕様 (設計書からの要点)

```
key: C

$eight = "o.o.o.o."             # Pattern 型の定義
$riff  = pow(C | E D)           # Measures 型の定義
$A     = pat($eight, C | Am | F | G)
$B     = $riff x2 | pat($eight, F | G)

$A x2 | $B | $A                 # 末尾式 = 曲
```

- 定義は行頭 `$name = <本体>`。本体は1行
- `name` は英数字 + `_` + 日本語
- 型は2つ。**Pattern** (`"o.o.o.o."`) と **Measures** (小節列・関数適用・参照の並び)
- 前方参照は禁止。循環参照はエラー
- `$name` は「小節の位置」「`pat` の第1引数」「関数の中」に書ける
- `xN` は直前の1項目 (小節 / `$参照` / 関数適用 / グループ) を N 回。`N` は 2〜32。`x0` `x1` はエラー
- `( ... )` でグループを作りリピート範囲を明示できる
- 定義は 0 小節を返す単位元。ファイルのどこに書いてもよいが、定義より前に小節行があったら **lint (警告)**

---

## 型の最終形

```rust
pub enum Expr {
    Comment(String),
    Keyed(Pitch, Box<Expr>),
    Scaled(Scale, Box<Expr>),
    Measure(Vec<Node>, bool),
    Apply(Func, Box<Expr>, bool),
    Seq(Vec<Expr>),
    // --- Phase 2 で足すもの ---
    /// `"o.ox"` のリズムパターン。`pat` の第1引数になる値
    Pattern(Pattern),
    /// `$name = ...`。2つ目は「この定義の後ろで改行されている」(常に true)
    Define(String, Box<Expr>),
    /// `$name`
    Ref(String),
    /// `xN`。N は 2〜32
    Repeat(Box<Expr>, u8),
    /// `$name` を展開した跡。どの定義から来た小節かを残す。
    /// パーサは作らない。`expand` だけが作る (Task 8 で足す)
    Section(String, Box<Expr>),
}

pub enum Func {
    Pow,
    Oct,
    /// 引数は `Expr::Pattern` か `Expr::Ref`
    Pat(Box<Expr>),
}
```

**`Group` は型に持たない。** `( )` は Display 側で導出する。`Repeat(Seq[2個以上], n)` は `(...) xn`、`Repeat(Measure, n)` は `C xn`。

---

## ファイル構成

| ファイル | 責務 | 変更 |
|---|---|---|
| `chord_midi/src/model/ast.rs` | 式の型 | 中 |
| `chord_midi/src/model/expand.rs` | **新規**。env、`expand`、循環検出、型検査、エラー4種 | 新規 |
| `chord_midi/src/model/transform.rs` | `resolve` のパイプラインに `expand` を挟む | 小 |
| `chord_midi/src/import/rechord.rs` | `define_parser` / `ref_parser` / `repeat_parser` / `group_parser` | 大 |
| `chord_midi/src/export/rechord.rs` | `Define`/`Ref`/`Repeat`/`Pattern` の Display と括弧の導出 | 中 |
| `chord_midi/src/export/midi.rs` | `Func::Pat` の引数が `Box<Expr>` になったことへの追随 | 小 |
| `chord_midi/src/fingering/mod.rs` | 同上 + `MeasureRef.section` | 中 |
| `chord_midi/src/edit.rs` | `shift_ast` に新しい腕 | 小 |
| `chord_midi/src/lint.rs` | **新規**。「定義より前に小節行」の警告 | 新規 |
| `chord_midi/tests/golden.rs` | 新記法のサンプル追加 | 小 |
| `chord_midi_cli/src/main.rs` | lint の表示 | 小 |
| `chord_midi_web/src/main.rs` | lint の表示、`MeasureRef.section` のラベル | 中 |
| `README.md` | 記法の説明 | 中 |

---

## 依存関係と並列化

```
Task 1 → Task 2 → Task 3 → Task 4 → Task 5 ─┬→ Task 6 (export/rechord.rs)
                                             ├→ Task 7 (edit.rs)
                                             └→ Task 8 (fingering/mod.rs)
                                                        ↓
                                             Task 9 → Task 10 → Task 11
```

**Task 6 / 7 / 8 は触るファイルが交わらないので並列に実行してよい。** それ以外は直列。

---

## Task 1: `Expr::Pattern` を足し、`Func::Pat` を `Box<Expr>` にする

`pat($eight, ...)` を書けるようにするための下ごしらえ。この Task では**参照はまだ作らない**ので、`Func::Pat` の中身は必ず `Expr::Pattern` になる。

**Files:**
- Modify: `chord_midi/src/model/ast.rs`
- Modify: `chord_midi/src/import/rechord.rs` (`apply_parser`)
- Modify: `chord_midi/src/export/rechord.rs` (Display)
- Modify: `chord_midi/src/export/midi.rs` (`interpret` / `interpret_with_pattern`)
- Modify: `chord_midi/src/fingering/mod.rs` (`collect_ast`)
- Modify: `chord_midi/src/model/transform.rs` (`desugar_shapes` / `into_pitch` / `into_degree` / `mark` の `Apply` の腕)

- [ ] **Step 1: 失敗するテストを書く**

`chord_midi/src/import/rechord.rs` の `mod tests` に足す:

```rust
    #[test]
    fn test_pat_argument_is_a_pattern_expr() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::{Expr, Func};
        let ast = RechordImporter.import("pat(\"oxox\", C)\n").unwrap();
        let Expr::Seq(items) = &ast else { panic!("not a seq") };
        let Expr::Apply(Func::Pat(arg), _, _) = &items[0] else {
            panic!("not a pat apply: {:?}", items[0])
        };
        let Expr::Pattern(p) = arg.as_ref() else {
            panic!("pat の第1引数は Expr::Pattern: {:?}", arg)
        };
        assert_eq!(p.to_string(), "oxox");
    }
```

- [ ] **Step 2: 落ちることを確認**

```bash
cargo test -p chord_midi --lib test_pat_argument_is_a_pattern_expr
```

Expected: FAIL (コンパイルエラー。`Expr::Pattern` が無い / `Func::Pat` が `Pattern` を直接持つ)

- [ ] **Step 3: 型を変える**

`chord_midi/src/model/ast.rs`:

```rust
    /// `"o.ox"` のリズムパターン。`pat` の第1引数になる値
    Pattern(Pattern),
```

を `Expr` に足し、`Func` を:

```rust
pub enum Func {
    /// パワーコード化
    Pow,
    /// オクターブ奏法化
    Oct,
    /// リズムパターン。引数は `Expr::Pattern` か `Expr::Ref`
    Pat(Box<Expr>),
}
```

- [ ] **Step 4: パーサを直す**

`chord_midi/src/import/rechord.rs` の `apply_parser` の `pat` の枝:

```rust
                map(
                    tuple((
                        tag("pat("),
                        space0,
                        pattern_parser,
                        space0,
                        tag(","),
                        space0,
                    )),
                    |(_, _, pattern, _, _, _)| Func::Pat(Box::new(Expr::Pattern(pattern))),
                ),
```

- [ ] **Step 5: パターンを取り出すヘルパを足す**

`chord_midi/src/model/ast.rs` に。`expand` 後は必ず `Expr::Pattern` になっている前提で、そうでなければエラーにする:

```rust
impl Expr {
    /// `Func::Pat` の引数からリズムパターンを取り出す。
    /// `expand` を通した後は必ず `Expr::Pattern` になっている
    pub fn as_pattern(&self) -> Result<&Pattern> {
        match self {
            Expr::Pattern(p) => Ok(p),
            other => Err(anyhow::anyhow!(
                "pat の第1引数がリズムパターンではない: {:?}",
                other
            )),
        }
    }
}
```

- [ ] **Step 6: 下流をコンパイラに拾わせる**

```bash
cargo check --workspace --all-targets 2>&1 | head -40
```

直す箇所:

`chord_midi/src/export/midi.rs` の `interpret`:

```rust
            Expr::Apply(Func::Pat(arg), inner, _) => {
                let pattern = arg.as_pattern()?.clone();
                self.interpret_with_pattern(*inner, &pattern)
            }
```

同 `interpret_with_pattern`:

```rust
            Expr::Apply(Func::Pat(arg), items, _) => {
                let inner_pattern = arg.as_pattern()?.clone();
                self.interpret_with_pattern(*items, &inner_pattern)
            }
```

`chord_midi/src/fingering/mod.rs` の `collect_ast`。ここは `Result` を返すので `?` が使える:

```rust
        Expr::Apply(func, inner, _) => {
            let owned;
            let next = match func {
                Func::Pat(arg) => {
                    owned = arg.as_pattern()?.clone();
                    Some(&owned)
                }
                _ => pattern,
            };
            collect_ast(inner, next, out)?;
        }
```

`chord_midi/src/export/rechord.rs` の Display:

```rust
                    Func::Pat(arg) => match arg.as_ref() {
                        Expr::Pattern(p) => write!(f, "pat(\"{}\", {})", p, inner)?,
                        other => write!(f, "pat({}, {})", other, inner)?,
                    },
```

`Expr::Pattern` 自体の Display も足す (単体で出ることは無いが、網羅性のため):

```rust
            Expr::Pattern(p) => write!(f, "\"{}\"", p),
```

`chord_midi/src/model/transform.rs` の `into_pitch` / `into_degree` / `mark` / `desugar_shapes` / `strip_comments` は `Func` を分解していないので、`Expr::Pattern` の腕だけ `other => other` で拾われる。**`match` が網羅的かコンパイラに確認させること。**

- [ ] **Step 7: テストとゴールデンを確認**

```bash
cargo test --workspace
```

Expected: PASS。ゴールデン3本を含む

- [ ] **Step 8: ゴールデンが無変更であることを確認**

```bash
git diff --stat HEAD -- chord_midi/tests/golden/
```

Expected: 空

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "refactor: Func::Pat の引数を Expr にして Expr::Pattern を足す"
```

---

## Task 2: `$name = ...` の定義と `$name` の参照をパースする

**Files:**
- Modify: `chord_midi/src/model/ast.rs`
- Modify: `chord_midi/src/import/rechord.rs`

- [ ] **Step 1: 失敗するテストを書く**

`chord_midi/src/import/rechord.rs` の `mod tests` に足す:

```rust
    #[test]
    fn test_define_and_ref_parse() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::Expr;
        let ast = RechordImporter
            .import("$riff = pow(C | E D)\n$riff | C |\n")
            .unwrap();
        let Expr::Seq(items) = &ast else { panic!("not a seq") };
        assert_eq!(items.len(), 2, "定義1つと小節列1つ: {:?}", items);
        let Expr::Define(name, body) = &items[0] else {
            panic!("1つ目は Define: {:?}", items[0])
        };
        assert_eq!(name, "riff");
        assert!(matches!(body.as_ref(), Expr::Apply(_, _, _)));
    }

    #[test]
    fn test_ref_in_measure_position() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::Expr;
        let ast = RechordImporter.import("$riff = C\n$riff | Am |\n").unwrap();
        let Expr::Seq(items) = &ast else { panic!("not a seq") };
        // 2つ目は $riff と Am の並び
        let Expr::Seq(body) = &items[1] else {
            // 実装によっては Seq でなく直に並ぶ。どちらでもよいが Ref が現れること
            panic!("2つ目が Seq でない: {:?}", items[1])
        };
        assert!(
            body.iter().any(|e| matches!(e, Expr::Ref(n) if n == "riff")),
            "Ref が現れない: {:?}", body
        );
    }

    #[test]
    fn test_pattern_define_and_ref_in_pat() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::{Expr, Func};
        let ast = RechordImporter
            .import("$eight = \"o.o.o.o.\"\npat($eight, C)\n")
            .unwrap();
        let Expr::Seq(items) = &ast else { panic!("not a seq") };
        let Expr::Define(name, body) = &items[0] else { panic!("not a define") };
        assert_eq!(name, "eight");
        assert!(matches!(body.as_ref(), Expr::Pattern(_)));
        // pat の第1引数が Ref
        fn find_pat_arg(e: &Expr) -> Option<&Expr> {
            match e {
                Expr::Apply(Func::Pat(arg), _, _) => Some(arg.as_ref()),
                Expr::Seq(items) => items.iter().find_map(find_pat_arg),
                _ => None,
            }
        }
        let arg = find_pat_arg(&ast).expect("pat が見つからない");
        assert!(matches!(arg, Expr::Ref(n) if n == "eight"), "{:?}", arg);
    }

    #[test]
    fn test_japanese_define_name() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::Expr;
        let ast = RechordImporter.import("$Aメロ = C | Am |\n$Aメロ\n").unwrap();
        let Expr::Seq(items) = &ast else { panic!("not a seq") };
        let Expr::Define(name, _) = &items[0] else { panic!("not a define") };
        assert_eq!(name, "Aメロ");
    }
```

- [ ] **Step 2: 落ちることを確認**

```bash
cargo test -p chord_midi --lib test_define_and_ref_parse
```

Expected: FAIL (`Expr::Define` が無い)

- [ ] **Step 3: 型を足す**

`chord_midi/src/model/ast.rs` の `Expr` に:

```rust
    /// `$name = ...`
    Define(String, Box<Expr>),
    /// `$name`
    Ref(String),
```

- [ ] **Step 4: パーサを書く**

`chord_midi/src/import/rechord.rs`。`$` に続く名前を読む共通部分と、定義・参照の2つを足す:

```rust
use nom::bytes::complete::take_while1;

/// `$` に続く名前。英数字 + `_` + 日本語。記号は取らない
fn name_parser(s: Span) -> IResult<Span, String> {
    map(
        tuple((
            tag("$"),
            take_while1(|c: char| c.is_alphanumeric() || c == '_'),
        )),
        |(_, name): (Span, Span)| name.fragment().to_string(),
    )(s)
}

/// `$name = <本体>` の1行。本体は Pattern か小節列
#[tracable_parser]
fn define_parser(s: Span) -> IResult<Span, Expr> {
    map(
        tuple((
            name_parser,
            space0,
            tag("="),
            space0,
            alt((
                map(pattern_parser, Expr::Pattern),
                // 本体の小節列。行末まで読む
                measure_seq_parser,
            )),
            space0,
            alt((line_ending, eof)),
            space_or_line_ending_many0,
        )),
        |(name, _, _, _, body, _, _, _)| Expr::Define(name, Box::new(body)),
    )(s)
}

/// `$name` の参照
#[tracable_parser]
fn ref_parser(s: Span) -> IResult<Span, Expr> {
    map(tuple((name_parser, space0)), |(name, _)| Expr::Ref(name))(s)
}
```

**`take_while1(|c| c.is_alphanumeric() || c == '_')` は日本語を通す。** Rust の `char::is_alphanumeric()` は漢字・かな・カタカナに対して `true` を返す (Phase 1 のゴールデンのファイル名サニタイズで確認済み)。

`measure_seq_parser` は「1行ぶんの小節列」を読むパーサ。`seq_parser` は行をまたいで全部飲むので、定義の本体には使えない。新しく書く:

```rust
/// 1行ぶんの式の列。定義の本体に使う。行末で止まる
fn measure_seq_parser(s: Span) -> IResult<Span, Expr> {
    let mut items: Vec<Expr> = Vec::new();
    let mut rest = s;
    loop {
        if peek(alt((line_ending, eof)))(rest).is_ok() {
            break;
        }
        match alt((apply_parser, ref_parser, measure_parser))(rest) {
            Ok((r, item)) => {
                if r.fragment().len() >= rest.fragment().len() {
                    break;
                }
                items.push(item);
                rest = r;
            }
            Err(_) => break,
        }
    }
    Ok((rest, Expr::Seq(items)))
}
```

**注意**: `measure_parser` は `measure_sep` で `line_ending` を消費するので、行末で止まらない可能性がある。`measure_seq_parser` のループ冒頭で `peek(line_ending)` を見て break しているが、`measure_parser` が改行を食った後は `rest` が次の行の先頭を指す。これでは定義の本体が次の行まで伸びてしまう。

**対策**: 定義の本体を読む前に、その行の範囲を切り出す。`not_line_ending` で1行ぶんの `Span` を取り、その上で `measure_seq_parser` を走らせる方が確実。実装時にどちらが素直か判断し、**選んだ方とその理由を報告すること。**

`seq_parser` のループに `define_parser` と `ref_parser` を足す:

```rust
        match alt((
            comment_parser,
            define_parser,
            apply_parser,
            ref_parser,
            measure_parser,
        ))(rest)
```

**順序が重要**: `define_parser` は `ref_parser` より先。`$riff = ...` の `$riff` だけを `ref_parser` が食ってしまうと定義にならない。

- [ ] **Step 5: 通ることを確認**

```bash
cargo test -p chord_midi --lib import::rechord
```

Expected: PASS。既存のテストも全部通ること

- [ ] **Step 6: 下流の網羅性を通す**

`Expr` に variant が増えたので、`match` が網羅的でない箇所がコンパイルエラーになる。`Define` / `Ref` は `expand` が消すので、この時点では**素直に「中身へ潜る」か「何もしない」**で通す。Task 4 以降で正しく扱う:

- `transform.rs` の `into_pitch` / `into_degree`: `Define(n, body)` は本体を潜る、`Ref` は `other => other`
- `export/midi.rs` / `fingering/mod.rs`: `Define`/`Ref` が来たら**エラーにする** (`expand` を通していれば来ないはず)
- `edit.rs` の `shift_ast`: Task 7 で正式化するので、いったん `Define` は本体を潜る、`Ref` は何もしない
- `export/rechord.rs` の Display: Task 6 で正式化するので、いったん `Define(n, b) => writeln!(f, "${} = {}", n, b)`、`Ref(n) => write!(f, "${} ", n)`

- [ ] **Step 7: 全テストとゴールデンを確認**

```bash
cargo test --workspace && git diff --stat HEAD -- chord_midi/tests/golden/
```

Expected: PASS、ゴールデンの差分は空

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "feat: \$name = ... の定義と \$name の参照をパースする"
```

---

## Task 3: `xN` と `( )` グループ

**Files:**
- Modify: `chord_midi/src/model/ast.rs`
- Modify: `chord_midi/src/import/rechord.rs`

- [ ] **Step 1: 失敗するテストを書く**

```rust
    #[test]
    fn test_repeat_on_measure() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::Expr;
        let ast = RechordImporter.import("C | Am x3 |\n").unwrap();
        fn find_repeat(e: &Expr) -> Option<u8> {
            match e {
                Expr::Repeat(_, n) => Some(*n),
                Expr::Seq(items) => items.iter().find_map(find_repeat),
                _ => None,
            }
        }
        assert_eq!(find_repeat(&ast), Some(3));
    }

    #[test]
    fn test_repeat_on_group() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::Expr;
        let ast = RechordImporter.import("(C | Am | F | G) x4\n").unwrap();
        fn find_repeat(e: &Expr) -> Option<(usize, u8)> {
            match e {
                Expr::Repeat(inner, n) => match inner.as_ref() {
                    Expr::Seq(items) => Some((items.len(), *n)),
                    _ => Some((1, *n)),
                },
                Expr::Seq(items) => items.iter().find_map(find_repeat),
                _ => None,
            }
        }
        assert_eq!(find_repeat(&ast), Some((4, 4)));
    }

    #[test]
    fn test_repeat_on_ref() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::Expr;
        let ast = RechordImporter.import("$a = C\n$a x2\n").unwrap();
        fn has_repeat_of_ref(e: &Expr) -> bool {
            match e {
                Expr::Repeat(inner, 2) => matches!(inner.as_ref(), Expr::Ref(n) if n == "a"),
                Expr::Seq(items) => items.iter().any(has_repeat_of_ref),
                _ => false,
            }
        }
        assert!(has_repeat_of_ref(&ast), "{:?}", ast);
    }

    #[test]
    fn test_repeat_rejects_out_of_range() {
        use crate::import::{Importer, RechordImporter};
        // x0 / x1 は意味がないのでエラー
        assert!(RechordImporter.import("C x0 |\n").is_err());
        assert!(RechordImporter.import("C x1 |\n").is_err());
        // 33 以上もエラー
        assert!(RechordImporter.import("C x33 |\n").is_err());
    }

    #[test]
    fn test_group_without_repeat() {
        use crate::import::{Importer, RechordImporter};
        // グループ単体も書ける (リピートなし)
        assert!(RechordImporter.import("(C | Am)\n").is_ok());
    }

    #[test]
    fn test_paren_is_not_confused_with_func() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::{Expr, Func};
        // pow( は関数、( はグループ
        let ast = RechordImporter.import("pow(C) (D | E) x2\n").unwrap();
        fn has_pow(e: &Expr) -> bool {
            match e {
                Expr::Apply(Func::Pow, _, _) => true,
                Expr::Seq(items) => items.iter().any(has_pow),
                Expr::Repeat(inner, _) => has_pow(inner),
                _ => false,
            }
        }
        assert!(has_pow(&ast), "{:?}", ast);
    }
```

- [ ] **Step 2: 落ちることを確認**

```bash
cargo test -p chord_midi --lib test_repeat_on_measure
```

Expected: FAIL (`Expr::Repeat` が無い)

- [ ] **Step 3: 型を足す**

```rust
    /// `xN`。N は 2〜32
    Repeat(Box<Expr>, u8),
```

- [ ] **Step 4: パーサを書く**

```rust
use nom::character::complete::digit1;

/// `( <小節列> )` のグループ。`pow(` などの関数名タグを先に試すこと
#[tracable_parser]
fn group_parser(s: Span) -> IResult<Span, Expr> {
    map(
        tuple((
            tag("("),
            space0,
            many1(alt((apply_parser, group_parser, ref_parser, measure_parser))),
            space0,
            tag(")"),
            space0,
        )),
        |(_, _, items, _, _, _)| Expr::Seq(items),
    )(s)
}

/// `xN` の接尾辞。2〜32 だけ受け付ける
fn repeat_suffix(s: Span) -> IResult<Span, u8> {
    map_res(
        tuple((tag("x"), digit1, space0)),
        |(_, n, _): (Span, Span, Span)| {
            let n: u32 = n.fragment().parse()?;
            if (2..=32).contains(&n) {
                Ok(n as u8)
            } else {
                Err(anyhow::anyhow!("x{} は 2〜32 の範囲外", n))
            }
        },
    )(s)
}

/// 1項目 + 省略可能な `xN`
#[tracable_parser]
fn item_parser(s: Span) -> IResult<Span, Expr> {
    map(
        tuple((
            alt((apply_parser, group_parser, ref_parser, measure_parser)),
            opt(repeat_suffix),
        )),
        |(item, n)| match n {
            Some(n) => Expr::Repeat(Box::new(item), n),
            None => item,
        },
    )(s)
}
```

`seq_parser` と `measure_seq_parser` と `group_parser` の中身を `item_parser` 経由にする。

**`(` の曖昧さ**: `apply_parser` が `pow(` `oct(` `pat(` のタグを先に試すので、`group_parser` より前に置けば衝突しない。`alt` の順序を守ること。

**`xN` と小節の区切りの関係に注意**: `C | Am x3 |` の `x3` は `Am` の後ろ、`|` の前に来る。`measure_parser` は `measure_sep` (`|` や改行) まで読むので、`x3` が小節の一部として食われないか確認すること。食われるなら `node_parser` が `x` を受け付けないことを利用して、`measure_parser` の後ろで `repeat_suffix` を見る形にする。**実装して確かめ、どうなったか報告すること。**

- [ ] **Step 5: 通ることを確認**

```bash
cargo test -p chord_midi --lib import::rechord
```

Expected: PASS

- [ ] **Step 6: 下流の網羅性を通す**

`Repeat` は `expand` が消すので、Task 2 と同じ方針:
- `transform.rs`: 中身を潜る
- `export/midi.rs` / `fingering/mod.rs`: エラー
- `edit.rs`: 中身を潜る
- `export/rechord.rs`: いったん `Repeat(inner, n) => write!(f, "{} x{}", inner, n)`

- [ ] **Step 7: 全テストとゴールデンを確認**

```bash
cargo test --workspace && git diff --stat HEAD -- chord_midi/tests/golden/
```

Expected: PASS、ゴールデンの差分は空

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "feat: xN のリピートと ( ) のグループをパースする"
```

---

## Task 4: `model/expand.rs` — 環境・展開・循環検出・型検査

**Files:**
- Create: `chord_midi/src/model/expand.rs`
- Modify: `chord_midi/src/model/mod.rs`

- [ ] **Step 1: 失敗するテストを書く**

`chord_midi/src/model/expand.rs` に (ファイルごと新規):

```rust
#[cfg(test)]
mod tests {
    use super::expand;
    use crate::import::{Importer, RechordImporter};
    use crate::model::ast::Expr;

    fn expanded(code: &str) -> Expr {
        expand(RechordImporter.import(code).unwrap()).unwrap()
    }

    fn err(code: &str) -> String {
        expand(RechordImporter.import(code).unwrap())
            .unwrap_err()
            .to_string()
    }

    /// 展開後に Define / Ref / Repeat が残っていないこと
    fn is_clean(e: &Expr) -> bool {
        match e {
            Expr::Define(_, _) | Expr::Ref(_) | Expr::Repeat(_, _) => false,
            Expr::Seq(items) => items.iter().all(is_clean),
            Expr::Apply(func, inner, _) => {
                let arg_ok = match func {
                    crate::model::ast::Func::Pat(a) => is_clean(a),
                    _ => true,
                };
                arg_ok && is_clean(inner)
            }
            Expr::Keyed(_, inner) | Expr::Scaled(_, inner) => is_clean(inner),
            _ => true,
        }
    }

    fn count_measures(e: &Expr) -> usize {
        match e {
            Expr::Measure(_, _) => 1,
            Expr::Seq(items) => items.iter().map(count_measures).sum(),
            Expr::Apply(_, inner, _) => count_measures(inner),
            Expr::Keyed(_, inner) | Expr::Scaled(_, inner) => count_measures(inner),
            _ => 0,
        }
    }

    #[test]
    fn test_ref_is_replaced() {
        let e = expanded("$riff = C | E |\n$riff | D |\n");
        assert!(is_clean(&e), "{:?}", e);
        // riff の2小節 + D の1小節
        assert_eq!(count_measures(&e), 3);
    }

    #[test]
    fn test_define_contributes_no_measures() {
        // 定義だけのファイルは 0 小節
        let e = expanded("$riff = C | E |\n");
        assert_eq!(count_measures(&e), 0);
    }

    #[test]
    fn test_repeat_expands() {
        let e = expanded("C x4 |\n");
        assert_eq!(count_measures(&e), 4);
    }

    #[test]
    fn test_group_repeat_expands() {
        let e = expanded("(C | Am) x3\n");
        assert_eq!(count_measures(&e), 6);
    }

    #[test]
    fn test_nested_refs() {
        let e = expanded("$a = C\n$b = $a | $a\n$b x2\n");
        assert!(is_clean(&e), "{:?}", e);
        assert_eq!(count_measures(&e), 4);
    }

    #[test]
    fn test_pattern_ref_in_pat() {
        use crate::model::ast::Func;
        let e = expanded("$eight = \"o.o.o.o.\"\npat($eight, C)\n");
        assert!(is_clean(&e), "{:?}", e);
        fn pat_arg_is_pattern(e: &Expr) -> bool {
            match e {
                Expr::Apply(Func::Pat(a), _, _) => matches!(a.as_ref(), Expr::Pattern(_)),
                Expr::Seq(items) => items.iter().any(pat_arg_is_pattern),
                _ => false,
            }
        }
        assert!(pat_arg_is_pattern(&e), "{:?}", e);
    }

    #[test]
    fn test_undefined_ref_errors() {
        assert!(err("$nope | C |\n").contains("nope"));
    }

    #[test]
    fn test_cyclic_ref_errors() {
        // 前方参照は禁止なので直接の循環は書けないが、
        // 実装の防御として self 参照を弾くこと
        let msg = err("$a = $a\n$a\n");
        assert!(msg.contains("a"), "{}", msg);
    }

    #[test]
    fn test_forward_reference_errors() {
        // 使う前に定義していない
        let msg = err("$b = $a\n$a = C\n$b\n");
        assert!(msg.contains("a"), "{}", msg);
    }

    #[test]
    fn test_type_mismatch_errors() {
        // Measures をリズム欄に渡した
        let msg = err("$riff = C | E |\npat($riff, C)\n");
        assert!(
            msg.contains("riff") || msg.contains("パターン") || msg.contains("pattern"),
            "{}", msg
        );
    }

    #[test]
    fn test_redefine_errors() {
        let msg = err("$a = C\n$a = D\n$a\n");
        assert!(msg.contains("a"), "{}", msg);
    }
}
```

- [ ] **Step 2: 落ちることを確認**

```bash
cargo test -p chord_midi --lib model::expand
```

Expected: FAIL (`expand` が無い)

- [ ] **Step 3: `expand` を書く**

`chord_midi/src/model/expand.rs` の本体:

```rust
use crate::model::ast::{Expr, Func};
use anyhow::{anyhow, Result};
use std::collections::HashMap;

/// 定義の環境。`$name` から本体への写像
#[derive(Default)]
struct Env {
    defs: HashMap<String, Expr>,
}

/// `Define` を環境に収容して `Seq` から除き、`Ref` を値に置き換え、
/// `Repeat` を n 回に展開する。
///
/// 通過後の `Expr` には `Define` / `Ref` / `Repeat` が残らない。
/// 前方参照は禁止なので、`Seq` を前から順に舐めるだけで解決できる。
pub fn expand(expr: Expr) -> Result<Expr> {
    let mut env = Env::default();
    expand_in(expr, &mut env)
}

fn expand_in(expr: Expr, env: &mut Env) -> Result<Expr> {
    match expr {
        Expr::Seq(items) => {
            let mut out = Vec::new();
            for item in items {
                match item {
                    // 定義は環境に入れて、式としては 0 小節を返す
                    Expr::Define(name, body) => {
                        if env.defs.contains_key(&name) {
                            return Err(anyhow!("${} は既に定義されている", name));
                        }
                        // 本体の中の参照も、その時点の環境で解決しておく。
                        // これで自己参照と前方参照が自然に弾かれる
                        let body = expand_in(*body, env)?;
                        env.defs.insert(name, body);
                    }
                    other => out.push(expand_in(other, env)?),
                }
            }
            Ok(Expr::Seq(out))
        }
        Expr::Ref(name) => env
            .defs
            .get(&name)
            .cloned()
            .ok_or_else(|| anyhow!("${} は定義されていない", name)),
        Expr::Repeat(inner, n) => {
            let inner = expand_in(*inner, env)?;
            Ok(Expr::Seq(
                std::iter::repeat_with(|| inner.clone()).take(n as usize).collect(),
            ))
        }
        Expr::Apply(Func::Pat(arg), inner, br) => {
            let arg = expand_in(*arg, env)?;
            // 型検査: pat の第1引数は Pattern でなければならない
            if !matches!(arg, Expr::Pattern(_)) {
                return Err(anyhow!(
                    "pat の第1引数はリズムパターンでなければならない: {:?}",
                    arg
                ));
            }
            Ok(Expr::Apply(
                Func::Pat(Box::new(arg)),
                Box::new(expand_in(*inner, env)?),
                br,
            ))
        }
        Expr::Apply(func, inner, br) => {
            Ok(Expr::Apply(func, Box::new(expand_in(*inner, env)?), br))
        }
        Expr::Keyed(p, inner) => Ok(Expr::Keyed(p, Box::new(expand_in(*inner, env)?))),
        Expr::Scaled(s, inner) => Ok(Expr::Scaled(s, Box::new(expand_in(*inner, env)?))),
        Expr::Define(name, _) => Err(anyhow!("${} の定義が式の位置にある", name)),
        other => Ok(other),
    }
}
```

**循環参照について**: 定義の本体をその場で展開する (`expand_in(*body, env)?` を環境に入れる前に呼ぶ) ので、`$a = $a` は「まだ `a` が環境に無い」→「定義されていない」エラーになる。前方参照も同じ理屈で弾かれる。**専用の循環検出スタックは要らない。** ただしテストで両方が確かに落ちることを確認すること。

`Expr` に `Clone` が要る。`ast.rs` の `Expr` の derive に `Clone` を足すこと。

- [ ] **Step 4: モジュールを登録する**

`chord_midi/src/model/mod.rs` に `pub mod expand;` を足す。

- [ ] **Step 5: 通ることを確認**

```bash
cargo test -p chord_midi --lib model::expand
```

Expected: PASS

- [ ] **Step 6: 全テストを確認**

```bash
cargo test --workspace
```

Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: 定義の展開と参照の解決を行う expand を足す"
```

---

## Task 5: `resolve` のパイプラインに `expand` を挟む

**Files:**
- Modify: `chord_midi/src/model/transform.rs`

- [ ] **Step 1: 失敗するテストを書く**

`chord_midi/src/model/transform.rs` の `mod tests` に足す:

```rust
    #[test]
    fn test_resolve_expands_refs() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::Expr;
        let ast = RechordImporter
            .import("$riff = C | E |\n$riff | D |\n")
            .unwrap()
            .resolve()
            .unwrap();
        fn count_measures(e: &Expr) -> usize {
            match e {
                Expr::Measure(_, _) => 1,
                Expr::Seq(items) => items.iter().map(count_measures).sum(),
                Expr::Apply(_, inner, _) => count_measures(inner),
                _ => 0,
            }
        }
        assert_eq!(count_measures(&ast), 3);
    }

    #[test]
    fn test_resolve_reports_undefined_ref() {
        use crate::import::{Importer, RechordImporter};
        let e = RechordImporter.import("$nope | C |\n").unwrap().resolve();
        assert!(e.is_err());
    }
```

- [ ] **Step 2: 落ちることを確認**

Expected: FAIL (`resolve()` が `Expr` を返していて `.unwrap()` できない)

- [ ] **Step 3: `resolve` を `Result` にする**

`chord_midi/src/model/transform.rs`:

```rust
    /// 定義を展開し、key/scale のスコープを解いて度数を実音にし、
    /// Comment を落とした再生可能な AST を返す。
    ///
    /// `key:` が無いときの tonic は C。
    pub fn resolve(self) -> Result<Expr> {
        let expanded = crate::model::expand::expand(self)?;
        let transposed = expanded.into_pitch(Pitch::C, Scale::Major);
        // pow は音程の話なのでここで潰す。pat はリズムの話なので残す
        let desugared = desugar_shapes(transposed)
            .into_iter()
            .next()
            .expect("desugar_shapes always yields at least one node for a Seq");
        Ok(strip_comments(desugared))
    }
```

- [ ] **Step 4: 呼び出し側を直す**

`resolve()` の呼び出し箇所を全部 `?` か `unwrap` にする:

```bash
grep -rn "\.resolve()" --include='*.rs' chord_midi/src chord_midi/tests chord_midi_cli/src chord_midi_web/src
```

- `export/midi.rs` の `export`: `ast.resolve()?`
- `fingering/mod.rs`: 呼んでいれば `?`
- `chord_midi_cli/src/main.rs`: `?`
- `chord_midi_web/src/main.rs`: `compile()` は `Result<_, String>` を返すので `.map_err(|e| e.to_string())?`
- `chord_midi/tests/golden.rs`: `midi_bytes` が `Result<Vec<u8>, String>` を返す形になっているので、そこで拾う
- 各テスト: `.resolve().unwrap()`

**`resolve_is_idempotent` を壊さないこと。** `expand` は展開後の AST に対しては何もしない (Define/Ref/Repeat が無いので) はずだが、**確認すること。**

- [ ] **Step 5: 全テストとゴールデンを確認**

```bash
cargo test --workspace && git diff --stat HEAD -- chord_midi/tests/golden/
```

Expected: PASS、ゴールデンの差分は空。**ここでゴールデンが落ちたら `expand` が既存の進行を変えている**

- [ ] **Step 6: web と CLI のビルドを確認**

```bash
cargo check --workspace --all-targets && (cd chord_midi_web && cargo check --target wasm32-unknown-unknown)
```

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: resolve のパイプラインに expand を挟み Result を返す"
```

---

## Task 6 / 7 / 8 は並列に実行してよい

触るファイルが交わらない。それぞれ別の worktree で走らせて、終わってからマージする。

---

## Task 6: Display と括弧の導出 (並列可)

**Files:**
- Modify: `chord_midi/src/export/rechord.rs`

- [ ] **Step 1: 失敗するテストを書く**

`chord_midi/src/export/rechord.rs` の `mod tests` に足す。`roundtrip` ヘルパは既にある:

```rust
    #[test]
    fn test_define_roundtrip() {
        assert_eq!(
            roundtrip("$riff = pow(C | E D)\n$riff | C |"),
            "$riff = pow(C | E D)\n$riff | C |"
        );
    }

    #[test]
    fn test_pattern_define_roundtrip() {
        assert_eq!(
            roundtrip("$eight = \"o.o.o.o.\"\npat($eight, C)"),
            "$eight = \"o.o.o.o.\"\npat($eight, C)"
        );
    }

    #[test]
    fn test_repeat_on_measure_roundtrip() {
        // 小節1つの繰り返しには括弧を付けない
        assert_eq!(roundtrip("C x4 |"), "C x4 |");
    }

    #[test]
    fn test_repeat_on_group_roundtrip() {
        // 2小節以上の繰り返しには括弧を付ける
        assert_eq!(roundtrip("(C | Am) x3"), "(C | Am) x3");
    }

    #[test]
    fn test_repeat_on_ref_roundtrip() {
        assert_eq!(roundtrip("$a = C\n$a x2"), "$a = C\n$a x2");
    }

    #[test]
    fn test_japanese_define_roundtrip() {
        assert_eq!(
            roundtrip("$Aメロ = C | Am |\n$Aメロ x2"),
            "$Aメロ = C | Am |\n$Aメロ x2"
        );
    }

    #[test]
    fn test_define_under_key_roundtrip() {
        assert_eq!(
            roundtrip("key: C\n$a = I | IV |\n$a x2"),
            "key: C\n$a = I | IV |\n$a x2"
        );
    }
```

- [ ] **Step 2: 落ちることを確認**

```bash
cargo test -p chord_midi --lib export::rechord
```

- [ ] **Step 3: Display を書く**

`chord_midi/src/export/rechord.rs` の `impl Display for Expr` に:

```rust
            Expr::Pattern(p) => write!(f, "\"{}\"", p),
            Expr::Define(name, body) => {
                let body = body.to_string();
                // 小節の Display は末尾に " | " を出す。定義の行末は揃える
                let body = body.trim_end();
                writeln!(f, "${} = {}", name, body)
            }
            Expr::Ref(name) => write!(f, "${} ", name),
            Expr::Repeat(inner, n) => {
                // 2小節以上の Seq を繰り返すときだけ括弧を出す。
                // 1項目なら括弧は要らない (`C x4`)
                let needs_paren = matches!(inner.as_ref(), Expr::Seq(items) if items.len() >= 2);
                let body = inner.to_string();
                let body = body.trim_end();
                let body = body.strip_suffix('|').unwrap_or(body).trim_end();
                if needs_paren {
                    write!(f, "({}) x{} ", body, n)
                } else {
                    write!(f, "{} x{} ", body, n)
                }
            }
```

**`Func::Pat` の Display は Task 1 で `Expr` を出す形にしてある。** `Expr::Ref` が来たら `pat($eight, ...)` と出る。**`Expr::Pattern` のときだけ `"..."` を付ける形になっているか確認すること** (二重引用符が二重に付かないように)。

- [ ] **Step 4: 通ることを確認**

```bash
cargo test -p chord_midi --lib export::rechord
```

- [ ] **Step 5: 全テストとゴールデンを確認**

```bash
cargo test --workspace && git diff --stat HEAD -- chord_midi/tests/golden/
```

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: 定義・参照・リピートの Display と括弧の導出"
```

---

## Task 7: 移調が定義を貫通する (並列可)

**Files:**
- Modify: `chord_midi/src/edit.rs`

- [ ] **Step 1: 失敗するテストを書く**

```rust
    #[test]
    fn test_shift_key_transposes_inside_define() {
        assert_eq!(
            shift_key("$riff = C | E |\n$riff | D |", 1),
            "$riff = C# | F |\n$riff | D# |"
        );
    }

    #[test]
    fn test_shift_key_keeps_pattern_define() {
        // リズムの定義は移調の対象外
        assert_eq!(
            shift_key("$eight = \"o.o.o.o.\"\npat($eight, C)", 1),
            "$eight = \"o.o.o.o.\"\npat($eight, C#)"
        );
    }

    #[test]
    fn test_shift_key_transposes_inside_repeat() {
        assert_eq!(shift_key("C x4 |", 1), "C# x4 |");
        assert_eq!(shift_key("(C | Am) x2", 1), "(C# | A#m) x2");
    }

    #[test]
    fn test_shift_key_round_trip_with_defines() {
        let src = "key: C\n$a = I | IV |\n$a x2";
        assert_eq!(shift_key(&shift_key(src, 1), -1), src);
    }
```

- [ ] **Step 2: 落ちることを確認**

```bash
cargo test -p chord_midi --lib edit
```

- [ ] **Step 3: `shift_ast` に腕を足す**

```rust
        Expr::Define(_, body) => shift_ast(body, delta, seen),
        // 参照は名前だけなので移調の対象が無い。定義の側でずれる
        Expr::Ref(_) => {}
        Expr::Repeat(inner, _) => shift_ast(inner, delta, seen),
        // リズムに音程は無い
        Expr::Pattern(_) => {}
```

**`match` を網羅的に保つこと。`_ => {}` を使わないこと。**

- [ ] **Step 4: 通ることを確認**

```bash
cargo test -p chord_midi --lib edit
```

- [ ] **Step 5: 全テストとゴールデンを確認**

```bash
cargo test --workspace && git diff --stat HEAD -- chord_midi/tests/golden/
```

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: 移調が定義とリピートを貫通する"
```

---

## Task 8: `MeasureRef.section` (並列可)

展開で `$A` の中身が平らになるので、プレビューのカードに「どのセクション由来か」を出すには展開時に名前を伝える必要がある。

**Files:**
- Modify: `chord_midi/src/model/ast.rs` (`Expr::Section` を足すか、別の手段)
- Modify: `chord_midi/src/model/expand.rs`
- Modify: `chord_midi/src/fingering/mod.rs`

- [ ] **Step 1: 設計を決める**

展開後に「この小節は `$A` 由来」という情報を残す手段は2つ:

- **案A: `Expr::Section(String, Box<Expr>)` を足す。** `expand` が `Ref(name)` を `Section(name, 中身)` に置き換える。`fingering::collect_ast` が `Section` を見て名前を降ろす。MIDI 側は中身へ潜るだけ
- **案B: `MeasureRef` を作るときに別の経路で名前を渡す。** `expand` が `(Expr, Vec<区間情報>)` を返す

**案A を採ること。** 木の中に情報が残るので JSON 表示にも出るし、`fingering` 以外からも使える。案B は区間のインデックス管理が壊れやすい。

- [ ] **Step 2: 失敗するテストを書く**

`chord_midi/src/fingering/mod.rs` の `mod tests` に足す:

```rust
    #[test]
    fn measure_refs_carry_section_name() {
        use crate::import::{Importer, RechordImporter};
        let ast = RechordImporter
            .import("$A = C | Am |\n$A | F |\n")
            .unwrap()
            .resolve()
            .unwrap();
        let ms = measure_refs(&ast).unwrap();
        assert_eq!(ms.len(), 3);
        assert_eq!(ms[0].section.as_deref(), Some("A"));
        assert_eq!(ms[1].section.as_deref(), Some("A"));
        // $A の外の小節には名前が付かない
        assert_eq!(ms[2].section, None);
    }
```

- [ ] **Step 3: `Expr::Section` を足す**

```rust
    /// `$name` を展開した跡。どの定義から来た小節かを残す。
    /// パーサは作らない。`expand` だけが作る
    Section(String, Box<Expr>),
```

- [ ] **Step 4: `expand` が `Ref` を `Section` に置き換える**

```rust
        Expr::Ref(name) => {
            let body = env
                .defs
                .get(&name)
                .cloned()
                .ok_or_else(|| anyhow!("${} は定義されていない", name))?;
            Ok(Expr::Section(name, Box::new(body)))
        }
```

**ただし `pat` の第1引数に来た `Ref` は `Section` で包んではいけない。** 型検査が `Expr::Pattern` を期待している。`Func::Pat` の腕では包まないようにするか、型検査で `Section` の中身を見るようにすること。**どちらにしたか報告すること。**

- [ ] **Step 5: `MeasureRef` に `section` を足す**

`chord_midi/src/fingering/mod.rs`:

```rust
pub struct MeasureRef {
    pub chords: Vec<ChordRef>,
    pub line_break: bool,
    pub pattern: Option<Pattern>,
    /// この小節が由来する `$name`。定義の外なら None
    pub section: Option<String>,
}
```

`collect_ast` に `section: Option<&str>` を引数で降ろす。`Expr::Section(name, inner)` の腕で名前を差し替える。**入れ子の `Section` は内側が勝つ** (一番近い定義名が付く)。

- [ ] **Step 6: 下流を直す**

`export/midi.rs` の `interpret` に `Section` の腕を足す (中身へ潜るだけ)。`transform.rs` / `edit.rs` / `export/rechord.rs` も網羅性のため腕が要る。

**`export/rechord.rs` の Display**: `Section` は展開の跡なので、Display では**中身をそのまま出す** (`write!(f, "{}", inner)`)。`resolve()` 前の AST には `Section` が現れないので round-trip には影響しない。

- [ ] **Step 7: 通ることを確認**

```bash
cargo test -p chord_midi --lib fingering
```

- [ ] **Step 8: 全テストとゴールデンを確認**

```bash
cargo test --workspace && git diff --stat HEAD -- chord_midi/tests/golden/
```

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "feat: 展開した小節にセクション名を残す"
```

---

## Task 9: lint — 定義より前に小節行

**Files:**
- Create: `chord_midi/src/lint.rs`
- Modify: `chord_midi/src/lib.rs`

- [ ] **Step 1: 失敗するテストを書く**

`chord_midi/src/lint.rs` に:

```rust
#[cfg(test)]
mod tests {
    use super::lint;
    use crate::import::{Importer, RechordImporter};

    fn warnings(code: &str) -> Vec<String> {
        lint(&RechordImporter.import(code).unwrap())
    }

    #[test]
    fn test_no_warning_when_defines_come_first() {
        assert!(warnings("$a = C\n$a | D |\n").is_empty());
    }

    #[test]
    fn test_warns_when_measure_precedes_define() {
        let w = warnings("C |\n$a = D\n$a\n");
        assert_eq!(w.len(), 1);
        assert!(w[0].contains("a"), "{}", w[0]);
    }

    #[test]
    fn test_no_warning_without_defines() {
        assert!(warnings("C | Am |\n").is_empty());
    }

    #[test]
    fn test_comment_and_directive_do_not_count_as_measures() {
        assert!(warnings("# memo\nkey: C\n$a = I\n$a\n").is_empty());
    }
}
```

- [ ] **Step 2: 落ちることを確認**

```bash
cargo test -p chord_midi --lib lint
```

- [ ] **Step 3: `lint` を書く**

```rust
use crate::model::ast::Expr;

/// 記法として正しいが、読みにくい書き方を警告する。
/// エラーではないので、パースも再生も止めない
pub fn lint(expr: &Expr) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen_measure = false;
    walk(expr, &mut seen_measure, &mut out);
    out
}

fn walk(expr: &Expr, seen_measure: &mut bool, out: &mut Vec<String>) {
    match expr {
        Expr::Seq(items) => {
            for item in items {
                walk(item, seen_measure, out);
            }
        }
        Expr::Keyed(_, inner) | Expr::Scaled(_, inner) => walk(inner, seen_measure, out),
        Expr::Measure(_, _) => *seen_measure = true,
        Expr::Apply(_, _, _) | Expr::Repeat(_, _) | Expr::Ref(_) => *seen_measure = true,
        Expr::Define(name, _) => {
            if *seen_measure {
                out.push(format!(
                    "${} の定義が小節より後ろにある。素材は曲より前にまとめると読みやすい",
                    name
                ));
            }
        }
        _ => {}
    }
}
```

- [ ] **Step 4: 公開する**

`chord_midi/src/lib.rs` に `pub mod lint;` を足す。

- [ ] **Step 5: CLI と web に出す**

`chord_midi_cli/src/main.rs`: パース後に `lint` を呼び、警告があれば `eprintln!` で出す (標準エラーなので、パイプで繋いでも邪魔にならない)。

`chord_midi_web/src/main.rs`: `compile()` が警告も返すようにし、エラー表示とは別のスタイル (`.warning`) で出す。`style.css` に `.warning` を足す。

- [ ] **Step 6: 全テストを確認**

```bash
cargo test --workspace && (cd chord_midi_web && cargo check --target wasm32-unknown-unknown)
```

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: 定義が小節より後ろにあるときに警告する"
```

---

## Task 10: ゴールデンの追加と README

**Files:**
- Modify: `chord_midi/tests/golden.rs`
- Modify: `README.md`

- [ ] **Step 1: 新記法のゴールデンを足す**

`chord_midi/tests/golden.rs` の `SAMPLES` に:

```rust
    // Phase 2 の記法
    ("define_ref", "$riff = pow(C | E D)\n$riff | C |\n"),
    ("define_pattern", "$eight = \"o.o.o.o.\"\npat($eight, C | Am)\n"),
    ("repeat_measure", "C x4 |\n"),
    ("repeat_group", "(C | Am | F | G) x2\n"),
    ("nested_define", "$a = C\n$b = $a | $a\n$b x2\n"),
    ("define_under_key", "key: C\n$a = I | IV |\n$a x2\n"),
```

- [ ] **Step 2: 生成して、既存が無変更であることを確認**

```bash
UPDATE_GOLDEN=1 cargo test -p chord_midi --test golden
git status --short chord_midi/tests/golden/
```

Expected: **新規追加 (`??`) だけ。既存の `.mid` に `M` が付いていたら後方互換の破壊。** 付いていたら手を止めて報告すること

- [ ] **Step 3: 展開結果が手で書いた形と一致することを確認**

新記法のサンプルと、それを手で展開した形が**同じ MIDI を出す**ことをテストで固定する。`chord_midi/tests/golden.rs` に足す:

```rust
/// 定義を使った書き方と、手で展開した書き方が同じ MIDI を出すこと
#[test]
fn defines_match_hand_expansion() {
    let pairs: &[(&str, &str, &str)] = &[
        ("riff", "$riff = pow(C | E D)\n$riff | C |\n", "pow(C | E D) C |\n"),
        ("repeat", "C x4 |\n", "C | C | C | C |\n"),
        ("group", "(C | Am) x2\n", "C | Am | C | Am |\n"),
        ("nested", "$a = C\n$b = $a | $a\n$b x2\n", "C | C | C | C |\n"),
    ];
    let mut failures = Vec::new();
    for (name, with_define, expanded) in pairs {
        match (midi_bytes(name, with_define), midi_bytes(name, expanded)) {
            (Ok(a), Ok(b)) if a == b => {}
            (Ok(a), Ok(b)) => failures.push(format!(
                "[{}] 定義版と展開版の MIDI が違う ({} bytes vs {} bytes)",
                name, a.len(), b.len()
            )),
            (Err(e), _) | (_, Err(e)) => failures.push(format!("[{}] {}", name, e)),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
```

**これが Phase 2 の中核の検証。** 定義は「書き方を短くするだけ」で、鳴る音は手で展開したものと1バイトも変わらないはず。

- [ ] **Step 4: README を書く**

`## 関数` の前に節を足す:

```markdown
## 定義と参照

`$名前 = ...` で進行やリズムに名前を付け、`$名前` で呼び出せる。
ファイル全体が `main()` の本体で、**定義行以外のすべてが曲**になる。

    key: C

    $eight = "o.o.o.o."          # リズム
    $riff  = pow(C | E D)        # 小節列
    $A     = pat($eight, C | Am | F | G)
    $B     = $riff x2 | pat($eight, F | G)

    $A x2 | $B | $A              # ここが曲

- 定義は行頭に書く。本体は1行
- 名前は英数字・`_`・日本語が使える (`$Aメロ`)
- 使う前に定義する。前方参照と循環参照はエラー
- `$名前` は小節の位置、`pat` の第1引数、関数の中に書ける

### 繰り返し

`xN` は直前の1項目を N 回繰り返す。`N` は 2〜32。

    C | Am x3 |          # C, Am, Am, Am （直前の小節だけ）
    $A x2 | $B           # A A B

`( )` で範囲を明示できる。

    (C | Am | F | G) x4  # 4小節を4回
```

- [ ] **Step 5: README の例が動くことを確認**

README に載せた例をそのまま CLI に食わせて、エラーにならないことを確認すること。

- [ ] **Step 6: 全テストを確認**

```bash
cargo test --workspace
```

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "test: 定義を使った書き方と手で展開した書き方が同じ MIDI を出すことを固定する"
```

---

## Task 11: 整形と締め

**Files:**
- 全体

- [ ] **Step 1: 整形する**

```bash
cargo fmt --all
(cd chord_midi_web && cargo fmt)
```

- [ ] **Step 2: 差分が整形だけであることを確認**

```bash
git diff | grep -E "^[+-]" | grep -v "^[+-][+-]" | head -40
```

- [ ] **Step 3: 全部を通し直す**

```bash
cargo test --workspace
cargo fmt --all -- --check
(cd chord_midi_web && cargo fmt -- --check && trunk build)
```

Expected: 全部成功

- [ ] **Step 4: ゴールデンの既存分が無変更であることを確認**

```bash
git diff --stat main -- chord_midi/tests/golden/
```

Expected: **新規追加だけ。既存の `.mid` に差分が出ていたら後方互換の破壊**

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "style: cargo fmt を当てる"
```

---

## 完了条件

- `cargo test --workspace` が通る
- `cargo fmt --all -- --check` と `(cd chord_midi_web && cargo fmt -- --check)` が通る
- `trunk build` が通る
- **Phase 1 から引き継いだゴールデン24件が1バイトも変わっていない**
- `defines_match_hand_expansion` が通る (定義版と手で展開した版が同じ MIDI)
- `expand` 通過後に `Define` / `Ref` / `Repeat` が残らない
- README の例がそのまま動く

## Phase 3

GUI (`reference.rs`、記法パネル、`RhythmGrid`、`FuncPalette`、`DefineChips`、`insert_at_cursor`、CLI の `--help-syntax`)。部品が独立しているので並列に実装できる。別の計画に書く。
