# pow() と pat() Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** rechord 記法に、小節列を引数に取る関数 `pow()`（パワーコード）と `pat()`（リズムパターン）を足す。

**Architecture:** AST に `Ast::Apply(Func, Vec<Box<Ast>>)` を足す。関数を AST に残すので `Display` がそのまま `pow(C | E D)` を再出力でき、キー上下でテキストを書き戻しても関数が展開形に化けない。`pow` は音程の話なので `resolve()` で脱糖して `Modifier::Power` に変える（MIDI と押さえ方プレビューの両方に効く）。`pat` はリズムの話なので `resolve()` では展開せず、MIDI の `Score::interpret` だけが処理する。

**Tech Stack:** Rust 2021 / nom 7 (パーサ) / midi_file 0.0.3

**Spec:** `docs/superpowers/specs/2026-09-15-pow-and-pat-functions-design.md`

**Issue:** https://github.com/wakame-tech/chord-midi/issues/20

---

## File Structure

| ファイル | 変更 |
| --- | --- |
| `chord_midi/src/model/ast.rs` | `Ast::Apply` / `Func` / `Pattern` / `Beat` / `Node::Brush` を追加。`Node` に `Clone` を derive |
| `chord_midi/src/import/rechord.rs` | `apply_parser` / `pattern_parser` を追加。`measure_sep` が `)` の手前でも止まるようにする |
| `chord_midi/src/export/rechord.rs` | `Ast::Apply` と `Pattern` の `Display`、`Node::Brush` の `Display` |
| `chord_midi/src/model/modifier.rs` | `Modifier::Power` を enum の末尾に追加、`Display` |
| `chord_midi/src/model/chord.rs` | `Chord::modify` に `Power` の腕 |
| `chord_midi/src/model/transform.rs` | `into_pitch` / `into_degree` に `Apply` と `Brush` の腕。`resolve` で `pow` を脱糖 |
| `chord_midi/src/export/midi.rs` | `Note` に velocity。`interpret` に `Apply` の腕。`pat` の展開 |
| `chord_midi/src/fingering/mod.rs` | `collect_ast` に `Apply` の腕（素通り）、`collect_node` に `Brush` |
| `chord_midi/src/edit.rs` | `shift_ast` に `Apply` の腕、`shift_node` に `Brush` |

---

## 前提知識（このリポジトリを知らない人向け）

- **ワークスペース**: ルートの `Cargo.toml` のメンバーは `chord_midi` と `chord_midi_cli`。`chord_midi_web` は `exclude` されており、`cd chord_midi_web` して別ビルドする。
- **`chord_midi_cli` は現在ビルドできない**（依存の `simplelog` → `time 0.3.34` が現行ツールチェインで `error[E0282]`）。このブランチとは無関係な既知の壊れ。テストは必ず `cargo test -p chord_midi` で走らせること。`cargo test`（ワークスペース全体）は使わない。
- **パーサ**: nom 7 + `nom_locate` + `nom_tracable`。`Span` は `import/parser_util.rs` の型エイリアス。パーサ関数には `#[tracable_parser]` を付ける。
- **AST の形**: `Ast::Score(Vec<Box<Ast>>)` → `Ast::Measure(Vec<Node>, bool)` → `Node`。`Ast::Score` の子は `Directive` / `Comment` / `Measure`。`Measure` の第2要素 `bool` は「この小節の後ろで改行される」。
- **小節の分割**: `MEASURE_LENGTH = 16` tick。`Score::measure_unit_size(n)` が「ノード数 n のときの1ノードあたりの尺」を返す。n を 1/2/4/8/16 に切り上げてから `16 / len` を返すので、3ノードなら `4` が返る。
- **`Chord::modify`**: `ChordNode.modifiers` は `BTreeSet<Modifier>` で、derive した `Ord` は **enum の宣言順**。順に `modify` が呼ばれるので、後ろに宣言した修飾ほど後から適用され、前のものを上書きできる。
- **`Velocity`**: `midi_file::core::Velocity`。`Velocity::new(u8)` は値をクランプする `const fn`。`Velocity::default()` は 72。

---

## Task 1: AST の型とパーサ

**Files:**
- Modify: `chord_midi/src/model/ast.rs`
- Modify: `chord_midi/src/import/rechord.rs`

- [ ] **Step 1: 失敗するテストを書く**

`chord_midi/src/import/rechord.rs` の `mod tests` の末尾に足す。

