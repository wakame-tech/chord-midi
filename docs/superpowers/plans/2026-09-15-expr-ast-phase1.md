# Phase 1: Ast → Expr 組み替えと key: スコープ化 実装計画

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 平らな項目列だった `Ast` を式の木 `Expr` に組み替え、`key:` / `scale:` を以降を包むスコープにする。記法の機能追加はゼロ。

**Architecture:** `Ast::Score(Vec<Box<Ast>>)` を `Expr::Seq(Vec<Expr>)` に、`Ast::Apply(Func, Vec<Box<Ast>>, bool)` を `Expr::Apply(Func, Box<Expr>, bool)` (中身は `Seq`) に畳む。`Directive` は `Expr::Keyed(Pitch, Box<Expr>)` / `Expr::Scaled(Scale, Box<Expr>)` になり、パーサが「以降を包む」形で組み立てる。`into_pitch` はスコープを再帰下降で受け渡し、`Keyed`/`Scaled` を解いて消す。Display は逆に `key: X` 行へ平らに戻すので、テキストの round-trip は変わらない。

**Tech Stack:** Rust 2021, nom 7 + nom_locate + nom-tracable (パーサ), serde + serde_json (AST 表示), yew 0.21 (web), clap 4 (CLI)

**設計書:** `docs/superpowers/specs/2026-09-15-notation-ux-design.md`

---

## 設計書からの意図的なずれ (3点)

1. **`Apply` は `br: bool` を残す。** 設計書の型は `Apply(Func, Box<Expr>)` だが、Display が関数の後ろの改行を復元するのにこのフラグが要る。実際の型は `Apply(Func, Box<Expr>, bool)`。
2. **`Func::Pat` は Phase 1 では `Pattern` のまま。** 設計書の `Pat(Box<Expr>)` は参照 (`pat($eight, ...)`) のための形で、Phase 2 で変える。Phase 1 で変えても使い道がなく、`export/midi.rs` に「Pattern 以外が来たら」の死んだ分岐が増えるだけ。
3. **既存テストの機械的書き換えは起きる。** 設計書の「既存テストを1つも書き換えずに通す」は**意味の話**に限定する。型名 (`Ast` → `Expr`, `Score` → `Seq`) と `Box` の増減による書き換えは避けられない。禁じるのは**アサーションの中身を変えること**で、`assert_eq!` の期待値が変わったらそれは挙動変更なので立ち止まる。

後方互換の本当の担保は Task 1 のゴールデン MIDI テストが持つ。

---

## ファイル構成

| ファイル | 責務 | 変更 |
|---|---|---|
| `chord_midi/tests/golden.rs` | 既存の進行が同じ MIDI を吐き続けることの回帰テスト | **新規** |
| `chord_midi/tests/golden/*.mid` | そのスナップショット | **新規** |
| `chord_midi/src/model/ast.rs` | 式の型 `Expr` と `Node` の定義 | 大 |
| `chord_midi/src/model/transform.rs` | `into_pitch` / `into_degree` / `resolve` / `desugar_shapes` | 大 |
| `chord_midi/src/import/rechord.rs` | rechord 記法のパーサ。`Keyed`/`Scaled` を組み立てる | 大 |
| `chord_midi/src/import/sexp.rs` | S式のパーサ | 小 |
| `chord_midi/src/export/rechord.rs` | Display。`Keyed` を `key:` 行へ平らに戻す | 中 |
| `chord_midi/src/export/midi.rs` | MIDI 書き出し | 中 |
| `chord_midi/src/fingering/mod.rs` | 押さえ方の抽出 | 中 |
| `chord_midi/src/edit.rs` | 移調 / スケール張り替え | 中 |
| `chord_midi_cli/src/main.rs` | `--print-ast` を JSON + 値省略可の引数に | 中 |
| `chord_midi_web/src/main.rs` | 「解釈結果」に表示3択 | 中 |

---

## Task 1: ゴールデン MIDI 回帰テスト

**この Task を最初に通す。** 以降の組み替えが既存の出力を壊していないことを、これだけが保証する。

**Files:**
- Create: `chord_midi/tests/golden.rs`
- Create: `chord_midi/tests/golden/*.mid` (テストが生成する)

- [ ] **Step 1: ゴールデンテストを書く**

`chord_midi/tests/golden.rs`:

```rust
//! 既存の進行が同じ MIDI を吐き続けることの回帰テスト。
//!
//! スナップショットを作り直すときだけ `UPDATE_GOLDEN=1 cargo test -p chord_midi --test golden`。
//! 差分が出たのに意図した変更でないなら、それは後方互換の破壊。

use chord_midi::export::{Exporter, MidiExporter};
use chord_midi::import::{Importer, RechordImporter};
use chord_midi::presets::PRESETS;
use std::io::BufWriter;
use std::path::PathBuf;

/// README に載っている記法を一通り踏む
const SAMPLES: &[(&str, &str)] = &[
    ("plain", "key: C\nC | Am | F | G |\n"),
    ("degrees", "key: C\nI | VIm | IV | V |\n"),
    ("scale", "key: C\nscale: dorian\nI | III | IV | V |\n"),
    ("modifiers", "CM7 | Dm7 | G7(9) | C6 |\n"),
    ("slash", "C/E | F/G | Am/G | G |\n"),
    ("rest_sustain", "C | = | N.C. | % |\n"),
    ("arp", "arp(C) | arp(Am) |\n"),
    ("transforms", "blk(F) | s25(G7) | sub(G7) | dom(Dm7) |\n"),
    ("pow", "pow(C | E D)\n"),
    ("oct", "oct(A | G#)\n"),
    ("pat", "pat(\"o.o.oxox\", C | E D)\n"),
    ("pat_nested", "pat(\"oxox\", pow(C))\n"),
    ("root_string", "pow(A@6 | D@5 | E@5)\n"),
    ("comment", "# memo\nkey: C\nC | F |\n"),
    ("multiline", "C | Am |\nF | G |\n"),
];

fn midi_bytes(code: &str) -> Vec<u8> {
    let ast = RechordImporter.import(code).expect("parse");
    let mut writer = BufWriter::new(Vec::new());
    MidiExporter { bpm: 120 }
        .export(&mut writer, ast.resolve())
        .expect("export");
    writer.into_inner().expect("flush")
}

fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(format!("{}.mid", name))
}

fn check(name: &str, code: &str) {
    let actual = midi_bytes(code);
    let path = golden_path(name);
    if std::env::var("UPDATE_GOLDEN").is_ok() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &actual).unwrap();
        return;
    }
    let expected = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("golden 未生成 {}: {}。UPDATE_GOLDEN=1 で作る", name, e));
    assert_eq!(
        actual, expected,
        "{} の MIDI が変わった。意図した変更でなければ後方互換の破壊",
        name
    );
}

#[test]
fn samples_are_stable() {
    for (name, code) in SAMPLES {
        check(name, code);
    }
}

#[test]
fn presets_are_stable() {
    for preset in PRESETS.iter() {
        check(
            &format!("preset_{}", preset.name),
            &format!("{}\n", preset.progression),
        );
    }
}
```

