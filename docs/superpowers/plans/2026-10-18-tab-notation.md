# tab 記法 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `tab(8 6 6 x x x)` と書いて、コード名を介さずギターの押さえ方そのものを入力できるようにする。

**Architecture:** `Node` に `Tab([Option<u8>; 6], bool)` を足す。`ChordExpr` には入れない (`eval` が `ChordNode` を返す契約に合わないため)。MIDI は `OPEN_STRING_NOTES[i] + fret` をそのまま鳴らし、図は書かれたフレットをそのまま描く。図の入力型は `&'static Fingering` から所有型 `Shape` に変える。

**Tech Stack:** Rust 2021 / nom 7 (`nom_locate` + `nom_tracable`) / yew 0.21 (web) / midly (MIDI)

**設計書:** `docs/superpowers/specs/2026-10-18-tab-notation-design.md`

---

## 前提

- 作業ブランチは `feat/tab-notation` (設計書のコミット済み)
- テストは `cargo test -q -p chord_midi` (workspace は `cargo test -q --workspace`)
- web は workspace 外。`cd chord_midi_web && cargo check --target wasm32-unknown-unknown`
- 整形は `cargo fmt --all`。コミット前に必ず走らせる

## ファイル構成

| ファイル | 責務 | 変更 |
|---|---|---|
| `chord_midi/src/model/ast.rs` | `Node::Tab` の定義 | 変種を足す |
| `chord_midi/src/import/chord.rs` | `tab(...)` のパース | `tab_parser` を足し `node_parser` に繋ぐ |
| `chord_midi/src/export/rechord.rs` | 書き戻し | `Display for Node` に枝 |
| `chord_midi/src/export/midi.rs` | 実音 | `tab_notes` と `interpret_node` の枝、`apply_pattern` のブラッシング |
| `chord_midi/src/model/transform.rs` | 度数変換 / 修飾の付与 | 素通しの枝 (2箇所) |
| `chord_midi/src/edit.rs` | 移調 | フレットを平行移動、`shift_key` を `Result` に |
| `chord_midi/src/fingering/mod.rs` | 図に渡す形 | `Shape` / `shape_of` / `ChordRef.tab` |
| `chord_midi/src/fingering/identify.rs` | 逆引き | `Candidate::reproduces_shape` を落とす |
| `chord_midi/src/reference.rs` | 記法表 | 1行足す |
| `chord_midi/src/import/sexp.rs` | S式入力 | `(tab ...)` を読む |
| `chord_midi_cli/src/diagram.rs` | 端末の図 | 入力型を `Shape` に |
| `chord_midi_web/src/diagram.rs` | web の図 | props を `Shape` に |
| `chord_midi_web/src/fretboard.rs` | 指板パネル | 挿入する記法を `tab(...)` に |
| `chord_midi_web/src/main.rs` | 画面 | `shape_of` に差し替え、`shift_key` の `Result` 対応 |

---

### Task 1: `Node::Tab` とパース・書き戻し・実音

**Files:**
- Modify: `chord_midi/src/model/ast.rs` (`enum Node`)
- Modify: `chord_midi/src/import/chord.rs` (`node_parser` の手前に `tab_parser`)
- Modify: `chord_midi/src/export/rechord.rs` (`impl Display for Node`)
- Modify: `chord_midi/src/export/midi.rs` (`interpret_node`)
- Modify: `chord_midi/src/model/transform.rs` (2箇所の match)
- Modify: `chord_midi/src/edit.rs` (`shift_node`)
- Modify: `chord_midi/src/fingering/mod.rs` (`collect_node`)

このタスクの終わりで `tab(8 6 6 x x x) |` が鳴り、同じ文字列に書き戻せる。移調と図は Task 3 / Task 5 で足す。

- [ ] **Step 1: パーサの失敗するテストを書く**

`chord_midi/src/import/rechord.rs` の `mod tests` の末尾 (`}` の直前) に足す:

```rust
    #[test]
    fn test_tab_parses_six_frets() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::{Expr, Node};
        let ast = RechordImporter.import("tab(8 6 6 x x x) |\n").unwrap();
        let Expr::Seq(items) = &ast else {
            panic!("not a score")
        };
        let Expr::Measure(nodes, _) = &items[0] else {
            panic!("not a measure: {:?}", items[0])
        };
        assert_eq!(
            nodes[0],
            Node::Tab([Some(8), Some(6), Some(6), None, None, None], false)
        );
    }

    #[test]
    fn test_tab_rejects_bad_input() {
        use crate::import::{Importer, RechordImporter};
        // 弦の数が6本でない
        assert!(RechordImporter.import("tab(8 6 6 x x) |\n").is_err());
        assert!(RechordImporter.import("tab(8 6 6 x x x x) |\n").is_err());
        // フレットが範囲外
        assert!(RechordImporter.import("tab(25 0 0 0 0 0) |\n").is_err());
        // 空
        assert!(RechordImporter.import("tab() |\n").is_err());
    }

    #[test]
    fn test_tab_cannot_be_wrapped_by_chord_functions() {
        use crate::import::{Importer, RechordImporter};
        // tab はコード名を持たないので、コードに掛かる関数には渡せない
        assert!(RechordImporter.import("pow(tab(8 6 6 x x x)) |\n").is_err());
        assert!(RechordImporter.import("arp(tab(8 6 6 x x x)) |\n").is_err());
        assert!(RechordImporter.import("blk(tab(8 6 6 x x x)) |\n").is_err());
        assert!(RechordImporter.import("s25(tab(8 6 6 x x x)) |\n").is_err());
        // pat は小節に掛かるので書ける
        assert!(RechordImporter
            .import("pat(\"o.o.\", tab(8 6 6 x x x)) |\n")
            .is_ok());
    }

    #[test]
    fn test_tab_accepts_two_digit_frets_and_extra_spaces() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::{Expr, Node};
        let ast = RechordImporter.import("tab(10  12 12 x x x) |\n").unwrap();
        let Expr::Seq(items) = &ast else { panic!() };
        let Expr::Measure(nodes, _) = &items[0] else {
            panic!()
        };
        assert_eq!(
            nodes[0],
            Node::Tab([Some(10), Some(12), Some(12), None, None, None], false)
        );
    }
```

- [ ] **Step 2: テストを走らせて落ちることを確かめる**