```rust
    #[test]
    fn test_pow_parser() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::{Ast, Func};
        let ast = RechordImporter.import("pow(C | E D)\n").unwrap();
        let Ast::Score(items) = &ast else { panic!("not a score") };
        assert_eq!(items.len(), 1);
        let Ast::Apply(Func::Pow, inner) = items[0].as_ref() else {
            panic!("not a pow apply: {:?}", items[0])
        };
        // C の小節と E D の小節
        assert_eq!(inner.len(), 2);
        let Ast::Measure(nodes, _) = inner[0].as_ref() else { panic!("not a measure") };
        assert_eq!(nodes.len(), 1);
        let Ast::Measure(nodes, _) = inner[1].as_ref() else { panic!("not a measure") };
        assert_eq!(nodes.len(), 2);
    }

    #[test]
    fn test_pat_parser() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::{Ast, Beat, Func};
        let ast = RechordImporter.import("pat(\"o.ox\", C | E D)\n").unwrap();
        let Ast::Score(items) = &ast else { panic!("not a score") };
        let Ast::Apply(Func::Pat(pattern), inner) = items[0].as_ref() else {
            panic!("not a pat apply: {:?}", items[0])
        };
        assert_eq!(
            pattern.0,
            vec![Beat::Hit, Beat::Rest, Beat::Hit, Beat::Brush]
        );
        assert_eq!(inner.len(), 2);
    }

    #[test]
    fn test_pat_nested_in_pow() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::{Ast, Func};
        let ast = RechordImporter.import("pat(\"oxox\", pow(C))\n").unwrap();
        let Ast::Score(items) = &ast else { panic!("not a score") };
        let Ast::Apply(Func::Pat(_), inner) = items[0].as_ref() else {
            panic!("not a pat apply")
        };
        assert!(matches!(inner[0].as_ref(), Ast::Apply(Func::Pow, _)));
    }

    #[test]
    fn test_apply_followed_by_measure() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::Ast;
        let ast = RechordImporter.import("pow(C) F |\n").unwrap();
        let Ast::Score(items) = &ast else { panic!("not a score") };
        assert_eq!(items.len(), 2);
        assert!(matches!(items[0].as_ref(), Ast::Apply(_, _)));
        assert!(matches!(items[1].as_ref(), Ast::Measure(_, _)));
    }

    #[test]
    fn test_pattern_length_must_be_power_of_two() {
        use crate::import::{Importer, RechordImporter};
        // 3 文字は 16 を割り切れないので受け付けない
        assert!(RechordImporter.import("pat(\"o.o\", C)\n").is_err());
        assert!(RechordImporter.import("pat(\"o.o.o\", C)\n").is_err());
    }

    #[test]
    fn test_pattern_rejects_unknown_char() {
        use crate::import::{Importer, RechordImporter};
        assert!(RechordImporter.import("pat(\"o.oz\", C)\n").is_err());
    }
```

- [ ] **Step 2: テストが失敗することを確認する**

Run: `cargo test -p chord_midi rechord::tests`
Expected: コンパイルエラー。`no variant or associated item named `Apply` found for enum `Ast``

- [ ] **Step 3: AST の型を足す**

`chord_midi/src/model/ast.rs` の `pub enum Ast` を差し替える。

```rust
#[derive(Debug, PartialEq)]
pub enum Ast {
    Comment(String),
    Directive(Directive),
    // nodes, br?
    Measure(Vec<Node>, bool),
    /// 小節列に関数を適用する。中身は Measure と Apply（ネスト）だけ
    Apply(Func, Vec<Box<Ast>>),
    Score(Vec<Box<Ast>>),
}

/// 小節列に掛ける関数
#[derive(Debug, Clone, PartialEq)]
pub enum Func {
    /// パワーコード化
    Pow,
    /// リズムパターン
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

同じファイルの `pub enum Node` に `Clone` を derive し、`Brush` を足す。`Brush` は `pat` の展開で内部的に作られるだけで、パーサからは生成されない。

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Chord(ChordNode),
    Arpeggio(ChordNode),
    Transform(Transform, ChordNode),
    /// ブラッシング。`pat` の `x` から作られる
    Brush(ChordNode),
    Rest,
    Sustain,
    Repeat,
}
```

- [ ] **Step 4: パーサを足す**

`chord_midi/src/import/rechord.rs` の `use` に型を足す。

```rust
use crate::model::ast::{Ast, Beat, Directive, Func, Pattern};
```

（既存の `use crate::model::ast::Ast;` と `use crate::model::ast::Directive;` の2行をこの1行に置き換える）

`measure_sep` に `)` の手前で止まる腕を足す。関数の閉じ括弧の直前にある小節が、区切りを見つけられずに失敗するのを防ぐ。

```rust
fn measure_sep(s: Span) -> IResult<Span, bool> {
    // eof acts as an implicit separator so the final measure need not end
    // with a trailing `|` or newline.
    alt((
        // `|` の直後が行末なら、その小節でひと行が終わる
        value(true, tuple((tag("|"), space0, peek(line_ending)))),
        value(false, tag("|")),
        value(true, line_ending),
        // 関数の閉じ括弧の直前も区切りとして扱う。`)` は消費しない
        value(false, peek(tag(")"))),
        value(false, eof),
    ))(s)
}
```

`ast_parser` の `alt` に `apply_parser` を足す。`measure_parser` より前に置く。

```rust
#[tracable_parser]
fn ast_parser(s: Span) -> IResult<Span, Ast> {
    map(
        tuple((
            many1(alt((
                directive_parser,
                comment_parser,
                apply_parser,
                measure_parser,
            ))),
            eof,
        )),
        |(score, _)| Ast::Score(score.into_iter().map(Box::new).collect()),
    )(s)
}
```

`measure_sep` の下に2つのパーサを足す。