- [ ] **Step 2: ゴールデン未生成で落ちることを確認**

```bash
cargo test -p chord_midi --test golden
```

Expected: FAIL、`golden 未生成 plain: ... UPDATE_GOLDEN=1 で作る`

- [ ] **Step 3: スナップショットを生成**

```bash
UPDATE_GOLDEN=1 cargo test -p chord_midi --test golden
```

- [ ] **Step 4: 生成物を確認して通ることを見る**

```bash
ls chord_midi/tests/golden/ && cargo test -p chord_midi --test golden
```

Expected: `.mid` が 15 + プリセット数だけ並び、テストは PASS

プリセット名に `/` や空白が入るとファイル名が壊れる。`ls` で確認して壊れていたら、`check` の呼び出し側でファイル名を作る際にサニタイズする:

```rust
let safe: String = preset.name.chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect();
check(&format!("preset_{}", safe), &format!("{}\n", preset.progression));
```

- [ ] **Step 5: 全テストが通ることを確認**

```bash
cargo test --workspace
```

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add chord_midi/tests/golden.rs chord_midi/tests/golden
git commit -m "test: 既存の進行の MIDI 出力を固定するゴールデンテスト"
```

---

## Task 2: serde 依存と derive

**Files:**
- Modify: `chord_midi/Cargo.toml`
- Modify: `chord_midi/src/model/ast.rs`
- Modify: `chord_midi/src/model/key.rs:7`
- Modify: `chord_midi/src/model/pitch.rs:4`, `chord_midi/src/model/pitch.rs:96`
- Modify: `chord_midi/src/model/scale.rs:5`, `chord_midi/src/model/scale.rs:87`
- Modify: `chord_midi/src/model/modifier.rs:4`
- Test: `chord_midi/src/model/ast.rs` (末尾の `mod tests`)

- [ ] **Step 1: 失敗するテストを書く**

`chord_midi/src/model/ast.rs` の `mod tests` に足す:

```rust
#[test]
fn test_ast_serializes_to_json() {
    use crate::import::{Importer, RechordImporter};
    let ast = RechordImporter.import("key: C\nC | Am |\n").unwrap();
    let json = serde_json::to_string(&ast).unwrap();
    // 構造が見えること。コード名とディレクティブが JSON に現れる
    assert!(json.contains("Measure"), "{}", json);
    assert!(json.contains("\"C\""), "{}", json);
}
```

- [ ] **Step 2: 落ちることを確認**

```bash
cargo test -p chord_midi test_ast_serializes_to_json
```

Expected: FAIL、`failed to resolve: use of undeclared crate or module `serde_json``

- [ ] **Step 3: 依存を足す**

`chord_midi/Cargo.toml` の `[dependencies]` に:

```toml
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

- [ ] **Step 4: derive を足す**

`chord_midi/src/model/ast.rs` の先頭に `use serde::Serialize;` を足し、`Directive` / `Ast` / `Func` / `Pattern` / `Beat` / `Node` / `Transform` / `ChordNode` の `#[derive(...)]` に `Serialize` を加える。例:

```rust
#[derive(Debug, PartialEq, Serialize)]
pub enum Ast {
```

同様に他4ファイルへ `use serde::Serialize;` を足して derive を足す:

- `chord_midi/src/model/key.rs:7` → `#[derive(Debug, PartialEq, Clone, Serialize)]`
- `chord_midi/src/model/pitch.rs:4` → `#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize)]`
- `chord_midi/src/model/pitch.rs:96` → `#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]`
- `chord_midi/src/model/scale.rs:5` → `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]`
- `chord_midi/src/model/scale.rs:87` → `#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]`
- `chord_midi/src/model/modifier.rs:4` → `#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]`

- [ ] **Step 5: 通ることを確認**

```bash
cargo test -p chord_midi test_ast_serializes_to_json
```

Expected: PASS

- [ ] **Step 6: 全テストと web のビルドを確認**

```bash
cargo test --workspace && (cd chord_midi_web && cargo check --target wasm32-unknown-unknown)
```

Expected: どちらも成功。wasm ターゲットが無ければ `rustup target add wasm32-unknown-unknown`

- [ ] **Step 7: Commit**

```bash
git add chord_midi/Cargo.toml Cargo.lock chord_midi/src/model
git commit -m "feat: AST を serde で JSON 化できるようにする"
```

---

## Task 3: `Ast` → `Expr`、`Score` → `Seq(Vec<Expr>)`

`Box` を外す。`Vec` が既にヒープに置くので `Vec<Box<Expr>>` の `Box` は無駄。

**Files:**
- Modify: `chord_midi/src/model/ast.rs`
- Modify: `chord_midi/src/model/transform.rs`
- Modify: `chord_midi/src/import/mod.rs`, `chord_midi/src/import/rechord.rs`, `chord_midi/src/import/sexp.rs`
- Modify: `chord_midi/src/export/mod.rs`, `chord_midi/src/export/rechord.rs`, `chord_midi/src/export/midi.rs`
- Modify: `chord_midi/src/fingering/mod.rs`, `chord_midi/src/edit.rs`
- Modify: `chord_midi_cli/src/main.rs`, `chord_midi_web/src/main.rs`

- [ ] **Step 1: 型を変える**

`chord_midi/src/model/ast.rs`:

```rust
#[derive(Debug, PartialEq, Serialize)]
pub enum Expr {
    Comment(String),
    Directive(Directive),
    /// 小節。2つ目は「この小節の後ろで入力が改行されている」
    Measure(Vec<Node>, bool),
    /// 小節列に関数を適用する。3つ目は「この関数の後ろで改行されている」
    Apply(Func, Box<Expr>, bool),
    /// `|` / 改行で連結した式の列。ファイル全体もこれ
    Seq(Vec<Expr>),
}
```

`Apply` の中身を `Box<Expr>` にするのは Task 4。この Step では `Apply(Func, Vec<Expr>, bool)` に留め、`Box` だけ外す:

```rust
    Apply(Func, Vec<Expr>, bool),
```

- [ ] **Step 2: 機械的に置換する**

```bash
cd /Users/kmt/dev/chord-midi
grep -rl 'Ast' --include='*.rs' chord_midi/src chord_midi_cli/src chord_midi_web/src \
  | xargs sed -i '' -e 's/\bAst::Score\b/Expr::Seq/g' -e 's/\bAst\b/Expr/g'
```