Run: `cargo test -q -p chord_midi tab_ 2>&1 | tail -20`
Expected: コンパイルエラー `no variant or associated item named `Tab` found for enum `Node``

- [ ] **Step 3: `Node::Tab` を足す**

`chord_midi/src/model/ast.rs` の `enum Node`、`Split(ChordExpr),` の次の行に:

```rust
    /// `tab(...)`。6弦→1弦のフレット。`None` はミュート、`Some(0)` は開放弦。
    /// コード名を持たないので `ChordExpr` には入らない (`eval` が `ChordNode`
    /// を返す契約に合わないため)。2つ目はブラッシングで、`Node::Brush` と
    /// 同じくパーサは常に false で作り、`pat` の `x` スロットだけが true にする
    Tab([Option<u8>; 6], bool),
```

- [ ] **Step 4: パーサを足す**

`chord_midi/src/import/chord.rs` の import を足す (既存の行を置き換える):

```rust
use nom::bytes::complete::tag;
use nom::character::complete::{digit1, one_of, space0, space1};
use nom::combinator::{map, map_res, opt, value};
use nom::multi::{many0, separated_list1};
```

`node_parser` の直前に足す:

```rust
/// `tab()` に書けるフレットの上限。ギターの実用上限に合わせる
const MAX_TAB_FRET: u8 = 24;

/// `tab(...)` の中の1弦ぶん。`x` はミュート、それ以外は 0〜24 の10進
fn fret_parser(s: Span) -> IResult<Span, Option<u8>> {
    alt((
        value(None, tag("x")),
        map_res(digit1, |d: Span| -> Result<Option<u8>> {
            let n: u32 = d.fragment().parse()?;
            if n <= MAX_TAB_FRET as u32 {
                Ok(Some(n as u8))
            } else {
                Err(anyhow::anyhow!("フレットは 0〜{}: {}", MAX_TAB_FRET, n))
            }
        }),
    ))(s)
}

/// `tab(8 6 6 x x x)`。6弦→1弦の順でちょうど6本書く
#[tracable_parser]
fn tab_parser(s: Span) -> IResult<Span, Node> {
    map_res(
        delimited(
            tuple((tag("tab("), space0)),
            separated_list1(space1, fret_parser),
            tuple((space0, tag(")"))),
        ),
        |frets: Vec<Option<u8>>| -> Result<Node> {
            let len = frets.len();
            let frets: [Option<u8>; 6] = frets
                .try_into()
                .map_err(|_| anyhow::anyhow!("tab は6弦ぶん書くこと: {} 本だった", len))?;
            Ok(Node::Tab(frets, false))
        },
    )(s)
}
```

`node_parser` の `alt((...))` の先頭 (`map(tag("="), ...)` の前) に足す:

```rust
        // `tab(` は他のどの枝とも先頭が重ならないが、コード名の枝より
        // 前に置いて読み落としが起きないようにする
        tab_parser,
```

- [ ] **Step 5: `Node` を網羅している match に枝を足す**

`chord_midi/src/model/transform.rs` の2箇所 (`into_degree` の中と `mark_node`):

```rust
                        // 音程を持たないものはそのまま
                        n @ (Node::Rest | Node::Sustain | Node::Repeat) => n,
```
を次に置き換える (2箇所とも同じ):
```rust
                        // 音程や度数を持たないものはそのまま。tab は絶対的な
                        // 押さえ方なので、度数にも修飾にも関係しない
                        n @ (Node::Rest | Node::Sustain | Node::Repeat | Node::Tab(_, _)) => n,
```

`mark_node` 側はインデントが浅いので:
```rust
        n @ (Node::Rest | Node::Sustain | Node::Repeat | Node::Tab(_, _)) => n,
```

`chord_midi/src/edit.rs` の `shift_node`:
```rust
        // tab の移調は Task 5 で足す
        Node::Rest | Node::Sustain | Node::Repeat | Node::Tab(_, _) => {}
```

`chord_midi/src/fingering/mod.rs` の `collect_node`:
```rust
        // tab の図は Task 3 で足す
        Node::Rest | Node::Sustain | Node::Repeat | Node::Tab(_, _) => {}
```

- [ ] **Step 6: 書き戻しを足す**

`chord_midi/src/export/rechord.rs` の `impl Display for Node` の `Node::Rest => ...` の前に:

```rust
            Node::Tab(frets, _) => {
                // ブラッシングのフラグは出さない。記法には無いもので、
                // `pat` の `x` が作る値だけが持つ (`Node::Brush` と同じ)
                let body = frets
                    .iter()
                    .map(|fret| match fret {
                        Some(n) => n.to_string(),
                        None => "x".to_string(),
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                write!(f, "tab({})", body)
            }
```

- [ ] **Step 7: パーサのテストが通ることを確かめる**

Run: `cargo test -q -p chord_midi tab_ 2>&1 | tail -10`
Expected: PASS (4件)

- [ ] **Step 8: 実音と書き戻しの失敗するテストを書く**

`chord_midi/src/export/midi.rs` の `mod tests` の末尾に:

```rust
    #[test]
    fn test_tab_sounds_the_pressed_strings() {
        // E メジャーの押さえ方。開放弦の MIDI 番号 + フレット
        let notes = notes_of("tab(0 2 2 1 0 0)");
        let chord: Vec<u8> = notes
            .iter()
            .filter_map(|n| n.chord.as_ref())
            .flatten()
            .map(|n| n.as_int())
            .collect();
        assert_eq!(chord, vec![40, 47, 52, 56, 59, 64]);
    }

    #[test]
    fn test_tab_skips_muted_strings() {
        let notes = notes_of("tab(x x 3 2 1 x)");
        let chord: Vec<u8> = notes
            .iter()
            .filter_map(|n| n.chord.as_ref())
            .flatten()
            .map(|n| n.as_int())
            .collect();
        assert_eq!(chord, vec![53, 57, 60]);
    }
```

`chord_midi/src/export/rechord.rs` の `mod tests` の末尾に:

```rust
    #[test]
    fn test_tab_roundtrip() {
        assert_eq!(roundtrip("tab(8 6 6 x x x) |"), "tab(8 6 6 x x x) |");
        assert_eq!(roundtrip("tab(0 2 2 1 0 0) |"), "tab(0 2 2 1 0 0) |");
        // 入力の余分な空白は正規化される
        assert_eq!(roundtrip("tab(10  12 12 x x x) |"), "tab(10 12 12 x x x) |");
    }
```

- [ ] **Step 9: テストを走らせて落ちることを確かめる**

Run: `cargo test -q -p chord_midi test_tab_ 2>&1 | tail -20`
Expected: `test_tab_roundtrip` は PASS (Step 6 で実装済み)、`test_tab_sounds_the_pressed_strings` と `test_tab_skips_muted_strings` はコンパイルエラー `non-exhaustive patterns: `Node::Tab(_, _)` not covered` (`interpret_node`)

- [ ] **Step 10: 実音を足す**

`chord_midi/src/export/midi.rs` の `voicing` の下に:

```rust
/// `tab(...)` の押さえ方を実音にする。`voicing` が押さえ方データに当たった
/// ときと同じ計算で、ミュートした弦は飛ばす
fn tab_notes(frets: &[Option<u8>; 6]) -> Vec<NoteNumber> {
    frets
        .iter()
        .enumerate()
        .filter_map(|(i, fret)| fret.map(|n| NoteNumber::new(OPEN_STRING_NOTES[i] + n)))
        .collect()
}
```

`interpret_node` の `Node::Repeat => {` の前に:

```rust
            Node::Tab(ref frets, brushed) => {
                let notes = tab_notes(frets);
                self.pre = Some(notes.clone());
                if brushed {
                    self.notes.push(Note::brushed(Some(notes), dur));
                } else {
                    // `Node::Chord` と同じく、次のノードまで伸ばす
                    self.sustain = dur;
                }
            }
```

- [ ] **Step 11: テストが通ることを確かめる**

Run: `cargo test -q -p chord_midi 2>&1 | tail -5`
Expected: すべて PASS

- [ ] **Step 12: 整形してコミット**

```bash
cd /Users/kmt/dev/chord-midi
cargo fmt --all
cargo test -q --workspace 2>&1 | tail -3
git add -A
git commit -m "$(cat <<'MSG'
feat: 押さえ方を直接書く tab(...) 記法を足す

コード名を介さずギターの押さえ方そのものを書けるようにする。指板で
押さえた形をコード名に直すと fingering::find の近似を通ってしまい、
元の形が戻らないことがあるため。

`Node::Tab([Option<u8>; 6], bool)` を足す。ChordExpr には入れない。
ChordExpr::eval が ChordNode を返す契約で、コード名を持たない tab は
型が合わないため。結果として pow/oct/arp/blk/s25 は tab に掛からない。

このコミットではパース・書き戻し・実音まで。移調と図は後続。

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01F28qZZi3uLGGKV2azgKPX8
MSG
)"
```

---

### Task 2: `pat` のブラッシングを tab にも効かせる

**Files:**
- Modify: `chord_midi/src/export/midi.rs` (`apply_pattern` の beat 合成)

- [ ] **Step 1: 失敗するテストを書く**

`chord_midi/src/export/midi.rs` の `mod tests` の末尾に:

```rust
    #[test]
    fn test_tab_takes_brush_from_pattern() {
        // "oxox" の 2/4 スロットがブラッシング。音は同じで強さだけ落ちる
        let notes = notes_of("pat(\"oxox\", tab(0 2 2 1 0 0))");
        let velocities: Vec<u8> = notes
            .iter()
            .filter(|n| n.chord.is_some())
            .map(|n| n.velocity.as_int())
            .collect();
        assert_eq!(velocities.len(), 4);
        assert!(velocities[0] > velocities[1], "打弦よりブラッシングが弱い");
        assert_eq!(velocities[0], velocities[2]);
        assert_eq!(velocities[1], velocities[3]);
    }
```

- [ ] **Step 2: テストを走らせて落ちることを確かめる**

Run: `cargo test -q -p chord_midi test_tab_takes_brush 2>&1 | tail -10`
Expected: FAIL。4つとも同じベロシティになる (`(Beat::Brush, other) => other` に落ちるため)

- [ ] **Step 3: beat の合成に tab を足す**

`chord_midi/src/export/midi.rs` の `apply_pattern` の match を次に置き換える:

```rust
            let node = match (beat, node) {
                // 元が休符のところは打弦もブラッシングもしない
                (_, Node::Rest) => Node::Rest,
                (Beat::Rest, _) => Node::Rest,
                (Beat::Hit, Node::Brush(c)) => Node::Chord(c),
                (Beat::Hit, Node::Tab(frets, _)) => Node::Tab(frets, false),
                (Beat::Hit, other) => other,
                (Beat::Brush, Node::Chord(c)) => Node::Brush(c),
                (Beat::Brush, Node::Arpeggio(c)) => Node::Brush(c),
                // tab はブラッシング用の別変種を持たず、自分のフラグを立てる
                (Beat::Brush, Node::Tab(frets, _)) => Node::Tab(frets, true),
                (Beat::Brush, other) => other,
            };
```

- [ ] **Step 4: テストが通ることを確かめる**

Run: `cargo test -q -p chord_midi test_tab_takes_brush 2>&1 | tail -5`
Expected: PASS

- [ ] **Step 5: 整形してコミット**

```bash
cd /Users/kmt/dev/chord-midi
cargo fmt --all
cargo test -q -p chord_midi 2>&1 | tail -3
git add -A
git commit -m "$(cat <<'MSG'
feat: pat の x (ブラッシング) を tab にも効かせる

tab はブラッシング用の別変種を持たないので、自分の bool を立てて
弱いベロシティで鳴らす。Node::Brush(ChordExpr) は ChordExpr しか
包めないため。

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01F28qZZi3uLGGKV2azgKPX8
MSG
)"
```

---

### Task 3: 図に渡す所有型 `Shape` と CLI の図

**Files:**
- Modify: `chord_midi/src/fingering/mod.rs` (`Shape` / `shape_of` / `ChordRef.tab` / `collect_node`)
- Modify: `chord_midi_cli/src/diagram.rs` (入力型)
- Modify: `chord_midi_cli/src/main.rs:120` (`find` → `shape_of`)

- [ ] **Step 1: 失敗するテストを書く**

`chord_midi/src/fingering/mod.rs` の `mod tests` の末尾に:

```rust
    #[test]
    fn tab_node_becomes_a_chord_ref_with_its_own_shape() {
        let r = refs("tab(8 6 6 x x x) |\n");
        assert_eq!(r.len(), 1);
        assert_eq!(
            r[0].tab,
            Some([Some(8), Some(6), Some(6), None, None, None])
        );
        let shape = shape_of(&r[0]).expect("tab は必ず形を持つ");
        assert_eq!(shape.frets, [Some(8), Some(6), Some(6), None, None, None]);
        // 原典の形ではないので「≒」用の名前は無い
        assert_eq!(shape.name, None);
        // 指番号のデータは無い
        assert_eq!(shape.fingers, [None; 6]);
    }

    #[test]
    fn shape_of_falls_back_to_the_data_shape() {
        let r = refs("C |\n");
        let shape = shape_of(&r[0]).expect("C は原典にある");
        assert_eq!(shape.frets, [None, Some(3), Some(2), Some(0), Some(1), None]);
        assert_eq!(shape.name, Some("C"));
    }

    #[test]
    fn tab_label_comes_from_identify() {
        // 6弦8/5弦6/4弦6 = C, D#, G# = G# のトライアドで最低音が C
        let r = refs("tab(8 6 6 x x x) |\n");
        assert_eq!(r[0].label, "G#/C");
    }

    #[test]
    fn tab_label_falls_back_to_the_notation() {
        // 1音だけではコード名が決まらない。記法をそのまま見出しにする
        let r = refs("tab(x x x x x 0) |\n");
        assert_eq!(r[0].label, "tab(x x x x x 0)");
    }
```

- [ ] **Step 2: テストを走らせて落ちることを確かめる**

Run: `cargo test -q -p chord_midi tab_node_becomes 2>&1 | tail -10`
Expected: コンパイルエラー `no field `tab` on type `ChordRef``

- [ ] **Step 3: `Shape` と `ChordRef.tab` を足す**

`chord_midi/src/fingering/mod.rs` の `pub struct ChordRef` の直前に:

```rust
/// 図に描ける押さえ方。原典の形 (`Fingering`) と `tab()` が書いた形を
/// 同じ型で扱うための所有型。`tab()` の形は実行時の値で `'static` に
/// できないので、図の側は `&'static Fingering` を受けられない
#[derive(Debug, Clone, PartialEq)]
pub struct Shape {
    /// 6弦→1弦。None = ミュート、Some(0) = 開放
    pub frets: [Option<u8>; 6],
    /// 押さえる指。`tab()` 由来なら全部 None
    pub fingers: [Option<Finger>; 6],
    /// 原典での呼び名。`tab()` 由来なら None。
    /// 図はこれが Some でコード名と違うときだけ「≒」を併記する
    pub name: Option<&'static str>,
}

impl Shape {
    /// 開放弦を除いた一番低いフレット。図の窓の開始位置に使う
    pub fn min_fret(&self) -> Option<u8> {
        self.frets.iter().flatten().copied().filter(|f| *f > 0).min()
    }

    /// 開放弦を除いた一番高いフレット
    pub fn max_fret(&self) -> Option<u8> {
        self.frets.iter().flatten().copied().filter(|f| *f > 0).max()
    }
}

impl From<&'static Fingering> for Shape {
    fn from(f: &'static Fingering) -> Self {
        Shape {
            frets: f.frets,
            fingers: f.fingers,
            name: Some(f.name),
        }
    }
}
```

`pub struct ChordRef` の末尾のフィールドとして:

```rust
    /// `tab()` が直接書いた押さえ方。書かれていれば `find` を通さず
    /// この形をそのまま図に出す
    pub tab: Option<[Option<u8>; 6]>,