```rust
/// `"o.ox"` のようなリズムパターン。長さは 1/2/4/8/16 のみ
#[tracable_parser]
fn pattern_parser(s: Span) -> IResult<Span, Pattern> {
    map_res(
        delimited(
            tag("\""),
            many1(alt((
                value(Beat::Hit, tag("o")),
                value(Beat::Rest, tag(".")),
                value(Beat::Brush, tag("x")),
            ))),
            tag("\""),
        ),
        |beats: Vec<Beat>| {
            // MEASURE_LENGTH = 16 を割り切れる長さだけ許す
            if matches!(beats.len(), 1 | 2 | 4 | 8 | 16) {
                Ok(Pattern(beats))
            } else {
                Err(anyhow::anyhow!(
                    "pattern length must be 1/2/4/8/16, got {}",
                    beats.len()
                ))
            }
        },
    )(s)
}

/// `pow( <小節列> )` と `pat( "<パターン>", <小節列> )`
#[tracable_parser]
fn apply_parser(s: Span) -> IResult<Span, Ast> {
    map(
        tuple((
            alt((
                map(tuple((tag("pow("), space0)), |_| Func::Pow),
                map(
                    tuple((
                        tag("pat("),
                        space0,
                        pattern_parser,
                        space0,
                        tag(","),
                        space0,
                    )),
                    |(_, _, pattern, _, _, _)| Func::Pat(pattern),
                ),
            )),
            many1(alt((apply_parser, measure_parser))),
            space0,
            tag(")"),
            space_or_line_ending_many0,
        )),
        |(func, items, _, _, _)| Ast::Apply(func, items.into_iter().map(Box::new).collect()),
    )(s)
}
```

- [ ] **Step 5: 足りない match の腕を埋めてコンパイルを通す**

`Ast::Apply` と `Node::Brush` を足したことで、既存の網羅的な match が壊れる。次の5箇所を埋める。

`chord_midi/src/export/rechord.rs` の `impl Display for Ast` に足す（中身は Task 2 で正しくする。ここではコンパイルを通すだけ）。

```rust
            Ast::Apply(_, _) => Ok(()),
```

`chord_midi/src/export/rechord.rs` の `impl Display for Node` の `Node::Chord` の腕の下に足す。

```rust
            Node::Brush(chord) => write!(f, "{}", chord),
```

`chord_midi/src/model/transform.rs` の `into_pitch` と `into_degree` それぞれの match に足す。

```rust
            Ast::Apply(func, items) => Ast::Apply(
                func,
                items
                    .into_iter()
                    .map(|ast| Box::new((*ast).into_pitch(tonic, scale)))
                    .collect(),
            ),
```

`into_degree` の方は `into_pitch` を `into_degree` に読み替える。

同じファイルの `Ast::Measure` の腕にあるノードの match（`chordnode_into_pitch` を呼んでいるところ）に `Brush` を足す。

```rust
                        Node::Brush(c) => Node::Brush(chordnode_into_pitch(c, tonic, scale)),
```

`into_degree` 側も同様に足す（そちらは `chordnode_into_degree` を呼んでいる）。

`chord_midi/src/export/midi.rs` の `interpret` に足す（中身は Task 4 で正しくする）。

```rust
            Ast::Apply(_, items) => {
                for item in items {
                    self.interpret(*item)?;
                }
                Ok(())
            }
```

`chord_midi/src/export/midi.rs` の `interpret_node` の match に足す（中身は Task 4 で正しくする）。

```rust
            Node::Brush(node) => {
                let chord = node.to_chord()?;
                self.pre = Some(chord);
                self.sustain = dur;
            }
```

`chord_midi/src/fingering/mod.rs` の `collect_ast` に足す。

```rust
        Ast::Apply(_, items) => {
            for item in items {
                collect_ast(item, out)?;
            }
        }
```

同じファイルの `collect_node` の `Node::Chord(c) | Node::Arpeggio(c)` の腕に `Brush` を足す。

```rust
        Node::Chord(c) | Node::Arpeggio(c) | Node::Brush(c) => out.push(chord_ref(c)?),
```

`chord_midi/src/edit.rs` の `shift_ast` に足す。

```rust
        Ast::Apply(_, items) => {
            for item in items {
                shift_ast(item, delta, seen);
            }
        }
```

同じファイルの `shift_node` の腕に `Brush` を足す。

```rust
        Node::Chord(c) | Node::Arpeggio(c) | Node::Transform(_, c) | Node::Brush(c) => {
            shift_chord(c, delta, seen)
        }
```

- [ ] **Step 6: テストが通ることを確認する**

Run: `cargo test -p chord_midi`
Expected: PASS。既存の67件に加えて今回の6件が通る。

- [ ] **Step 7: コミット**

```bash
git add chord_midi/src/model/ast.rs chord_midi/src/import/rechord.rs chord_midi/src/export/rechord.rs chord_midi/src/model/transform.rs chord_midi/src/export/midi.rs chord_midi/src/fingering/mod.rs chord_midi/src/edit.rs
git commit -m "feat(ast): pow() と pat() のパーサと AST

小節列を引数に取る関数を表す Ast::Apply と、リズムパターンの型を
足す。閉じ括弧の直前でも小節が終われるよう measure_sep に peek を
足した。Display と interpret の中身は後続のコミットで埋める。"
```

---

## Task 2: Display の往復

**Files:**
- Modify: `chord_midi/src/export/rechord.rs`

- [ ] **Step 1: 失敗するテストを書く**

`chord_midi/src/export/rechord.rs` の `mod tests` に足す。