`ast.rs` のモジュール名 (`model::ast`) は変えない。ファイル名も据え置く。

- [ ] **Step 3: `Box` の除去をコンパイラに拾わせる**

```bash
cargo check --workspace 2>&1 | head -60
```

エラーごとに直す。パターンは3つだけ:

- `Box::new(x)` → `x`
- `it.as_ref()` (要素が `&Box<Expr>` だった所) → `it`
- `(*ast)` → `ast`

例、`transform.rs` の `into_pitch`:

```rust
            Expr::Seq(nodes) => Expr::Seq(
                nodes
                    .into_iter()
                    .map(|expr| expr.into_pitch(tonic, scale))
                    .collect::<Vec<_>>(),
            ),
```

例、`transform.rs` の `resolve`:

```rust
        if let Expr::Seq(items) = &self {
            for it in items {
                match it {
                    Expr::Directive(Directive::Key(p)) => tonic = *p,
                    Expr::Directive(Directive::Scale(s)) => scale = *s,
                    _ => {}
                }
            }
        }
```

- [ ] **Step 4: ビルドが通ることを確認**

```bash
cargo check --workspace
```

Expected: 成功

- [ ] **Step 5: テストとゴールデンを確認**

```bash
cargo test --workspace
```

Expected: PASS。**ゴールデンが落ちたらここで止まる** — 型のリネームで MIDI が変わることはあり得ないので、`Box` 除去の過程で構造を壊している

- [ ] **Step 6: web のビルドを確認**

```bash
(cd chord_midi_web && cargo check --target wasm32-unknown-unknown)
```

Expected: 成功

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "refactor: Ast を Expr にリネームし Score を Seq にする"
```

---

## Task 4: `Apply` の中身を `Box<Expr>` (= `Seq`) に畳む

**Files:**
- Modify: `chord_midi/src/model/ast.rs`
- Modify: `chord_midi/src/model/transform.rs`
- Modify: `chord_midi/src/import/rechord.rs`
- Modify: `chord_midi/src/export/rechord.rs`, `chord_midi/src/export/midi.rs`
- Modify: `chord_midi/src/fingering/mod.rs`, `chord_midi/src/edit.rs`

- [ ] **Step 1: 失敗するテストを書く**

`chord_midi/src/import/rechord.rs` の `mod tests` の `test_pow_parser` を、新しい型に合わせて書き換える。アサーションの意味 (「C の小節と E D の小節」) は変えない:

```rust
    #[test]
    fn test_pow_parser() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::{Expr, Func};
        let ast = RechordImporter.import("pow(C | E D)\n").unwrap();
        let Expr::Seq(items) = &ast else { panic!("not a seq") };
        assert_eq!(items.len(), 1);
        let Expr::Apply(Func::Pow, inner, _) = &items[0] else {
            panic!("not a pow apply: {:?}", items[0])
        };
        // 中身は Seq 1つ
        let Expr::Seq(measures) = inner.as_ref() else { panic!("apply の中身は Seq") };
        // C の小節と E D の小節
        assert_eq!(measures.len(), 2);
        let Expr::Measure(nodes, _) = &measures[0] else { panic!("not a measure") };
        assert_eq!(nodes.len(), 1);
        let Expr::Measure(nodes, _) = &measures[1] else { panic!("not a measure") };
        assert_eq!(nodes.len(), 2);
    }
```

- [ ] **Step 2: 落ちることを確認**

```bash
cargo test -p chord_midi test_pow_parser
```

Expected: FAIL (コンパイルエラー。`inner` は `Vec<Expr>` で `as_ref()` が `Seq` にならない)

- [ ] **Step 3: 型を変える**

`chord_midi/src/model/ast.rs`:

```rust
    /// 小節列に関数を適用する。中身は必ず `Seq`。
    /// 3つ目は「この関数の後ろで入力が改行されている」
    Apply(Func, Box<Expr>, bool),
```

- [ ] **Step 4: パーサを直す**

`chord_midi/src/import/rechord.rs` の `apply_parser` の `map` のクロージャ:

```rust
        |(func, items, _, _, _, br, _)| {
            Expr::Apply(func, Box::new(Expr::Seq(items)), br.is_some())
        },
```

- [ ] **Step 5: `desugar_shapes` に Seq の平坦化を足す**

`chord_midi/src/model/transform.rs`。`Apply(Pow, ..)` は親の `Seq` の中で複数要素に展開されるので、`Seq` を親へ差し込む関数が要る:

```rust
/// `Seq` を親のリストへ差し込む。`Apply` の中身は常に `Seq` なので、
/// `pow(C | E D)` を潰したときに `Seq` が入れ子で残らないようにする
fn splice(expr: Expr) -> Vec<Expr> {
    match expr {
        Expr::Seq(items) => items,
        other => vec![other],
    }
}

fn desugar_shapes(expr: Expr) -> Vec<Expr> {
    match expr {
        Expr::Seq(items) => vec![Expr::Seq(
            items.into_iter().flat_map(desugar_shapes).collect(),
        )],
        Expr::Apply(Func::Pow, inner, _) => desugar_shapes(*inner)
            .into_iter()
            .flat_map(splice)
            .map(|item| mark(item, Modifier::Power))
            .collect(),
        Expr::Apply(Func::Oct, inner, _) => desugar_shapes(*inner)
            .into_iter()
            .flat_map(splice)
            .map(|item| mark(item, Modifier::Octave))
            .collect(),
        Expr::Apply(func, inner, br) => {
            let inner = Expr::Seq(desugar_shapes(*inner).into_iter().flat_map(splice).collect());
            vec![Expr::Apply(func, Box::new(inner), br)]
        }
        other => vec![other],
    }
}
```

`mark` / `into_pitch` / `into_degree` の `Apply` の腕も `Box<Expr>` に合わせる:

```rust
        Expr::Apply(func, inner, br) => Expr::Apply(
            func,
            Box::new(mark(*inner, modifier)),
            br,
        ),
```

```rust
            Expr::Apply(func, inner, br) => Expr::Apply(
                func,
                Box::new(inner.into_pitch(tonic, scale)),
                br,
            ),
```

- [ ] **Step 6: Display を直す**

`chord_midi/src/export/rechord.rs` の `Expr::Apply` の腕。中身が `Seq` 1つになったので `join("")` は要らない:

```rust
            Expr::Apply(func, inner, br) => {
                let inner = inner.to_string();
                // 小節の Display は末尾に " | " を出すので、関数の中では
                // 最後の区切りを落として `pow(C | E D)` の形に揃える
                let inner = inner.trim_end();
                let inner = inner.strip_suffix('|').unwrap_or(inner).trim_end();
                match func {
                    Func::Pow => write!(f, "pow({})", inner)?,
                    Func::Oct => write!(f, "oct({})", inner)?,
                    Func::Pat(pattern) => write!(f, "pat(\"{}\", {})", pattern, inner)?,
                }
                if *br {
                    writeln!(f)
                } else {
                    write!(f, " ")
                }
            }