```

`chord_ref` の中で `ChordRef { ... }` を組んでいる箇所に `tab: None,` を足す
(コンパイルエラーが指す場所。`ChordRef` を組んでいるのは `chord_ref` の1箇所)。

- [ ] **Step 4: `shape_of` と tab の `ChordRef` を足す**

`chord_midi/src/fingering/mod.rs` の `pub fn find` の直後に:

```rust
/// 図に出す押さえ方。`tab()` が書いた形があればそれを、無ければ `find` の
/// 結果を返す。見つからなければ None
pub fn shape_of(chord: &ChordRef) -> Option<Shape> {
    if let Some(frets) = chord.tab {
        return Some(Shape {
            frets,
            fingers: [None; 6],
            name: None,
        });
    }
    find(chord).map(Shape::from)
}

/// `tab()` の押さえ方から `ChordRef` を作る。
///
/// 見出しは逆引きの先頭候補。候補が無ければ記法そのものを見出しにする。
/// `pitch_classes` と `bass` も鳴る音から出しておくので、`find` を通しても
/// 破綻はしない (通る経路は無いが、空の集合を配って事故るのを避ける)
fn tab_ref(frets: [Option<u8>; 6]) -> ChordRef {
    let mut pitch_classes = 0u16;
    let mut lowest: Option<(u16, u8)> = None;
    for (s, fret) in frets.iter().enumerate() {
        let Some(fret) = fret else { continue };
        let midi = u16::from(OPEN_STRING_PITCHES[s]) + u16::from(*fret);
        let pc = (midi % 12) as u8;
        pitch_classes |= 1u16 << pc;
        if lowest.is_none_or(|(m, _)| midi < m) {
            lowest = Some((midi, pc));
        }
    }
    let label = match identify(frets).first() {
        Some(c) => c.name.clone(),
        None => Node::Tab(frets, false).to_string(),
    };
    ChordRef {
        label,
        root: lowest.map(|(_, pc)| pc).unwrap_or(0),
        pitch_classes,
        bass: lowest.map(|(_, pc)| pc),
        root_string: None,
        wanted: None,
        tab: Some(frets),
    }
}
```

`collect_node` の tab の枝を Task 1 の `{}` から差し替える:

```rust
        Node::Tab(frets, _) => out.push(tab_ref(*frets)),
        Node::Rest | Node::Sustain | Node::Repeat => {}
```

`chord_midi/src/fingering/mod.rs` の `use` に `Node` が無ければ足す
(既に `crate::model::ast::{ChordNode, Expr, Func, Node, Pattern}` を import 済み)。

- [ ] **Step 5: テストが通ることを確かめる**

Run: `cargo test -q -p chord_midi tab_ 2>&1 | tail -10`
Expected: PASS (`tab_node_becomes_a_chord_ref_with_its_own_shape`, `shape_of_falls_back_to_the_data_shape`, `tab_label_comes_from_identify`, `tab_label_falls_back_to_the_notation`)

- [ ] **Step 6: CLI の図を `Shape` に移す**

`chord_midi_cli/src/diagram.rs`:

```rust
use chord_midi::fingering::{Finger, Shape};
```

`window_start` と `render` の引数の型を変える:

```rust
/// 描き始めるフレット。5フレットに収まらないときだけ最小フレットから始める
fn window_start(s: &Shape) -> u8 {
    match (s.min_fret(), s.max_fret()) {
        (Some(min), Some(max)) if max > FRET_COUNT => min,
        _ => 1,
    }
}