```rust
    fn roundtrip(code: &str) -> String {
        let ast = RechordImporter.import(&format!("{}\n", code)).unwrap();
        format!("{}", ast).trim_end().to_string()
    }

    #[test]
    fn test_pow_roundtrip() {
        assert_eq!(roundtrip("pow(C | E D)"), "pow(C | E D)");
        assert_eq!(roundtrip("pow(C)"), "pow(C)");
    }

    #[test]
    fn test_pat_roundtrip() {
        assert_eq!(roundtrip("pat(\"o.o.oxox\", C | E D)"), "pat(\"o.o.oxox\", C | E D)");
    }

    #[test]
    fn test_nested_apply_roundtrip() {
        assert_eq!(roundtrip("pat(\"oxox\", pow(C))"), "pat(\"oxox\", pow(C))");
    }

    #[test]
    fn test_apply_then_measure_roundtrip() {
        assert_eq!(roundtrip("pow(C) F |"), "pow(C) F |");
    }
```

- [ ] **Step 2: テストが失敗することを確認する**

Run: `cargo test -p chord_midi export::rechord`
Expected: FAIL。`Ast::Apply` の Display が何も出さないので `assertion `left == right` failed` になる（left が空文字か `F |` だけ）

- [ ] **Step 3: Display を実装する**

`chord_midi/src/export/rechord.rs` の `use` に型を足す。

```rust
use crate::model::ast::{Ast, Beat, ChordNode, Directive, Func, Node, Pattern};
```

（既存の `use crate::model::ast::{Ast, ChordNode, Directive, Node};` を置き換える）

Task 1 で置いた仮の腕 `Ast::Apply(_, _) => Ok(()),` を差し替える。

```rust
            Ast::Apply(func, items) => {
                let inner = items
                    .iter()
                    .map(|item| item.to_string())
                    .collect::<Vec<_>>()
                    .join("");
                // 小節の Display は末尾に " | " を出すので、関数の中では
                // 最後の区切りを落として `pow(C | E D)` の形に揃える
                let inner = inner.trim_end();
                let inner = inner.strip_suffix('|').unwrap_or(inner).trim_end();
                match func {
                    Func::Pow => write!(f, "pow({}) ", inner),
                    Func::Pat(pattern) => write!(f, "pat(\"{}\", {}) ", pattern, inner),
                }
            }
```

同じファイルに `Pattern` の `Display` を足す。`impl Display for Node` の下に置く。

```rust
impl Display for Pattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for beat in &self.0 {
            let c = match beat {
                Beat::Hit => 'o',
                Beat::Rest => '.',
                Beat::Brush => 'x',
            };
            write!(f, "{}", c)?;
        }
        Ok(())
    }
}
```

`mod tests` の先頭に import を足す。

```rust
    use crate::import::{Importer, RechordImporter};
```

（既存のテストが同じ import をしている場合はそのまま使う）

- [ ] **Step 4: テストが通ることを確認する**

Run: `cargo test -p chord_midi`
Expected: PASS

- [ ] **Step 5: コミット**

```bash
git add chord_midi/src/export/rechord.rs
git commit -m "feat(export): pow() と pat() を書き戻せるようにする

Ast::Apply と Pattern の Display を実装する。小節の Display が出す
末尾の区切りを落として pow(C | E D) の形に揃える。"
```

---

## Task 3: pow の意味（Modifier::Power と resolve での脱糖）

**Files:**
- Modify: `chord_midi/src/model/modifier.rs`
- Modify: `chord_midi/src/model/chord.rs`
- Modify: `chord_midi/src/model/transform.rs`

- [ ] **Step 1: 失敗するテストを書く**

`chord_midi/src/model/transform.rs` の `mod tests` に足す。

```rust
    #[test]
    fn test_pow_becomes_root_and_fifth() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::Node;

        fn semitones(code: &str) -> Vec<u8> {
            let ast = RechordImporter.import(code).unwrap().resolve();
            let Ast::Score(items) = &ast else { panic!("not a score") };
            let Ast::Measure(nodes, _) = items[0].as_ref() else {
                panic!("pow should be desugared into a measure: {:?}", items[0])
            };
            let Node::Chord(c) = &nodes[0] else { panic!("not a chord") };
            c.to_chord().unwrap().semitones.into_iter().collect()
        }

        // ルートと5thだけになる
        assert_eq!(semitones("pow(C)\n"), vec![0, 7]);
        // 元の修飾を上書きする
        assert_eq!(semitones("pow(Cm7)\n"), vec![0, 7]);
        assert_eq!(semitones("pow(CM7)\n"), vec![0, 7]);
    }

    #[test]
    fn test_pow_keeps_slash_bass() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::Node;
        use crate::model::key::Key;
        use crate::model::pitch::Pitch;

        let ast = RechordImporter.import("pow(C/E)\n").unwrap().resolve();
        let Ast::Score(items) = &ast else { panic!("not a score") };
        let Ast::Measure(nodes, _) = items[0].as_ref() else { panic!("not a measure") };
        let Node::Chord(c) = &nodes[0] else { panic!("not a chord") };
        assert_eq!(c.on, Some(Key::Absolute(Pitch::E)));
    }

    #[test]
    fn test_pow_flattens_all_measures() {
        use crate::import::{Importer, RechordImporter};

        let ast = RechordImporter.import("pow(C | E D)\n").unwrap().resolve();
        let Ast::Score(items) = &ast else { panic!("not a score") };
        // Apply は消えて2つの Measure になる
        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|i| matches!(i.as_ref(), Ast::Measure(_, _))));
    }

    #[test]
    fn test_pat_survives_resolve() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::Func;

        let ast = RechordImporter.import("pat(\"oxox\", C)\n").unwrap().resolve();
        let Ast::Score(items) = &ast else { panic!("not a score") };
        // リズムは MIDI の関心なので resolve では展開しない
        assert!(matches!(items[0].as_ref(), Ast::Apply(Func::Pat(_), _)));
    }

    #[test]
    fn test_pow_inside_pat_is_desugared() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::{Func, Node};

        let ast = RechordImporter
            .import("pat(\"oxox\", pow(C))\n")
            .unwrap()
            .resolve();
        let Ast::Score(items) = &ast else { panic!("not a score") };
        let Ast::Apply(Func::Pat(_), inner) = items[0].as_ref() else { panic!("not a pat") };
        let Ast::Measure(nodes, _) = inner[0].as_ref() else {
            panic!("pow should be desugared inside pat: {:?}", inner[0])
        };
        let Node::Chord(c) = &nodes[0] else { panic!("not a chord") };
        assert_eq!(
            c.to_chord().unwrap().semitones.into_iter().collect::<Vec<_>>(),
            vec![0, 7]
        );
    }
```