```

- [ ] **Step 7: MIDI / 押さえ方 / 移調の `Apply` の腕を直す**

`chord_midi/src/export/midi.rs` の `interpret`:

```rust
            Expr::Apply(Func::Pat(pattern), inner, _) => {
                let pattern = pattern.clone();
                self.interpret_with_pattern(*inner, &pattern)
            }
            Expr::Apply(_, inner, _) => self.interpret(*inner),
```

同 `interpret_with_pattern`:

```rust
            Expr::Apply(Func::Pat(inner_pattern), items, _) => {
                let inner_pattern = inner_pattern.clone();
                self.interpret_with_pattern(*items, &inner_pattern)
            }
            Expr::Apply(_, items, _) => self.interpret_with_pattern(*items, pattern),
```

`chord_midi/src/fingering/mod.rs` の `collect_ast`:

```rust
        Expr::Apply(func, inner, _) => {
            let next = match func {
                Func::Pat(p) => Some(p),
                _ => pattern,
            };
            collect_ast(inner, next, out)?;
        }
```

`chord_midi/src/edit.rs` の `shift_ast`:

```rust
        Expr::Apply(_, inner, _) => shift_ast(inner, delta, seen),
```

- [ ] **Step 8: テストが通ることを確認**

```bash
cargo test -p chord_midi test_pow_parser
```

Expected: PASS

- [ ] **Step 9: 全テストとゴールデンを確認**

```bash
cargo test --workspace
```

Expected: PASS。**ゴールデンが落ちたら止まる。** 特に `pow` / `oct` / `pat_nested` は `splice` の実装に直結する

- [ ] **Step 10: Commit**

```bash
git add -A
git commit -m "refactor: Apply の中身を Seq 1つに畳む"
```

---

## Task 5: `Keyed` / `Scaled` をパーサで組み立てる

`key:` / `scale:` が「以降を包む」形になる。**この Task で挙動が変わる**: 今までは最後の `key:` が全体に効いていた。

**Files:**
- Modify: `chord_midi/src/model/ast.rs`
- Modify: `chord_midi/src/import/rechord.rs`
- Test: `chord_midi/src/import/rechord.rs` (`mod tests`)

- [ ] **Step 1: 失敗するテストを書く**

`chord_midi/src/import/rechord.rs` の `mod tests` に足す:

```rust
    #[test]
    fn test_key_directive_wraps_the_rest() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::Expr;
        use crate::model::pitch::Pitch;
        let ast = RechordImporter.import("key: C\nC | Am |\n").unwrap();
        let Expr::Seq(items) = &ast else { panic!("not a seq") };
        // key: 以降は Keyed に包まれるので、トップの Seq は要素1つ
        assert_eq!(items.len(), 1);
        let Expr::Keyed(Pitch::C, inner) = &items[0] else {
            panic!("key: は Keyed になる: {:?}", items[0])
        };
        let Expr::Seq(measures) = inner.as_ref() else { panic!("中身は Seq") };
        assert_eq!(measures.len(), 2);
    }

    #[test]
    fn test_comment_before_key_stays_outside() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::Expr;
        let ast = RechordImporter.import("# memo\nkey: C\nC |\n").unwrap();
        let Expr::Seq(items) = &ast else { panic!("not a seq") };
        assert_eq!(items.len(), 2);
        assert!(matches!(&items[0], Expr::Comment(_)));
        assert!(matches!(&items[1], Expr::Keyed(_, _)));
    }

    #[test]
    fn test_scale_nests_inside_key() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::Expr;
        let ast = RechordImporter.import("key: C\nscale: dorian\nI |\n").unwrap();
        let Expr::Seq(items) = &ast else { panic!("not a seq") };
        let Expr::Keyed(_, inner) = &items[0] else { panic!("not keyed") };
        let Expr::Seq(inner_items) = inner.as_ref() else { panic!("中身は Seq") };
        assert!(matches!(&inner_items[0], Expr::Scaled(_, _)));
    }
```

- [ ] **Step 2: 落ちることを確認**

```bash
cargo test -p chord_midi test_key_directive_wraps_the_rest
```

Expected: FAIL (コンパイルエラー。`Expr::Keyed` が無い)

- [ ] **Step 3: 型を変える**

`chord_midi/src/model/ast.rs` の `Expr` から `Directive(Directive)` を消し、代わりに:

```rust
    /// `key:` 以降を包む
    Keyed(Pitch, Box<Expr>),
    /// `scale:` 以降を包む
    Scaled(Scale, Box<Expr>),
```

`Directive` enum は消す。`Pitch` / `Scale` は既に import 済み。

- [ ] **Step 4: パーサを再帰に変える**

`chord_midi/src/import/rechord.rs`。`directive_parser` は「ディレクティブの頭だけを食う」形に変え、`ast_parser` の `many1` を再帰する `seq_parser` に置き換える:

```rust
/// `key:` / `scale:` の1行。値だけを返す。以降を包むのは seq_parser の仕事
enum DirectiveHead {
    Key(Pitch),
    Scale(Scale),
}

#[tracable_parser]
fn directive_head(s: Span) -> IResult<Span, DirectiveHead> {
    // a bad value (e.g. `key: H`) fails the parse gracefully (map_res) rather
    // than panicking — important for the realtime web editor while typing.
    // The terminator accepts eof so a trailing directive without a newline parses.
    alt((
        map_res(
            tuple((tag("key:"), space0, not_line_ending, alt((line_ending, eof)))),
            |(_, _, val, _): (Span, Span, Span, Span)| {
                Pitch::from_str(val.fragment().trim()).map(DirectiveHead::Key)
            },
        ),
        map_res(
            tuple((tag("scale:"), space0, not_line_ending, alt((line_ending, eof)))),
            |(_, _, val, _): (Span, Span, Span, Span)| {
                Scale::from_str(val.fragment().trim()).map(DirectiveHead::Scale)
            },
        ),
    ))(s)
}

/// 式の列。`key:` / `scale:` に当たったら、そこから後ろを再帰で読んで包む
fn seq_parser(s: Span) -> IResult<Span, Expr> {
    let mut items: Vec<Expr> = Vec::new();
    let mut rest = s;
    loop {
        if let Ok((r, head)) = directive_head(rest) {
            let (r2, inner) = seq_parser(r)?;
            items.push(match head {
                DirectiveHead::Key(p) => Expr::Keyed(p, Box::new(inner)),
                DirectiveHead::Scale(sc) => Expr::Scaled(sc, Box::new(inner)),
            });
            rest = r2;
            // 以降は inner が飲み切っている
            break;
        }
        match alt((comment_parser, apply_parser, measure_parser))(rest) {
            Ok((r, item)) => {
                items.push(item);
                rest = r;
            }
            Err(_) => break,
        }
    }
    Ok((rest, Expr::Seq(items)))
}