/// コード1つ分の図。見つからなければ「押さえ方データなし」だけ返す
pub fn render(label: &str, shape: Option<Shape>) -> String {
    let Some(f) = shape else {
        return format!("{}\n  (押さえ方データなし)\n", label);
    };
```

`render` の中の見出しの組み立て (`let head = if f.name == label { ... }`) を
`Option` 対応に直す:

```rust
    // 代替の押さえ方が選ばれたときだけ、原典での名前を併記する。
    // tab() の形は代替ではないので name が None で、ここには来ない
    let head = match f.name {
        Some(name) if name != label => format!("{} (≒ {})", label, name),
        _ => label.to_string(),
    };
```

`f.frets` / `f.fingers` は `Fingering` と同じフィールド名なのでそのまま通る。

`mod tests` のヘルパも `Shape` を返すようにする:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use chord_midi::fingering::FINGERINGS;

    fn by_name(name: &str) -> Shape {
        FINGERINGS
            .iter()
            .find(|f| f.name == name)
            .unwrap()
            .into()
    }
```

`use chord_midi::fingering::{Finger, Shape};` に変えたので、ファイル先頭の
`Fingering` の import は消えている。テストの `by_name` 以外は変更不要。

`chord_midi_cli/src/main.rs:120`:

```rust
            print!("{}", diagram::render(&chord.label, fingering::shape_of(chord)));
```

- [ ] **Step 7: CLI が動くことを確かめる**

```bash
cd /Users/kmt/dev/chord-midi
printf 'tab(8 6 6 x x x) | C |\n' > /tmp/tab-check.rechord
cargo run -q -p chord_midi_cli -- -i /tmp/tab-check.rechord --fingering
```
Expected: 1つ目に `G#/C` の見出しで 8/6/6 のドットが出る図、2つ目に `C` の原典の図

- [ ] **Step 8: 整形してコミット**

```bash
cd /Users/kmt/dev/chord-midi
cargo fmt --all
cargo test -q --workspace 2>&1 | tail -3
git add -A
git commit -m "$(cat <<'MSG'
feat: 図に渡す押さえ方を所有型 Shape にして tab を描けるようにする

tab() の形は実行時の値で 'static にできないので、図の側が
&'static Fingering を受けていると渡せない。原典の形と tab の形を
同じ型で扱う Shape を足し、shape_of が使い分ける。

tab の見出しは identify の先頭候補。候補が無ければ記法そのもの。

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01F28qZZi3uLGGKV2azgKPX8
MSG
)"
```

---

### Task 4: web の図を `Shape` に移す

**Files:**
- Modify: `chord_midi_web/src/diagram.rs` (props)
- Modify: `chord_midi_web/src/main.rs` (`cards` の組み立て)

- [ ] **Step 1: props を `Shape` に変える**

`chord_midi_web/src/diagram.rs`:

```rust
use chord_midi::fingering::Shape;
use yew::prelude::*;
```

```rust
#[derive(Properties, PartialEq)]
pub struct ChordDiagramProps {
    /// 入力に書かれたコード名
    pub label: String,
    /// 描く押さえ方。無ければ None
    pub shape: Option<Shape>,
    /// カードの色 (CSS の色文字列)
    pub color: String,
}
```

`chord_diagram` の中:

```rust
    let body = match &props.shape {
        Some(s) => diagram(s),
        None => html! { <div class="diagram-empty">{"データなし"}</div> },
    };
    // 代替の押さえ方が選ばれたときだけ、コード名の直後に原典での名前を出す。
    // tab() の形は代替ではないので name が None で、ここには来ない
    let alias = props.shape.as_ref().and_then(|s| match s.name {
        Some(name) if name != props.label => Some(html! {
            <span class="chord-alias">{format!(" (≒ {})", name)}</span>
        }),
        _ => None,
    });
```

`window_start` と `diagram` の引数を `&Shape` に変える:

```rust
fn window_start(f: &Shape) -> u8 {
fn diagram(f: &Shape) -> Html {
```

- [ ] **Step 2: 呼び出し側を直す**

`chord_midi_web/src/main.rs` の `cards` の中:

```rust
                cards.push(html! {
                    <ChordDiagram
                        label={c.label.clone()}
                        shape={fingering::shape_of(c)}
                        color={PALETTE[index % PALETTE.len()].to_string()}
                    />
                });
```

`use chord_midi::fingering::{self, MeasureRef};` は既にあるのでそのまま。

- [ ] **Step 3: web がコンパイルできることを確かめる**

Run: `cd /Users/kmt/dev/chord-midi/chord_midi_web && cargo check --target wasm32-unknown-unknown 2>&1 | tail -5`
Expected: エラー無し

- [ ] **Step 4: 画面で見る**

```bash
cd /Users/kmt/dev/chord-midi/chord_midi_web
trunk serve --port 8099 --no-autoreload
```
ブラウザで `http://127.0.0.1:8099/` を開き、エディタに
`C | tab(8 6 6 x x x) |` と入れてプレビュータブを見る。
Expected: 2枚目のカードの見出しが `G#/C`、`(≒ ...)` は出ず、
ドットが 6弦8 / 5弦6 / 4弦6 の位置に出る。確認したら `Ctrl-C` で止める。

- [ ] **Step 5: 整形してコミット**

```bash
cd /Users/kmt/dev/chord-midi
cargo fmt --all
(cd chord_midi_web && cargo fmt --all && cargo check -q --target wasm32-unknown-unknown)
git add -A
git commit -m "$(cat <<'MSG'
feat(web): コード図を Shape で受けて tab の形を描く

tab() の形は 'static ではないので props の型を変える。原典名が
None のときは「≒」を出さない。tab は代替ではなく書かれた形そのもの。

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01F28qZZi3uLGGKV2azgKPX8
MSG
)"
```

---

### Task 5: 移調

**Files:**
- Modify: `chord_midi/src/edit.rs` (`shift_node` / `transpose` / `shift_key`)
- Modify: `chord_midi_cli/src/main.rs:109`
- Modify: `chord_midi_web/src/main.rs` (`on_key_up` / `on_key_down`)

- [ ] **Step 1: 失敗するテストを書く**

`chord_midi/src/edit.rs` の `mod tests` の末尾に:

```rust
    #[test]
    fn shift_key_moves_tab_frets() {
        assert_eq!(
            shift_key("tab(4 6 6 x x x) |", 1).unwrap(),
            "tab(5 7 7 x x x) |"
        );
        assert_eq!(
            shift_key("tab(4 6 6 x x x) |", -1).unwrap(),
            "tab(3 5 5 x x x) |"
        );
    }

    #[test]
    fn shift_key_refuses_to_push_a_tab_below_the_nut() {
        // 開放弦を含む形は下げられない
        let e = shift_key("tab(0 2 2 1 0 0) |", -1).unwrap_err();
        assert!(
            e.to_string().contains("tab"),
            "tab が下げられないことを伝える: {}",
            e
        );
    }

    #[test]
    fn shift_key_moves_chords_and_tabs_together() {
        assert_eq!(
            shift_key("C | tab(4 6 6 x x x) |", 1).unwrap(),
            "C# | tab(5 7 7 x x x) |"
        );
    }
```

- [ ] **Step 2: テストを走らせて落ちることを確かめる**

Run: `cargo test -q -p chord_midi shift_key_moves_tab 2>&1 | tail -10`
Expected: コンパイルエラー `no method named `unwrap` found for struct `String``

- [ ] **Step 3: `shift_key` を `Result` にして tab を動かす**

`chord_midi/src/edit.rs`:

```rust
pub fn shift_key(text: &str, delta: i8) -> Result<String> {
    match transpose(text, delta) {
        Some(shifted) => shifted,
        // パースできない入力は AST を作れないので、`key:` の行だけ書き換える
        None => Ok(shift_key_directive(text, delta)),
    }
}
```

`transpose` の戻り値を `Option<Result<String>>` にする。
「パースできない = None」「パースできたが動かせない = Some(Err)」を分けるため:

```rust
fn transpose(text: &str, delta: i8) -> Option<Result<String>> {
    let mut ast = RechordImporter.import(&format!("{}\n", text)).ok()?;
    let mut seen = Seen::default();
    Some(shift_expr(&mut ast, delta, &mut seen).map(|()| ast.to_string().trim_end().to_string()))
}
```

`shift_expr` / `shift_node` / `shift_chord` の連鎖を `Result<()>` にする。
`shift_node` の tab の枝:

```rust
        Node::Tab(frets, _) => {
            for fret in frets.iter_mut().flatten() {
                let next = i16::from(*fret) + i16::from(delta);
                if next < 0 {
                    return Err(anyhow::anyhow!(
                        "tab を {} 半音ずらすとナットより下になる。開放弦を含む形は下げられない",
                        delta
                    ));
                }
                *fret = next as u8;
            }
            Ok(())
        }
        Node::Rest | Node::Sustain | Node::Repeat => Ok(()),
```

`edit.rs` の先頭に `use anyhow::Result;` が無ければ足す。

**注意:** `transpose` は AST を丸ごと書き換えてから文字列化するので、
エラーのときは元の `text` がそのまま返る (呼び出し側が `?` で落とすため、
書き換え途中の AST は捨てられる)。

- [ ] **Step 4: 呼び出し側を直す**

`chord_midi_cli/src/main.rs:109`:

```rust
        Some(delta) => shift_key(code.as_str(), delta)?,
```

`chord_midi_web/src/main.rs` の `on_key_up` / `on_key_down`:

```rust
    let on_key_up = {
        let input = input.clone();
        let error = error.clone();
        Callback::from(move |_| match shift_key(input.as_str(), 1) {
            Ok(next) => input.set(next),
            Err(e) => error.set(e.to_string()),
        })
    };
    let on_key_down = {
        let input = input.clone();
        let error = error.clone();
        Callback::from(move |_| match shift_key(input.as_str(), -1) {
            Ok(next) => input.set(next),
            Err(e) => error.set(e.to_string()),
        })
    };
```

`error` は `let error = use_state(String::new);` で既に存在する。
`on_key_up` / `on_key_down` の定義は `error` の定義より後ろにあるのでそのまま書ける。

- [ ] **Step 5: テストが通ることを確かめる**

Run: `cargo test -q --workspace 2>&1 | tail -5`
Expected: すべて PASS

- [ ] **Step 6: web がコンパイルできることを確かめる**

Run: `cd /Users/kmt/dev/chord-midi/chord_midi_web && cargo check --target wasm32-unknown-unknown 2>&1 | tail -5`
Expected: エラー無し

- [ ] **Step 7: 整形してコミット**

```bash
cd /Users/kmt/dev/chord-midi
cargo fmt --all
(cd chord_midi_web && cargo fmt --all)
cargo test -q --workspace 2>&1 | tail -3
git add -A
git commit -m "$(cat <<'MSG'
feat: 移調で tab のフレットを平行移動する

形を保ったまま半音 = 1フレットで動かす。どれかの弦が 0 未満になる
ときは何も変えずにエラーにする。

shift_key の戻り値を Result<String> に変えた。String のままだと
失敗を黙って握り潰すしかないため。web は既存のエラー表示欄に出す。

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01F28qZZi3uLGGKV2azgKPX8
MSG
)"
```

---

### Task 6: 指板パネルの挿入を `tab(...)` にする

**Files:**
- Modify: `chord_midi/src/fingering/identify.rs` (`Candidate::reproduces_shape` を落とす)
- Modify: `chord_midi_web/src/fretboard.rs` (挿入する記法、注記の削除)

- [ ] **Step 1: `reproduces_shape` を使っているテストを直す**

`chord_midi/src/fingering/identify.rs` の `mod tests` で
`reproduces_shape` を見ている3箇所 (`assert!(c[0].reproduces_shape);` ×2 と
`assert!(!c[0].reproduces_shape);`) を消す。その行だけを削除し、
同じテストの他のアサーションは残す。

- [ ] **Step 2: `Candidate` から公開フィールドを落とす**

`chord_midi/src/fingering/identify.rs`:

```rust
/// 逆引きの結果1件
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    /// rechord のコード名。"C" や "C/E"
    pub name: String,
    /// そのまま入力に書ける記法。"C/E@6" や "pow(A@6)"
    pub notation: String,
    pub source: Source,
    /// 小さいほど良い。表示順に使う
    pub score: u32,
}
```

`identify` の中の `Candidate { ... }` を組む2箇所から `reproduces_shape` の
行を消す。`notation_for` が返す `bool` は `Source` の判定に使い続けるので
残す (`let (notation, reproduces) = notation_for(...)` はそのまま)。
`movable_candidate` からも `reproduces_shape: true,` の行を消す。

- [ ] **Step 3: ビルドとテストを通す**

Run: `cargo test -q -p chord_midi 2>&1 | tail -5`
Expected: すべて PASS

- [ ] **Step 4: 指板パネルの挿入を tab にする**

`chord_midi_web/src/fretboard.rs` の `fretboard` 関数の本体で、
`candidate_cards` に `props.frets` を渡せるようにする:

```rust
            <div class="block">
                <span class="block-label">{"コードの候補 (カーソル位置に挿入)"}</span>
                { candidate_cards(&candidates, props.frets, &props.on_edit, all_muted) }
            </div>
```

`candidate_cards` の頭:

```rust
fn candidate_cards(
    candidates: &[Candidate],
    frets: [Option<u8>; 6],
    on_edit: &Callback<Edit>,
    all_muted: bool,
) -> Html {
```

挿入する記法を tab に変える。`candidates.iter().map(...)` の中の
`onclick` と `note` を次に置き換える:

```rust
            let onclick = {
                let on_edit = on_edit.clone();
                // 押さえた形をそのまま入れる。コード名を経由すると
                // `fingering::find` の近似を通って別の形に化けることがある
                let edit = Edit::Insert(format!("{} |", tab_notation(frets)));
                Callback::from(move |_| on_edit.emit(edit.clone()))
            };
```

`note` を作っている `let note = (!c.reproduces_shape).then(|| { ... });` の
ブロックごと消し、`html!` の中の `{note}` も消す。

`fretboard.rs` の末尾に:

```rust
/// 押さえ方を `tab(...)` の記法にする。6弦→1弦、ミュートは `x`
fn tab_notation(frets: [Option<u8>; 6]) -> String {
    let body = frets
        .iter()
        .map(|fret| match fret {
            Some(n) => n.to_string(),
            None => "x".to_string(),
        })
        .collect::<Vec<_>>()
        .join(" ");
    format!("tab({})", body)
}
```

カードに出している `<code class="fretboard-notation">{&c.notation}</code>` は
そのまま残す (そのコードの書き方を見せる役)。

- [ ] **Step 5: web がコンパイルできることを確かめる**

Run: `cd /Users/kmt/dev/chord-midi/chord_midi_web && cargo check --target wasm32-unknown-unknown 2>&1 | tail -5`
Expected: エラー無し

- [ ] **Step 6: 画面で確かめる**

```bash
cd /Users/kmt/dev/chord-midi/chord_midi_web
trunk serve --port 8099 --no-autoreload
```
`http://127.0.0.1:8099/` で 6弦8 / 5弦6 / 4弦6 を押し、候補の「挿入」を押す。
Expected: エディタに `tab(8 6 6 x x x) |` が入る。プレビュータブの図が
指板で押さえた形と**同じ**になる。「再現できない」の注記はどのカードにも
出ない。確認したら `Ctrl-C` で止める。

- [ ] **Step 7: 整形してコミット**

```bash
cd /Users/kmt/dev/chord-midi
cargo fmt --all
(cd chord_midi_web && cargo fmt --all)
cargo test -q --workspace 2>&1 | tail -3
git add -A
git commit -m "$(cat <<'MSG'
feat(web): 指板パネルの挿入を tab(...) にする

押さえた形がそのまま戻るようになったので、Candidate の
reproduces_shape と「この形は今の押さえ方データでは再現できない」の
注記を消す。コード名の見出しとバッジは残す。

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01F28qZZi3uLGGKV2azgKPX8
MSG
)"
```

---

### Task 7: S式インポータ

**Files:**
- Modify: `chord_midi/src/import/sexp.rs` (`parse_node`)

- [ ] **Step 1: 失敗するテストを書く**

`chord_midi/src/import/sexp.rs` の `mod tests` の末尾に:

```rust
    #[test]
    fn test_tab_node() {
        use super::parse_node;
        use symbolic_expressions::Sexp;
        let sexp = Sexp::List(vec![
            Sexp::String("tab".into()),
            Sexp::String("8".into()),
            Sexp::String("6".into()),
            Sexp::String("6".into()),
            Sexp::String("x".into()),
            Sexp::String("x".into()),
            Sexp::String("x".into()),
        ]);
        assert_eq!(
            parse_node(&sexp).unwrap(),
            Node::Tab([Some(8), Some(6), Some(6), None, None, None], false)
        );
    }
```

`mod tests` は `chord_midi/src/import/sexp.rs:102` にあり、`Node` は既に
import 済み。`parse_node` は同じファイルの private 関数なので
`use super::parse_node;` で呼べる。

- [ ] **Step 2: テストを走らせて落ちることを確かめる**

Run: `cargo test -q -p chord_midi test_tab_node 2>&1 | tail -10`
Expected: FAIL (`invalid key: tab` などのエラー)

- [ ] **Step 3: `(tab ...)` を読む**

`chord_midi/src/import/sexp.rs` の `parse_node` の
`Sexp::List(list) if starts_with(&sexp, "chord") => {` の前に:

```rust
        Sexp::List(list) if starts_with(&sexp, "tab") => {
            let frets = list[1..]
                .iter()
                .map(|s| {
                    let token = s.string()?;
                    if token == "x" {
                        Ok(None)
                    } else {
                        Ok(Some(token.parse::<u8>()?))
                    }
                })
                .collect::<Result<Vec<Option<u8>>>>()?;
            let len = frets.len();
            let frets: [Option<u8>; 6] = frets
                .try_into()
                .map_err(|_| anyhow::anyhow!("tab は6弦ぶん書くこと: {} 本だった", len))?;
            Ok(Node::Tab(frets, false))
        }
```

`Sexp::string()` は `Result<&String>` を返す (同じファイルの
`list[1].string()?` と同じ使い方)。

- [ ] **Step 4: テストが通ることを確かめる**

Run: `cargo test -q -p chord_midi test_tab_node 2>&1 | tail -5`
Expected: PASS

- [ ] **Step 5: 整形してコミット**

```bash
cd /Users/kmt/dev/chord-midi
cargo fmt --all
cargo test -q -p chord_midi 2>&1 | tail -3
git add -A
git commit -m "$(cat <<'MSG'
feat: S式でも (tab 8 6 6 x x x) を読む

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01F28qZZi3uLGGKV2azgKPX8
MSG
)"
```

---

### Task 8: 記法表・README・golden

**Files:**
- Modify: `chord_midi/src/reference.rs` (記法表)
- Modify: `chord_midi/tests/golden.rs` (`SAMPLES`)
- Create: `chord_midi/tests/golden/tab.mid`
- Modify: `README.md`

- [ ] **Step 1: 記法表に足す**

`chord_midi/src/reference.rs` の `FUNCTIONS` の配列に、`pat` の次の要素として:

```rust
    Function {
        name: "tab",
        args: "6弦→1弦のフレット",
        scope: Scope::Chord,
        summary: "押さえ方を直接書く (x = ミュート)",
        example: "tab(8 6 6 x x x)",
    },
```

`Scope::Chord` にするのは、表の「コードが書けるところに置ける」群に
並べるため。`tab` は `ChordExpr` を取らないので入れ子にはできないが、
置ける場所は `Scope::Chord` の群と同じ。

- [ ] **Step 2: 記法表のテストを確かめる**

Run: `cargo test -q -p chord_midi reference 2>&1 | tail -5 && cargo test -q -p chord_midi_cli 2>&1 | tail -5`
Expected: PASS。`chord_midi/src/reference.rs` の `every_example_parses` が
`tab(8 6 6 x x x)` をパースできることを見る (Task 1 で通るようになっている)。
`help_syntax.rs` は "pow" "pat" などの有無しか見ないので影響しない

- [ ] **Step 3: 記法表の見た目を確かめる**

Run: `cargo run -q -p chord_midi_cli -- --help-syntax | grep tab`
Expected: `tab(6弦→1弦のフレット)` の行が出て、桁がずれていない

- [ ] **Step 4: golden にサンプルを足す**

`chord_midi/tests/golden.rs` の `SAMPLES` の末尾に:

```rust
    // 押さえ方を直接書く記法。コード名を介さないので find を通らない
    ("tab", "tab(0 2 2 1 0 0) | tab(x 3 2 0 1 x) |\n"),
    ("tab_pat", "pat(\"oxox\", tab(0 2 2 1 0 0))\n"),
```

- [ ] **Step 5: golden を生成する**

```bash
cd /Users/kmt/dev/chord-midi
UPDATE_GOLDEN=1 cargo test -q -p chord_midi --test golden 2>&1 | tail -3
git status --short chord_midi/tests/golden/
```
Expected: `?? chord_midi/tests/golden/tab.mid` と `?? .../tab_pat.mid` の2件だけ。
既存の `.mid` が `M` になっていたら中断して原因を調べる (tab は既存の
経路を変えないので、他が変わるのはバグ)

- [ ] **Step 6: golden が安定することを確かめる**

Run: `cargo test -q -p chord_midi --test golden 2>&1 | tail -3`
Expected: PASS

- [ ] **Step 7: README に節を足す**

`README.md` の「## 逆引き（指板）」の節の直前に:

```markdown
## 押さえ方を直接書く（tab）

コード名ではなく押さえ方そのものを書ける。

    tab(8 6 6 x x x) |

- 6弦→1弦の順でちょうど6本。`x` はミュート、`0` は開放弦
- フレットは 0〜24
- MIDI は押さえた弦の実音をそのまま鳴らす。コード名を介さないので、
  押さえ方データの近似を通らない
- プレビューの見出しは逆引きした候補名。該当が無ければ記法がそのまま出る
- キーを上げ下げすると全弦が同じ数だけ動く。開放弦を含む形は下げられない
- `pow` `oct` `arp` `blk` `s25` `sub` `dom` は tab に掛けられない。
  コード名を持たないため。`pat` は掛けられる

指板タブの「挿入」が入れるのもこの記法で、押さえた形がそのまま戻る。
```

同じ節の中の「- 「挿入」を押すと、その記法が `<記法> |` の形でカーソル位置に入る」を
次に直す:

```markdown
- 「挿入」を押すと、`tab(...)` の形でカーソル位置に入る。押さえた形が
  そのまま戻るので、図と指板が食い違わない
```

- [ ] **Step 8: 整形してコミット**

```bash
cd /Users/kmt/dev/chord-midi
cargo fmt --all
cargo test -q --workspace 2>&1 | tail -3
git add -A
git commit -m "$(cat <<'MSG'
docs: tab 記法を記法表と README に載せ、golden を足す

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01F28qZZi3uLGGKV2azgKPX8
MSG
)"
```

---

## 仕上げ

- [ ] **Step 1: 全部通す**

```bash
cd /Users/kmt/dev/chord-midi
cargo fmt --all --check
cargo test -q --workspace 2>&1 | tail -3
cargo clippy -q --workspace --all-targets 2>&1 | tail -5
(cd chord_midi_web && cargo fmt --all --check && cargo check -q --target wasm32-unknown-unknown)
```
Expected: 整形済み、テスト全通過、clippy の新規警告なし
(`chord_midi/src/import/chord.rs:217` の `casting to the same type` などは既存)

- [ ] **Step 2: 画面で通しで確かめる**

```bash
cd /Users/kmt/dev/chord-midi/chord_midi_web
trunk serve --port 8099 --no-autoreload
```

`http://127.0.0.1:8099/` で:
1. 指板で 6弦8 / 5弦6 / 4弦6 を押す
2. 候補の「挿入」→ エディタに `tab(8 6 6 x x x) |` が入る
3. プレビュータブの図が指板と同じ形になる
4. `♯` を押すと `tab(9 7 7 x x x) |` になる
5. 開放弦を含む形 (`tab(0 2 2 1 0 0) |`) で `♭` を押すとエラー表示が出る
6. 再生すると押さえた音が鳴る

確認したら `Ctrl-C` で止める。

- [ ] **Step 3: PR を作る**

```bash
cd /Users/kmt/dev/chord-midi
git push -u origin feat/tab-notation
gh pr create --base main --title "feat: 押さえ方を直接書く tab(...) 記法" --body "設計書: docs/superpowers/specs/2026-10-18-tab-notation-design.md"
```