- [ ] **Step 2: テストが失敗することを確認する**

Run: `cargo test -p chord_midi transform::tests`
Expected: FAIL。`pow should be desugared into a measure` で panic する（`resolve` がまだ `Apply` を潰していない）

- [ ] **Step 3: `Modifier::Power` を足す**

`chord_midi/src/model/modifier.rs` の `pub enum Modifier` の**末尾**に足す。BTreeSet の順序が宣言順なので、末尾に置くことで必ず最後に適用され、他の修飾を上書きできる。

```rust
    Tension(Degree),
    /// パワーコード。ルートと5thだけを鳴らす。他の修飾を上書きするため
    /// enum の末尾に置いて BTreeSet の順で最後に適用させる
    Power,
}
```

`impl Display for Modifier` の match に足す。`Modifier::Dominant(5)` の表示と衝突するが、脱糖後の AST をテキストに戻す経路が無いので実害は無い。

```rust
            Modifier::Power => write!(f, "5"),
```

- [ ] **Step 4: `Chord::modify` に腕を足す**

`chord_midi/src/model/chord.rs` の `modify` の match、`Modifier::Tension` の腕の下（`_ => Err(...)` の前）に足す。

```rust
            Modifier::Power => {
                // 他の修飾を全部捨ててルートと5thだけにする
                self.semitones.clear();
                self.semitones.insert(Scale::Major.semitone(1));
                self.semitones.insert(Scale::Major.semitone(5));
                Ok(())
            }
```

- [ ] **Step 5: `resolve` で pow を脱糖する**

`chord_midi/src/model/transform.rs` の `use` に型を足す。

```rust
use crate::model::ast::{Ast, ChordNode, Directive, Func, Node};
use crate::model::modifier::Modifier;
```

（既存の `use` 行に足りないものを足す。すでに import されているものは重複させない）

`impl Ast` の `resolve` を差し替える。

```rust
    pub fn resolve(self) -> Ast {
        let mut tonic = Pitch::C;
        let mut scale = Scale::Major;
        if let Ast::Score(items) = &self {
            for it in items {
                match it.as_ref() {
                    Ast::Directive(Directive::Key(p)) => tonic = *p,
                    Ast::Directive(Directive::Scale(s)) => scale = *s,
                    _ => {}
                }
            }
        }
        let transposed = self.into_pitch(tonic, scale);
        // pow は音程の話なのでここで潰す。pat はリズムの話なので残す
        let desugared = desugar_pow(transposed)
            .into_iter()
            .next()
            .expect("desugar_pow always yields at least one node for a Score");
        match desugared {
            Ast::Score(items) => Ast::Score(
                items
                    .into_iter()
                    .filter(|it| !matches!(it.as_ref(), Ast::Directive(_) | Ast::Comment(_)))
                    .collect(),
            ),
            other => other,
        }
    }
```

同じファイルの `impl Ast` の外側に脱糖の実装を足す。

```rust
/// `Apply(Func::Pow, ..)` を潰し、中の全コードに `Modifier::Power` を足す。
///
/// `Apply` は親のリストの中で複数の要素に展開されるので `Vec` を返す。
fn desugar_pow(ast: Ast) -> Vec<Ast> {
    match ast {
        Ast::Score(items) => vec![Ast::Score(
            items
                .into_iter()
                .flat_map(|item| desugar_pow(*item))
                .map(Box::new)
                .collect(),
        )],
        Ast::Apply(Func::Pow, items) => items
            .into_iter()
            .flat_map(|item| desugar_pow(*item))
            .map(mark_power)
            .collect(),
        Ast::Apply(func, items) => vec![Ast::Apply(
            func,
            items
                .into_iter()
                .flat_map(|item| desugar_pow(*item))
                .map(Box::new)
                .collect(),
        )],
        other => vec![other],
    }
}

/// AST に含まれる全コードに `Modifier::Power` を足す
fn mark_power(ast: Ast) -> Ast {
    match ast {
        Ast::Measure(nodes, br) => {
            Ast::Measure(nodes.into_iter().map(mark_power_node).collect(), br)
        }
        Ast::Apply(func, items) => Ast::Apply(
            func,
            items
                .into_iter()
                .map(|item| Box::new(mark_power(*item)))
                .collect(),
        ),
        other => other,
    }
}

fn mark_power_node(node: Node) -> Node {
    match node {
        Node::Chord(c) => Node::Chord(mark_power_chord(c)),
        Node::Arpeggio(c) => Node::Arpeggio(mark_power_chord(c)),
        Node::Brush(c) => Node::Brush(mark_power_chord(c)),
        Node::Transform(t, c) => Node::Transform(t, mark_power_chord(c)),
        other => other,
    }
}

fn mark_power_chord(mut chord: ChordNode) -> ChordNode {
    chord.modifiers.insert(Modifier::Power);
    chord
}
```