#[tracable_parser]
fn ast_parser(s: Span) -> IResult<Span, Expr> {
    map(tuple((seq_parser, eof)), |(seq, _)| seq)(s)
}
```

`directive_parser` は消す。旧 `ast_parser` の `many1` も消す。

**注意**: 旧 `ast_parser` は `many1` だったので空入力を拒んでいた。`seq_parser` は空の `Seq` を返す。空入力を拒む必要があるなら `ast_parser` で弾く:

```rust
#[tracable_parser]
fn ast_parser(s: Span) -> IResult<Span, Expr> {
    map_res(tuple((seq_parser, eof)), |(seq, _)| {
        match &seq {
            Expr::Seq(items) if items.is_empty() => {
                Err(anyhow::anyhow!("empty input"))
            }
            _ => Ok(seq),
        }
    })(s)
}
```

- [ ] **Step 5: 落ちている箇所をコンパイラに拾わせる**

```bash
cargo check --workspace 2>&1 | head -60
```

`Expr::Directive` を参照している所 (`transform.rs` の `resolve`、`export/midi.rs`、`fingering/mod.rs`、`edit.rs`、`export/rechord.rs`) が全部落ちる。Task 6 / 7 / 8 で埋めるので、この Step では**最小限のコンパイルを通す**だけにする:

- `export/midi.rs` `interpret`: `Expr::Directive(_) => Ok(())` を `Expr::Keyed(_, inner) | Expr::Scaled(_, inner) => self.interpret(*inner)` に
- `fingering/mod.rs` `collect_ast`: `Expr::Directive(_)` を落とし、`Expr::Keyed(_, inner) | Expr::Scaled(_, inner) => collect_ast(inner, pattern, out)?` を足す
- `export/rechord.rs`: Task 7 で書くので、いったん `Expr::Keyed(p, inner) => { writeln!(f, "key: {}", p)?; write!(f, "{}", inner) }` / `Expr::Scaled(s, inner) => { writeln!(f, "scale: {}", s)?; write!(f, "{}", inner) }`
- `transform.rs` `resolve`: プリスキャンのループを消す (Task 6 で置き換える)。いったん `let transposed = self.into_pitch(Pitch::C, Scale::Major);` にする
- `edit.rs` `shift_ast`: `Expr::Directive(Directive::Key(p))` の腕を `Expr::Keyed(p, inner) => { seen.key_directive = true; *p = shift_pitch(*p, delta); shift_ast(inner, delta, seen); }` に、`Expr::Scaled(_, inner) => shift_ast(inner, delta, seen)` を足す

- [ ] **Step 6: 新しいテストが通ることを確認**

```bash
cargo test -p chord_midi test_key_directive_wraps_the_rest test_comment_before_key_stays_outside test_scale_nests_inside_key
```

Expected: PASS

> **注意: Task 5 と Task 6 は一緒に終わらせる。** Task 5 の終わりは
> `into_pitch` がまだスコープを見ていない壊れた状態で、commit できない。
> 中断するなら Task 6 の commit まで進めるか、Task 5 の頭に戻す。

- [ ] **Step 7: 他のテストの状態を記録する**

```bash
cargo test --workspace 2>&1 | tail -40
```

この時点では `into_pitch` がスコープを見ていないので、度数表記のテストとゴールデンが落ちる。**落ちたテスト名を控えて Task 6 に進む。** ここでは commit しない

---

## Task 6: `into_pitch` / `into_degree` / `resolve` をスコープ再帰にする

**Files:**
- Modify: `chord_midi/src/model/transform.rs`
- Test: `chord_midi/src/model/transform.rs` (`mod tests`)

- [ ] **Step 1: 失敗するテストを書く**

`chord_midi/src/model/transform.rs` の `mod tests` に足す:

```rust
    #[test]
    fn test_key_scope_applies_to_following_measures() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::{Expr, Node};
        use crate::model::key::Key;
        use crate::model::pitch::Pitch;

        fn first_pitches(expr: &Expr, out: &mut Vec<Pitch>) {
            match expr {
                Expr::Seq(items) => items.iter().for_each(|i| first_pitches(i, out)),
                Expr::Apply(_, inner, _) => first_pitches(inner, out),
                Expr::Keyed(_, inner) | Expr::Scaled(_, inner) => first_pitches(inner, out),
                Expr::Measure(nodes, _) => {
                    for n in nodes {
                        if let Node::Chord(c) = n {
                            if let Key::Absolute(p) = c.key {
                                out.push(p);
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        // 途中の key: がそこから後ろにだけ効く
        let ast = RechordImporter
            .import("key: C\nI |\nkey: D\nI |\n")
            .unwrap()
            .resolve();
        let mut got = Vec::new();
        first_pitches(&ast, &mut got);
        assert_eq!(got, vec![Pitch::C, Pitch::D]);
    }

    #[test]
    fn test_resolve_removes_keyed_wrappers() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::Expr;

        fn has_wrapper(expr: &Expr) -> bool {
            match expr {
                Expr::Keyed(_, _) | Expr::Scaled(_, _) => true,
                Expr::Seq(items) => items.iter().any(has_wrapper),
                Expr::Apply(_, inner, _) => has_wrapper(inner),
                _ => false,
            }
        }

        let ast = RechordImporter.import("key: C\nI |\n").unwrap().resolve();
        assert!(!has_wrapper(&ast), "resolve 後に Keyed/Scaled が残っている");
    }
```

- [ ] **Step 2: 落ちることを確認**

```bash
cargo test -p chord_midi test_key_scope_applies_to_following_measures
```

Expected: FAIL (`got` が `[C, C]`。`into_pitch` が `Keyed` を見ていない)

- [ ] **Step 3: `into_pitch` にスコープの腕を足す**

`chord_midi/src/model/transform.rs` の `into_pitch` の `match` に:

```rust
            // key: は以降のスコープ。中身を新しい tonic で解決し、包みは消す
            Expr::Keyed(p, inner) => inner.into_pitch(p, scale),
            Expr::Scaled(s, inner) => inner.into_pitch(tonic, s),
```

`into_degree` は逆変換なので包みを残す:

```rust
            Expr::Keyed(p, inner) => Expr::Keyed(p, Box::new(inner.into_degree(p, scale))),
            Expr::Scaled(s, inner) => Expr::Scaled(s, Box::new(inner.into_degree(tonic, s))),
```

- [ ] **Step 4: `resolve` のプリスキャンを消す**

```rust
    /// key/scale のスコープを解いて度数を実音にし、Comment を落とした
    /// 再生可能な AST を返す。Transform は MIDI 解釈時に展開する。
    ///
    /// `key:` が無いときの tonic は C。度数表記の進行 (プリセット等) が
    /// 常に実音に解決されて鳴るようにするため。
    pub fn resolve(self) -> Expr {
        let transposed = self.into_pitch(Pitch::C, Scale::Major);
        // pow は音程の話なのでここで潰す。pat はリズムの話なので残す
        let desugared = desugar_shapes(transposed)
            .into_iter()
            .next()
            .expect("desugar_shapes always yields at least one node for a Seq");
        strip_comments(desugared)
    }
```

`strip_comments` を足す。`Keyed` は `into_pitch` が既に消しているので相手にしない:

```rust
/// Comment を落とす。`Keyed`/`Scaled` は into_pitch が解いて消えている
fn strip_comments(expr: Expr) -> Expr {
    match expr {
        Expr::Seq(items) => Expr::Seq(
            items
                .into_iter()
                .filter(|it| !matches!(it, Expr::Comment(_)))
                .map(strip_comments)
                .collect(),
        ),
        Expr::Apply(func, inner, br) => Expr::Apply(func, Box::new(strip_comments(*inner)), br),
        other => other,
    }
}
```

- [ ] **Step 5: 新しいテストが通ることを確認**

```bash
cargo test -p chord_midi test_key_scope_applies_to_following_measures test_resolve_removes_keyed_wrappers
```

Expected: PASS

- [ ] **Step 6: 全テストとゴールデンを確認**

```bash
cargo test --workspace
```

Expected: PASS。**ゴールデンは全部通らなければならない。** サンプルもプリセットも `key:` は先頭1つだけなので、スコープ化で出力は変わらない。落ちたら `into_pitch` の再帰か `strip_comments` が壊れている

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: key: / scale: を以降に効くスコープにする"
```

---

## Task 7: Display で `Keyed` / `Scaled` を平らに戻す

**Files:**
- Modify: `chord_midi/src/export/rechord.rs`
- Test: `chord_midi/src/export/rechord.rs` (`mod tests`)

- [ ] **Step 1: 失敗するテストを書く**

`chord_midi/src/export/rechord.rs` の `mod tests` に足す:

```rust
    #[test]
    fn test_key_directive_roundtrip() {
        assert_eq!(roundtrip("key: C\nC | Am |"), "key: C\nC | Am |");
    }

    #[test]
    fn test_scale_directive_roundtrip() {
        assert_eq!(
            roundtrip("key: C\nscale: dorian\nI | IV |"),
            "key: C\nscale: dorian\nI | IV |"
        );
    }

    #[test]
    fn test_comment_and_key_roundtrip() {
        assert_eq!(roundtrip("# memo\nkey: C\nC |"), "# memo\nkey: C\nC |");
    }

    #[test]
    fn test_mid_song_key_change_roundtrip() {
        assert_eq!(
            roundtrip("key: C\nI |\nkey: D\nI |"),
            "key: C\nI |\nkey: D\nI |"
        );
    }
```

既存の `roundtrip` ヘルパは行末の空白を落としていないので、`mod tests` の `roundtrip` を直す:

```rust
    fn roundtrip(code: &str) -> String {
        let ast = RechordImporter.import(&format!("{}\n", code)).unwrap();
        format!("{}", ast)
            .lines()
            .map(|l| l.trim_end())
            .collect::<Vec<_>>()
            .join("\n")
            .trim_end()
            .to_string()
    }
```

- [ ] **Step 2: 落ちることを確認**

```bash
cargo test -p chord_midi test_key_directive_roundtrip test_mid_song_key_change_roundtrip
```

Expected: FAIL

- [ ] **Step 3: Display を書く**

`chord_midi/src/export/rechord.rs` の `impl Display for Expr` に:

```rust
            // 入れ子は出力しない。`key: X` 行 + 中身というフラットな形に戻す
            Expr::Keyed(p, inner) => {
                writeln!(f, "key: {}", p)?;
                write!(f, "{}", inner)
            }
            Expr::Scaled(s, inner) => {
                writeln!(f, "scale: {}", s)?;
                write!(f, "{}", inner)
            }
            Expr::Seq(items) => {
                for item in items {
                    write!(f, "{}", item)?;
                }
                Ok(())
            }
```

旧 `Expr::Directive(...)` の腕は消す。

- [ ] **Step 4: 通ることを確認**

```bash
cargo test -p chord_midi --lib export
```

Expected: PASS

- [ ] **Step 5: 全テストを確認**

```bash
cargo test --workspace
```

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "fix: Keyed / Scaled を key: 行へ平らに戻して round-trip させる"
```

---

## Task 8: 移調が `Keyed` を貫通する

**Files:**
- Modify: `chord_midi/src/edit.rs`
- Test: `chord_midi/src/edit.rs` (`mod tests`)

- [ ] **Step 1: 失敗するテストを書く**

`chord_midi/src/edit.rs` の `mod tests` に足す:

```rust
    #[test]
    fn test_shift_key_transposes_every_key_directive() {
        // 途中転調も両方ずれる
        assert_eq!(
            shift_key("key: C\nI |\nkey: D\nI |", 1),
            "key: C#\nI |\nkey: D#\nI |"
        );
    }

    #[test]
    fn test_shift_key_scale_directive_survives() {
        assert_eq!(
            shift_key("key: C\nscale: dorian\nI |", 1),
            "key: C#\nscale: dorian\nI |"
        );
    }
```

- [ ] **Step 2: 落ちることを確認**

```bash
cargo test -p chord_midi test_shift_key_transposes_every_key_directive
```

Expected: FAIL

- [ ] **Step 3: `shift_ast` の腕を書く**

`chord_midi/src/edit.rs`:

```rust
fn shift_ast(expr: &mut Expr, delta: i8, seen: &mut Seen) {
    match expr {
        Expr::Seq(items) => {
            for item in items {
                shift_ast(item, delta, seen);
            }
        }
        Expr::Keyed(p, inner) => {
            seen.key_directive = true;
            *p = shift_pitch(*p, delta);
            shift_ast(inner, delta, seen);
        }
        Expr::Scaled(_, inner) => shift_ast(inner, delta, seen),
        Expr::Measure(nodes, _) => {
            for node in nodes {
                shift_node(node, delta, seen);
            }
        }
        Expr::Apply(_, inner, _) => shift_ast(inner, delta, seen),
        Expr::Comment(_) => {}
    }
}
```

`use crate::model::ast::Directive;` は消す。

- [ ] **Step 4: 通ることを確認**

```bash
cargo test -p chord_midi --lib edit
```

Expected: PASS。既存の `test_shift_key_keeps_comments_and_directives` も通ること

- [ ] **Step 5: 全テストを確認**

```bash
cargo test --workspace
```

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "fix: 移調が key: スコープを貫通するようにする"
```

---

## Task 9: S式インポータを `Keyed` に合わせる

**Files:**
- Modify: `chord_midi/src/import/sexp.rs`
- Test: `chord_midi/src/import/sexp.rs` (`mod tests`)

- [ ] **Step 1: 失敗するテストを書く**

`chord_midi/src/import/sexp.rs` の `mod tests` に足す:

```rust
    #[test]
    fn test_keyed_becomes_keyed_expr() -> Result<()> {
        use crate::model::ast::Expr;
        use crate::model::pitch::Pitch;
        let ast = SexpImporter.import("(score (keyed C (I IV)))")?;
        let Expr::Seq(items) = &ast else { panic!("not a seq") };
        assert!(
            matches!(&items[0], Expr::Keyed(Pitch::C, _)),
            "keyed は Expr::Keyed になる: {:?}",
            items[0]
        );
        Ok(())
    }
```

- [ ] **Step 2: 落ちることを確認**

```bash
cargo test -p chord_midi test_keyed_becomes_keyed_expr
```

Expected: FAIL (今の `keyed` は `into_pitch` をその場で呼んで包みを残さない)

- [ ] **Step 3: `keyed` の腕を書き換える**

`chord_midi/src/import/sexp.rs` の `parse_ast`:

```rust
        // keyed(pitch: Pitch, measure: Measure) -> Measure
        Sexp::List(list) if starts_with(sexp, "keyed") => {
            let key = parse_key(list[1].string()?)?;
            let Key::Absolute(pitch) = key else {
                return Err(anyhow::anyhow!("pitch must be absolute: {:?}", key));
            };
            // 解決は resolve() の仕事。ここでは包むだけ
            let inner = parse_ast(&list[2])?;
            Ok(Expr::Keyed(pitch, Box::new(inner)))
        }
```

`use crate::model::scale::Scale;` が未使用になったら消す。

- [ ] **Step 4: 通ることを確認**

```bash
cargo test -p chord_midi --lib import::sexp
```

Expected: PASS。既存の `test_parse_score` も通ること

- [ ] **Step 5: 全テストを確認**

```bash
cargo test --workspace
```

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor: S式の keyed を Expr::Keyed に直接対応させる"
```

---

## Task 10: CLI の `--print-ast` を JSON にする

**Files:**
- Modify: `chord_midi_cli/src/main.rs:43-45`, `chord_midi_cli/src/main.rs:155-175`

- [ ] **Step 1: 引数を値省略可にする**

`chord_midi_cli/src/main.rs` の `print_ast` フィールドを差し替える:

```rust
    /// AST を表示する。syntax = 構文木 (既定), expanded = 解決後, text = rechord 記法
    #[arg(
        long,
        value_name = "FORM",
        num_args = 0..=1,
        default_missing_value = "syntax"
    )]
    print_ast: Option<AstForm>,
```

同ファイルに:

```rust
#[derive(Debug, Clone, Copy, PartialEq, clap::ValueEnum)]
enum AstForm {
    /// パース直後の構文木を JSON で
    Syntax,
    /// key/scale を解決し pow/oct を潰した後の木を JSON で
    Expanded,
    /// rechord 記法に書き戻したテキスト
    Text,
}
```

- [ ] **Step 2: 出力を書く**

`chord_midi_cli/src/main.rs` の `if args.print_ast {` のブロックを差し替える:

```rust
    if let Some(form) = args.print_ast {
        let ast = parse()?;
        match form {
            AstForm::Syntax => println!("{}", serde_json::to_string_pretty(&ast)?),
            AstForm::Expanded => {
                println!("{}", serde_json::to_string_pretty(&ast.resolve())?)
            }
            AstForm::Text => print!("{}", ast),
        }
    }
```

`None if !args.print_ast && !args.fingering` の条件を直す:

```rust
        None if args.print_ast.is_none() && !args.fingering => {
```

`chord_midi_cli/Cargo.toml` の `[dependencies]` に `serde_json = "1"` を足す。

- [ ] **Step 3: ビルドを確認**

```bash
cargo build -p chord_midi_cli
```

Expected: 成功

- [ ] **Step 4: 手で3形を確認**

```bash
printf 'key: C\nC | Am |\n' > /tmp/t.txt
cargo run -q -p chord_midi_cli -- -i /tmp/t.txt --print-ast | head -20
cargo run -q -p chord_midi_cli -- -i /tmp/t.txt --print-ast expanded | head -20
cargo run -q -p chord_midi_cli -- -i /tmp/t.txt --print-ast text
```

Expected:
- 1つ目: `"Keyed"` を含む pretty JSON
- 2つ目: `"Keyed"` を**含まない** pretty JSON (resolve で解けている)
- 3つ目: `key: C` / `C | Am |`

- [ ] **Step 5: 既存の使い方が壊れていないことを確認**

```bash
cargo run -q -p chord_midi_cli -- -i /tmp/t.txt --transpose 1
cargo run -q -p chord_midi_cli -- -i /tmp/t.txt --fingering | head -20
```

Expected: 1つ目は `key: C#` / `C# | A#m |`、2つ目は押さえ方の図

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat(cli): --print-ast を JSON にして表示形を選べるようにする"
```

---

## Task 11: web の「解釈結果」に表示3択を出す

**Files:**
- Modify: `chord_midi_web/src/main.rs:28-42` (`compile`)
- Modify: `chord_midi_web/src/main.rs:54-70` (state)
- Modify: `chord_midi_web/src/main.rs:388-391` (`details.ast`)
- Modify: `chord_midi_web/Cargo.toml`

- [ ] **Step 1: 依存を足す**

`chord_midi_web/Cargo.toml` の `[dependencies]` に:

```toml
serde_json = "1"
```

- [ ] **Step 2: `compile` が3形を返すようにする**

`chord_midi_web/src/main.rs`:

```rust
/// 「解釈結果」に出す3つの形
#[derive(Clone, Copy, PartialEq)]
enum AstForm {
    Syntax,
    Expanded,
    Text,
}

/// 解釈結果の3形。どれも文字列なので、表示側は選ぶだけ
#[derive(Clone, Default, PartialEq)]
struct AstViews {
    syntax: String,
    expanded: String,
    text: String,
}

fn compile(input: &str, bpm: u8) -> Result<(AstViews, Vec<MeasureRef>, Vec<u8>), String> {
    let ast = RechordImporter
        .import(&format!("{}\n", input))
        .map_err(|e| e.to_string())?;
    // 度数表記のまま見せたいので resolve する前に文字列化する
    let views_text = format!("{}", ast);
    let syntax = serde_json::to_string_pretty(&ast).map_err(|e| e.to_string())?;
    // resolve() も export() も self を consume するので、解決済みの AST を
    // 使い回す。MidiExporter は内部でもう一度 resolve() を呼ぶが冪等。
    let resolved = ast.resolve();
    let expanded = serde_json::to_string_pretty(&resolved).map_err(|e| e.to_string())?;
    let measures = fingering::measure_refs(&resolved).map_err(|e| e.to_string())?;
    let mut writer = BufWriter::new(Vec::new());
    MidiExporter { bpm }
        .export(&mut writer, resolved)
        .map_err(|e| e.to_string())?;
    let bytes = writer.into_inner().map_err(|e| e.to_string())?;
    Ok((
        AstViews { syntax, expanded, text: views_text },
        measures,
        bytes,
    ))
}
```

- [ ] **Step 3: state を差し替える**

`let result = use_state(String::new);` を:

```rust
    let result = use_state(AstViews::default);
    let ast_form = use_state(|| AstForm::Syntax);
```

`use_effect_with` の中の `result.set(String::new());` を `result.set(AstViews::default());` に、`result.set(ast_str);` を `result.set(views);` に、`Ok((ast_str, ms, bytes))` の分解を `Ok((views, ms, bytes))` にする。

- [ ] **Step 4: 表示を書く**

`details.ast` のブロックを差し替える:

```rust
            <details class="ast">
                <summary>{"解釈結果"}</summary>
                <div class="ast-forms">
                    { for [
                        (AstForm::Syntax, "構文"),
                        (AstForm::Expanded, "展開後"),
                        (AstForm::Text, "テキスト"),
                    ].into_iter().map(|(form, label)| {
                        let ast_form = ast_form.clone();
                        let onclick = Callback::from(move |_| ast_form.set(form));
                        html! {
                            <button
                                class={classes!("btn", "btn-icon",
                                    (*ast_form == form).then_some("is-active"))}
                                {onclick}
                            >{label}</button>
                        }
                    }) }
                </div>
                <pre>{
                    match *ast_form {
                        AstForm::Syntax => result.syntax.clone(),
                        AstForm::Expanded => result.expanded.clone(),
                        AstForm::Text => result.text.clone(),
                    }
                }</pre>
            </details>
```

- [ ] **Step 5: CSS を足す**

`chord_midi_web/style.css` の末尾に足す。`.btn` / `.is-active` は既にあるので、
並びの間隔だけ決める:

```css
.ast-forms {
  display: flex;
  gap: 0.25rem;
  margin-bottom: 0.5rem;
}

.ast-forms .btn {
  width: auto;
  padding: 0.2rem 0.6rem;
}
```

`.is-active` の見た目が `.tab` 側にしか付いていないなら、`.ast-forms .btn.is-active`
にも同じ背景色を当てる。

- [ ] **Step 6: ビルドを確認**

```bash
(cd chord_midi_web && cargo check --target wasm32-unknown-unknown)
```

Expected: 成功

- [ ] **Step 7: 手で動かして確認**

```bash
cd chord_midi_web && trunk serve
```

`trunk` が無ければ `cargo install trunk`。CI (`.github/workflows/gh-pages.yml`) と同じ 0.21 系を使う。
ブラウザで開いて:

1. `key: C` / `C | Am | F | G |` を打つ
2. 「解釈結果」を開く
3. 「構文」に `Keyed` を含む JSON が出る
4. 「展開後」に `Keyed` を含まない JSON が出る
5. 「テキスト」に `key: C` / `C | Am | F | G |` が出る
6. 再生ボタンで今までどおり鳴る

- [ ] **Step 8: Commit**

`chord_midi_web` は独自の `Cargo.lock` を持つので一緒に入れる。

```bash
git add -A
git commit -m "feat(web): 解釈結果に構文 / 展開後 / テキストの3表示を足す"
```

---

## Task 12: ドキュメント更新と締め

**Files:**
- Modify: `README.md`

- [ ] **Step 1: README に `key:` のスコープを書く**

`README.md` の `## 関数` の前に節を足す:

```markdown
## キーとスケール

    key: C
    scale: dorian

`key:` と `scale:` は**それ以降の行に効く**。曲の途中で書き直すと、そこから
後ろだけがそのキーになる。

    key: C
    I | IV |
    key: D
    I | IV |     # ここは D のキーで解釈される
```

- [ ] **Step 2: README の CLI 節に `--print-ast` を書く**

`## CLI` の「表示」のブロックを差し替える:

```markdown
    # 表示
    cargo run -p chord_midi_cli -- -i song.txt --print-ast            # 構文木 (JSON)
    cargo run -p chord_midi_cli -- -i song.txt --print-ast expanded   # 解決後 (JSON)
    cargo run -p chord_midi_cli -- -i song.txt --print-ast text       # rechord 記法に書き戻し
    cargo run -p chord_midi_cli -- -i song.txt --fingering
```

- [ ] **Step 3: 全部を通し直す**

```bash
cargo test --workspace && (cd chord_midi_web && cargo check --target wasm32-unknown-unknown)
```

Expected: どちらも成功。ゴールデンが全部通ること

- [ ] **Step 4: ゴールデンに差分が無いことを目視で確認**

```bash
git status --short chord_midi/tests/golden
```

Expected: 空 (Task 1 以降 `.mid` が1つも書き換わっていない)

差分が出ていたら後方互換を壊している。`git diff` では中身が読めないので、`--print-ast expanded` で当該サンプルの木を見て原因を探る。

- [ ] **Step 5: Commit**

```bash
git add README.md
git commit -m "docs: key: のスコープと --print-ast の使い方を書く"
```

---

## 完了条件

- `cargo test --workspace` が通る
- `chord_midi/tests/golden/*.mid` が Task 1 の生成時から1バイトも変わっていない
- `(cd chord_midi_web && cargo check --target wasm32-unknown-unknown)` が通る
- web で入力 → 再生 → MIDI 保存 → 移調 → スケール変更 → 押さえ方表示が今までどおり動く
- `Expr` に `Directive` が無く、`Keyed` / `Scaled` がある
- `resolve()` の後に `Keyed` / `Scaled` / `Comment` が残らない

## Phase 2 / Phase 3

この計画は Phase 1 だけを対象にする。Phase 1 がマージされてから、それぞれ別の計画を書く。

- **Phase 2 (定義・参照・リピート)**: `$name =` / `$name` / `xN` / `( )`、`model/expand.rs`、エラー4種、`Func::Pat` を `Box<Expr>` に、`MeasureRef.section`
- **Phase 3 (GUI)**: `reference.rs`、記法パネル、`RhythmGrid`、`FuncPalette`、`DefineChips`、`insert_at_cursor`、CLI の `--help-syntax`