- [ ] **Step 6: テストが通ることを確認する**

Run: `cargo test -p chord_midi`
Expected: PASS

- [ ] **Step 7: コミット**

```bash
git add chord_midi/src/model/modifier.rs chord_midi/src/model/chord.rs chord_midi/src/model/transform.rs
git commit -m "feat(pow): パワーコード化を resolve で脱糖する

Modifier::Power を enum の末尾に足し、BTreeSet の順で最後に適用させて
他の修飾を上書きする。resolve() で Apply(Pow) を潰すので、MIDI 出力と
押さえ方プレビューの両方に効く。pat は残す。"
```

---

## Task 4: pat の MIDI 解釈とブラッシング

**Files:**
- Modify: `chord_midi/src/export/midi.rs`

- [ ] **Step 1: 失敗するテストを書く**

`chord_midi/src/export/midi.rs` の `mod tests` に足す。既存のヘルパ `notes_of` はパースした AST をそのまま解釈するが、`pat` は `resolve()` を通さなくても解釈できる。

```rust
    fn resolved_notes_of(code: &str) -> Vec<Note> {
        let ast = RechordImporter.import(&format!("{}\n", code)).unwrap();
        let mut score = Score::new();
        score.interpret(ast.resolve()).unwrap();
        score.notes
    }

    #[test]
    fn test_pat_splits_measure_into_slots() {
        // "o.o." は4分割。打弦は2回で、それぞれ1スロットぶんの尺になる。
        // 休符は連続すると1つの Note にまとまるので、鳴っている音だけを見る
        let notes = resolved_notes_of("pat(\"o.o.\", C)");
        let sounding: Vec<_> = notes.iter().filter(|n| n.chord.is_some()).collect();
        assert_eq!(sounding.len(), 2);
        assert!(sounding.iter().all(|n| n.duration == 4), "{:?}", sounding);
    }

    #[test]
    fn test_pat_hit_count_matches_pattern() {
        let notes = resolved_notes_of("pat(\"oooo\", C)");
        assert_eq!(notes.iter().filter(|n| n.chord.is_some()).count(), 4);
        let notes = resolved_notes_of("pat(\"o...\", C)");
        assert_eq!(notes.iter().filter(|n| n.chord.is_some()).count(), 1);
    }

    #[test]
    fn test_pat_total_duration_matches_plain_measure() {
        // 全スロットで打弦するパターンなら、総尺はパターン無しと一致する
        let plain = resolved_notes_of("C | F |")
            .iter()
            .map(|n| n.duration)
            .sum::<u32>();
        let patterned = resolved_notes_of("pat(\"oooo\", C | F)")
            .iter()
            .map(|n| n.duration)
            .sum::<u32>();
        assert_eq!(plain, patterned);
    }

    #[test]
    fn test_pat_brush_is_quieter() {
        let notes = resolved_notes_of("pat(\"ox\", C)");
        let sounding: Vec<_> = notes.iter().filter(|n| n.chord.is_some()).collect();
        assert_eq!(sounding.len(), 2);
        assert!(sounding[0].velocity > sounding[1].velocity);
    }

    #[test]
    fn test_pat_follows_the_chord_at_each_slot() {
        // E D の小節に4スロット。前半が E、後半が D
        let notes = resolved_notes_of("pat(\"oooo\", E D)");
        assert_eq!(notes.len(), 4);
        let first = notes[0].chord.clone().unwrap();
        let second = notes[1].chord.clone().unwrap();
        let third = notes[2].chord.clone().unwrap();
        let fourth = notes[3].chord.clone().unwrap();
        assert_eq!(first, second, "前半2スロットは同じコード");
        assert_eq!(third, fourth, "後半2スロットは同じコード");
        assert_ne!(first, third, "前半と後半は違うコード");
    }

    #[test]
    fn test_pat_keeps_rest_as_rest() {
        // 元が休符の位置は o でも鳴らさない。鳴るのは後半2スロットだけ
        let notes = resolved_notes_of("pat(\"oooo\", _ C)");
        let sounding: Vec<_> = notes.iter().filter(|n| n.chord.is_some()).collect();
        assert_eq!(sounding.len(), 2);
        assert!(sounding.iter().all(|n| n.duration == 4));
    }
```

- [ ] **Step 2: テストが失敗することを確認する**

Run: `cargo test -p chord_midi export::midi`
Expected: コンパイルエラー。`no field `velocity` on type `Note``（Task 3 で velocity を足すまで通らない）

- [ ] **Step 3: `Note` に velocity を足す**

`chord_midi/src/export/midi.rs` の `struct Note` とその `impl` を差し替える。

```rust
/// ブラッシング（ミュートした空振り）のベロシティ
const BRUSH_VELOCITY: Velocity = Velocity::new(40);

#[derive(Debug, Clone)]
struct Note {
    pub chord: Option<Vec<NoteNumber>>,
    pub duration: u32,
    pub velocity: Velocity,
}

impl Note {
    fn new(chord: Option<Vec<NoteNumber>>, duration: u32) -> Self {
        Note {
            chord,
            duration,
            velocity: Velocity::default(),
        }
    }

    fn brushed(chord: Option<Vec<NoteNumber>>, duration: u32) -> Self {
        Note {
            chord,
            duration,
            velocity: BRUSH_VELOCITY,
        }
    }
}
```

`write_notes` にベロシティを渡す。

```rust
fn write_notes(
    track: &mut Track,
    ch: Channel,
    notes: &[NoteNumber],
    dur: u32,
    velocity: Velocity,
    skip: &mut u32,
) {
    for (i, n) in notes.iter().enumerate() {
        track
            .push_note_on(if i == 0 { *skip } else { 0 }, ch, *n, velocity)
            .unwrap();
        *skip = 0;
    }
    for (i, n) in notes.iter().enumerate() {
        track
            .push_note_off(if i == 0 { dur } else { 0 }, ch, *n, Velocity::default())
            .unwrap();
    }
}
```

`dump_notes` の呼び出しを直す。

```rust
        write_notes(&mut track, ch, chord, dur, note.velocity, &mut skip);
```

- [ ] **Step 4: ブラッシングのノードを鳴らす**

Task 1 で置いた仮の `Node::Brush` の腕を差し替える。

```rust
            Node::Brush(node) => {
                let chord = node.to_chord()?;
                let notes = into_note_numbers(&chord)?;
                self.pre = Some(chord);
                self.notes.push(Note::brushed(Some(notes), dur));
            }
```

- [ ] **Step 5: `pat` を解釈する**

Task 1 で置いた仮の `Ast::Apply` の腕を差し替える。

```rust
            Ast::Apply(Func::Pat(pattern), items) => {
                for item in items {
                    self.interpret_with_pattern(*item, &pattern)?;
                }
                Ok(())
            }
            Ast::Apply(_, items) => {
                // pow は resolve() で消えているが、素の AST を渡された場合に備える
                for item in items {
                    self.interpret(*item)?;
                }
                Ok(())
            }
```

`impl Score` に2つのメソッドを足す。`interpret` の下に置く。

```rust
    /// `pat(...)` の中身を、パターンに従って読み替えながら解釈する
    fn interpret_with_pattern(&mut self, ast: Ast, pattern: &Pattern) -> Result<()> {
        match ast {
            Ast::Measure(nodes, _) => self.interpret_patterned_measure(nodes, pattern),
            // 内側の pat が優先される
            Ast::Apply(Func::Pat(inner), items) => {
                for item in items {
                    self.interpret_with_pattern(*item, &inner)?;
                }
                Ok(())
            }
            Ast::Apply(_, items) => {
                for item in items {
                    self.interpret_with_pattern(*item, pattern)?;
                }
                Ok(())
            }
            other => self.interpret(other),
        }
    }

    /// 1小節をパターンのスロット数に割り直す
    fn interpret_patterned_measure(&mut self, nodes: Vec<Node>, pattern: &Pattern) -> Result<()> {
        let slots = pattern.0.len() as u32;
        // パターン長は 1/2/4/8/16 に限られるので必ず割り切れる
        let slot_dur = MEASURE_LENGTH / slots;
        // 元の小節で1ノードが占める尺
        let node_dur = Self::measure_unit_size(nodes.len())?;
        for (i, beat) in pattern.0.iter().enumerate() {
            let at = i as u32 * slot_dur;
            let index = ((at / node_dur) as usize).min(nodes.len() - 1);
            let node = nodes[index].clone();
            let node = match (beat, node) {
                // 元が休符のところは打弦もブラッシングもしない
                (_, Node::Rest) => Node::Rest,
                (Beat::Rest, _) => Node::Rest,
                (Beat::Hit, Node::Brush(c)) => Node::Chord(c),
                (Beat::Hit, other) => other,
                (Beat::Brush, Node::Chord(c)) => Node::Brush(c),
                (Beat::Brush, Node::Arpeggio(c)) => Node::Brush(c),
                (Beat::Brush, Node::Transform(_, c)) => Node::Brush(c),
                (Beat::Brush, other) => other,
            };
            self.interpret_node(node, slot_dur)?;
        }
        Ok(())
    }
```

`use` に型を足す。

```rust
use crate::model::ast::{Ast, Beat, Func, Node, Pattern};
```

（既存の `use crate::model::ast::Ast;` と `use crate::model::ast::Node;` の2行をこの1行に置き換える）

- [ ] **Step 6: テストが通ることを確認する**

Run: `cargo test -p chord_midi`
Expected: PASS

- [ ] **Step 7: コミット**

```bash
git add chord_midi/src/export/midi.rs
git commit -m "feat(pat): リズムパターンを MIDI で解釈する

小節をパターンの文字数に割り直し、各スロットの時刻に鳴っている
コードを弾く。o は打弦、. は休符、x はブラッシング。Note に
velocity を足し、ブラッシングはベロシティを下げて表現する。"
```

---

## Task 5: 移調と押さえ方プレビューの追従

**Files:**
- Modify: `chord_midi/src/edit.rs`
- Modify: `chord_midi/src/fingering/mod.rs`

- [ ] **Step 1: 失敗するテストを書く**

`chord_midi/src/edit.rs` の `mod tests` に足す。

```rust
    #[test]
    fn test_shift_key_transposes_inside_pow() {
        assert_eq!(shift_key("pow(C | E D)", 1), "pow(C# | F D#)");
        assert_eq!(shift_key("pow(C# | F D#)", -1), "pow(C | E D)");
    }

    #[test]
    fn test_shift_key_keeps_pattern_literal() {
        // パターン文字列は移調で変わらない
        assert_eq!(shift_key("pat(\"o.o.\", C)", 1), "pat(\"o.o.\", C#)");
    }
```

`chord_midi/src/fingering/mod.rs` の `mod tests` に足す。

```rust
    #[test]
    fn find_handles_power_chords() {
        // pow(C) は C と G の2音になる
        let r = refs("pow(C)\n");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].pitch_classes, mask(&[0, 7]));
        assert!(find(&r[0]).is_some());
    }

    #[test]
    fn measure_refs_ignores_rhythm() {
        // リズムでカードが増えないこと
        let ms = measures("pat(\"o.o.oxox\", C)\n");
        assert_eq!(ms.len(), 1);
        assert_eq!(ms[0].chords.len(), 1);
        assert_eq!(ms[0].chords[0].label, "C");
    }
```

- [ ] **Step 2: テストが失敗することを確認する**

Run: `cargo test -p chord_midi`
Expected: `test_shift_key_transposes_inside_pow` と `test_shift_key_keeps_pattern_literal` は Task 1 で `shift_ast` に `Apply` の腕を足してあるので**通る**はず。`find_handles_power_chords` と `measure_refs_ignores_rhythm` も Task 1 と Task 3 の変更で通るはず。

ここで **1つでも落ちたら** Step 3 に進む。全部通ったら Step 3 は飛ばして Step 4 へ。

- [ ] **Step 3: 落ちたテストを直す**

`shift_key` が落ちる場合は `chord_midi/src/edit.rs` の `shift_ast` に `Ast::Apply` の腕があるか確認する。無ければ足す。

```rust
        Ast::Apply(_, items) => {
            for item in items {
                shift_ast(item, delta, seen);
            }
        }
```

`find_handles_power_chords` が落ちる場合は `chord_midi/src/fingering/mod.rs` の `collect_ast` に `Ast::Apply` の腕があるか確認する。無ければ足す。

```rust
        Ast::Apply(_, items) => {
            for item in items {
                collect_ast(item, out)?;
            }
        }
```

`measure_refs_ignores_rhythm` が落ちる場合は、`Ast::Apply` の中の `Ast::Measure` が `MeasureRef` として1つ積まれているかを確認する。`collect_ast` が `Apply` を素通りして中の `Measure` を処理すればよい。

- [ ] **Step 4: テストが通ることを確認する**

Run: `cargo test -p chord_midi`
Expected: PASS

- [ ] **Step 5: コミット**

```bash
git add chord_midi/src/edit.rs chord_midi/src/fingering/mod.rs
git commit -m "test: pow / pat が移調と押さえ方プレビューに追従することを固定する

キー上下が pow の中のコードを移調しパターン文字列を変えないこと、
リズムで押さえ方カードが増えないことをテストで固定する。"
```

---

## Task 6: 仕上げ

**Files:**
- Modify: `README.md`

- [ ] **Step 1: README に記法を足す**

`README.md` の末尾に足す。既存の内容は消さない。

```markdown
## 関数

    arp(C)              アルペジオ
    blk(F)              ブラックアダーコード
    s25(G7)             ツーファイブ分割
    sub(G7)             裏コード化
    dom(Dm7)            ドミナント化

小節列を引数に取る関数もある。

    pow(C | E D)              パワーコード化（ルートと5thだけ鳴らす）
    pat("o.o.oxox", C | E D)  リズムパターン
    pat("oxox", pow(C))       ネストできる

`pat` のパターンは `o` が打弦、`.` が休符、`x` がブラッシング。
文字数が1小節の分割数になり、1/2/4/8/16 のいずれかだけ書ける。
```

- [ ] **Step 2: 全部のテストを流す**

Run: `cargo test -p chord_midi`
Expected: 全部 PASS

Run: `cd chord_midi_web && cargo check --target wasm32-unknown-unknown`
Expected: error ゼロ

Run: `cd chord_midi_web && trunk build --release --public-url /chord-midi/`
Expected: `✅ success`（CI と同じコマンド。`trunk` が無ければ `cargo install --locked trunk`）

- [ ] **Step 3: ブラウザで動きを見る**

Run: `cd chord_midi_web && trunk serve --port 8088 --no-autoreload`

`http://127.0.0.1:8088/` を開いて次を入力する。

```
key: C
pow(C | E D)
pat("o.o.oxox", C | F)
```

確認する点:
1. エラーが出ないこと
2. プレビュータブで `pow` の小節の押さえ方が2音（ルートと5th）の形になっていること
3. `pat` の小節の押さえ方カードが `C` と `F` の2枚だけで、リズムでカードが増えていないこと
4. 再生するとパワーコードの小節が2音で鳴り、`pat` の小節にリズムが付いていること
5. `♯` を押すと `pow(C# | F D#)` のように中身が移調され、`pat("o.o.oxox", ...)` のパターン文字列は変わらないこと

- [ ] **Step 4: コミットして PR を出す**

```bash
git add README.md
git commit -m "docs: pow() と pat() の記法を README に追記"
git push -u origin feat/pow-and-pat
gh pr create --fill
```

PR 本文には `Closes #20` を入れる。
