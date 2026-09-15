# ノードに掛かる関数の型付けと入れ子 実装計画

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `arp(blk(I))` を書けるようにする。ノードに掛かる関数を型で分け、入れ子の可否を型から落とす。

**Architecture:** `ChordExpr`(コードを返す式)と `ChordMap`(Chord→Chord の `blk`/`sub`/`dom`)を足し、`Node` を `Chord`/`Arpeggio`/`Brush`/`Split` が `ChordExpr` を持つ形に作り替える。`arp` と `s25` は `Node` を返すので `ChordExpr` の位置に置けず、`arp(s25(V7))` などはパーサが組み立てられない = パースエラーになる。

**Tech Stack:** Rust 2021, nom 7 (パーサ), serde (AST 表示), yew 0.21 (web)

**設計書:** `docs/superpowers/specs/2026-09-16-chord-expr-design.md`

---

## 引き継ぐ約束

1. **ゴールデン MIDI (`chord_midi/tests/golden/*.mid`) を1バイトも変えない。** 記法も MIDI も変えない変更なので、1件でも動いたら壊している
2. **既存テストのアサーションの意味を変えない。** 型変更に伴う機械的な書き換えは可。期待値を変えたくなったら手を止めて報告する
3. **個別タスクで `cargo fmt` を走らせない。** 最後にまとめて当てる。ただし CI に `fmt --check` があるので `rustfmt --check` (読み取り専用) で自分の追加分は整形済みにしておく
4. **`match` に catch-all (`_ =>`) を置かない。** Phase 1/2 で catch-all が実バグを2件隠していた実例がある
5. **macOS の BSD sed は `\b` を解釈せず、エラーなしで0件置換になる。** 一括置換は `perl -pi -e`
6. `chord_midi_web` は workspace から exclude。チェックは `(cd chord_midi_web && cargo check --target wasm32-unknown-unknown)`、ビルドは `trunk build`
7. **計画書のコードがそのままでは動かない可能性がある。** Phase 1/2/3 で8回あった。動かなければ直し、直した内容と理由を報告する。**計画書が間違っていると感じたら、実装を歪めるのではなく報告する**

---

## 実測で確定している等価 (テストに使う)

```
blk(F)    ≡ Baug/F     (バイト同一を確認済み)
sub(G7)   ≡ Db7
dom(Dm7)  ≡ D7
```

入れ子のテストは**中の変換を既知の等価形に置き換えて比べる**。最終的なコードを計算する必要がない。

---

## 現在の型 (作り替える前)

```rust
pub enum Node {
    Chord(ChordNode),
    Arpeggio(ChordNode),
    Transform(Transform, ChordNode),
    Brush(ChordNode),
    Rest, Sustain, Repeat,
}

pub enum Transform { S25, Sub, Dom, Blk }

impl Transform {
    pub fn expand(self, c: ChordNode, tonic: Pitch, scale: Scale) -> Vec<Node> { .. }
}
```

`Transform::expand` は4つとも `Vec<Node>` を返す。`S25` だけ2要素、他は1要素。

---

## 依存関係

```
Task 1 (型 + パーサ) -> Task 2 (Display) -> Task 3 (テスト) -> Task 4 (記法表・README・整形)
```

**全部直列。** 型を作り替えるので同じファイルを積み上げる。

---

## Task 1: `ChordExpr` / `ChordMap` を足し、`Node` を作り替える

型とパーサは一体。片方だけではコンパイルが通らない。

**Files:**
- Modify: `chord_midi/src/model/ast.rs`
- Modify: `chord_midi/src/import/chord.rs`
- Modify: `chord_midi/src/model/transform.rs`
- Modify: `chord_midi/src/export/midi.rs`
- Modify: `chord_midi/src/fingering/mod.rs`
- Modify: `chord_midi/src/edit.rs`
- Modify: `chord_midi/src/export/rechord.rs` (暫定。正式化は Task 2)

- [ ] **Step 1: 失敗するテストを書く**

`chord_midi/src/import/chord.rs` の `mod tests` に足す (無ければ作る):

```rust
    #[test]
    fn test_arp_can_wrap_a_transform() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::{ChordExpr, ChordMap, Expr, Node};
        let ast = RechordImporter.import("key: C\narp(blk(I)) |\n").unwrap();
        fn first_node(e: &Expr) -> Option<&Node> {
            match e {
                Expr::Measure(nodes, _) => nodes.first(),
                Expr::Seq(items) => items.iter().find_map(first_node),
                Expr::Keyed(_, inner) | Expr::Scaled(_, inner) => first_node(inner),
                _ => None,
            }
        }
        let Some(Node::Arpeggio(expr)) = first_node(&ast) else {
            panic!("arp にならない: {:?}", ast)
        };
        assert!(
            matches!(expr, ChordExpr::Map(ChordMap::Blk, _)),
            "中身が blk の Map にならない: {:?}", expr
        );
    }

    #[test]
    fn test_transforms_can_nest() {
        use crate::import::{Importer, RechordImporter};
        assert!(RechordImporter.import("blk(sub(G7)) |\n").is_ok());
        assert!(RechordImporter.import("arp(blk(sub(G7))) |\n").is_ok());
        assert!(RechordImporter.import("s25(blk(C)) |\n").is_ok());
    }

    #[test]
    fn test_node_returning_functions_cannot_nest() {
        use crate::import::{Importer, RechordImporter};
        // arp と s25 は Node を返すので ChordExpr の位置に置けない
        assert!(RechordImporter.import("arp(s25(G7)) |\n").is_err());
        assert!(RechordImporter.import("blk(arp(C)) |\n").is_err());
        assert!(RechordImporter.import("s25(s25(G7)) |\n").is_err());
        assert!(RechordImporter.import("blk(s25(G7)) |\n").is_err());
        assert!(RechordImporter.import("arp(arp(C)) |\n").is_err());
    }
```

- [ ] **Step 2: 落ちることを確認**

```bash
cargo test -p chord_midi --lib test_arp_can_wrap_a_transform
```

Expected: FAIL (`ChordExpr` が無い)

- [ ] **Step 3: 型を足す**

`chord_midi/src/model/ast.rs`:

```rust
/// コードを返す式。入れ子にできる
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ChordExpr {
    /// 素のコード
    Chord(ChordNode),
    /// コード1つを別のコード1つに変える
    Map(ChordMap, Box<ChordExpr>),
}

/// `Chord -> Chord` の変換
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ChordMap {
    /// ブラックアダーコード
    Blk,
    /// 裏コード化
    Sub,
    /// ドミナント化
    Dom,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Node {
    /// 鳴らす
    Chord(ChordExpr),
    /// アルペジオで鳴らす
    Arpeggio(ChordExpr),
    /// ブラッシング。`pat` の `x` から作られる (パーサは作らない)
    Brush(ChordExpr),
    /// ツーファイブ分割。自分の尺を2つに割る
    Split(ChordExpr),
    Rest,
    Sustain,
    Repeat,
}
```

`Transform` enum は廃止する。

- [ ] **Step 4: `ChordExpr` の評価と `Split` の展開を書く**

`chord_midi/src/model/ast.rs` (今の `Transform::expand` があった場所の近く):

```rust
impl ChordMap {
    /// コード1つを別のコード1つに変える。
    /// `tonic`/`scale` は Relative なキーをずらすためのもので、
    /// `resolve()` 済み (= Absolute) なら無視される
    pub fn apply(self, c: ChordNode, tonic: Pitch, scale: Scale) -> ChordNode {
        match self {
            // sub/dom は新しいドミナントを作るので、元の分数コードのベース (`on`) は捨てる
            ChordMap::Sub => transform_chord(
                c.key.shift(6, tonic, scale),
                &[Modifier::Dominant(7)],
                None,
            ),
            ChordMap::Dom => transform_chord(c.key, &[Modifier::Dominant(7)], None),
            ChordMap::Blk => transform_chord(
                c.key.clone().shift(6, tonic, scale),
                &[Modifier::Aug],
                Some(c.key),
            ),
        }
    }
}

impl ChordExpr {
    /// 入れ子を内側から畳んで1つのコードにする
    pub fn eval(&self, tonic: Pitch, scale: Scale) -> ChordNode {
        match self {
            ChordExpr::Chord(c) => c.clone(),
            ChordExpr::Map(m, inner) => m.apply(inner.eval(tonic, scale), tonic, scale),
        }
    }
}
```

`Split` の展開 (今の `Transform::S25` の腕):

```rust
impl Node {
    /// `s25` を2つのコードに展開する。呼ぶ側が尺を2等分する
    pub fn split_into(expr: &ChordExpr, tonic: Pitch, scale: Scale) -> Vec<ChordNode> {
        let c = expr.eval(tonic, scale);
        let ii = transform_chord(
            c.key.clone().shift(7, tonic, scale),
            &[Modifier::Minor(7)],
            None,
        );
        vec![ii, c]
    }
}
```

**注意**: 今の `Transform::expand` の `S25` の腕を読んで、`shift(7, ..)` と修飾の付け方をそのまま写すこと。ここを写し損なうとゴールデンが動く。

- [ ] **Step 5: パーサを再帰にする**

`chord_midi/src/import/chord.rs`:

```rust
/// `blk(` `sub(` `dom(` の入れ子。中身は `ChordExpr`
fn chord_expr_parser(s: Span) -> IResult<Span, ChordExpr> {
    alt((
        map(
            delimited(tag("blk("), chord_expr_parser, tag(")")),
            |e| ChordExpr::Map(ChordMap::Blk, Box::new(e)),
        ),
        map(
            delimited(tag("sub("), chord_expr_parser, tag(")")),
            |e| ChordExpr::Map(ChordMap::Sub, Box::new(e)),
        ),
        map(
            delimited(tag("dom("), chord_expr_parser, tag(")")),
            |e| ChordExpr::Map(ChordMap::Dom, Box::new(e)),
        ),
        map(chord_node_parser, ChordExpr::Chord),
    ))(s)
}

pub fn node_parser(s: Span) -> IResult<Span, Node> {
    alt((
        map(tag("="), |_| Node::Sustain),
        map(tag("_"), |_| Node::Rest),
        map(tag("%"), |_| Node::Repeat),
        map(tag("N.C."), |_| Node::Rest),
        // arp と s25 は Node を返すので、chord_expr_parser の中には現れない
        map(
            delimited(tag("arp("), chord_expr_parser, tag(")")),
            Node::Arpeggio,
        ),
        map(
            delimited(tag("s25("), chord_expr_parser, tag(")")),
            Node::Split,
        ),
        map(chord_expr_parser, Node::Chord),
    ))(s)
}
```

**`chord_expr_parser` は再帰する。** `blk(` を必ず消費してから再帰するので無限再帰にはならないが、確認すること。

- [ ] **Step 6: 下流をコンパイラに拾わせる**

```bash
cargo check --workspace --all-targets 2>&1 | head -40
```

直す箇所:

`chord_midi/src/export/midi.rs` の `interpret_node`:

```rust
            Node::Chord(e) => {
                let c = e.eval(Pitch::C, Scale::Major);
                self.pre = Some(voicing(&c)?);
                self.sustain = dur;
            }
            Node::Arpeggio(e) => {
                let c = e.eval(Pitch::C, Scale::Major);
                let all = voicing(&c)?;
                // (今のアルペジオの尺の配り方をそのまま写すこと)
            }
            Node::Split(e) => {
                let chords = Node::split_into(e, Pitch::C, Scale::Major);
                // (今の Transform の腕の尺の配り方をそのまま写すこと)
            }
            Node::Brush(e) => {
                let c = e.eval(Pitch::C, Scale::Major);
                let notes = voicing(&c)?;
                self.pre = Some(notes.clone());
                self.notes.push(Note::brushed(Some(notes), dur));
            }
```

`interpret_patterned_measure` の事前展開 (直前のコミットで入れたもの) は `Node::Transform` を見ているので `Node::Split` に直すこと。

`chord_midi/src/fingering/mod.rs` の `collect_node`:

```rust
        Node::Chord(e) | Node::Arpeggio(e) | Node::Brush(e) => {
            out.push(chord_ref(&e.eval(Pitch::C, Scale::Major))?)
        }
        Node::Split(e) => {
            for c in Node::split_into(e, Pitch::C, Scale::Major) {
                out.push(chord_ref(&c)?);
            }
        }
        Node::Rest | Node::Sustain | Node::Repeat => {}
```

`chord_midi/src/model/transform.rs` の `into_pitch` / `into_degree` / `mark_node`: `ChordNode` を直接持たなくなったので `ChordExpr` へ潜る関数を足すこと。

`chord_midi/src/edit.rs` の `shift_node`: 同じく `ChordExpr` へ潜る。

`chord_midi/src/export/rechord.rs`: Task 2 で正式化するので、いったん通る形にする。

- [ ] **Step 7: 通ることを確認**

```bash
cargo test -p chord_midi --lib import::chord
cargo test --workspace
```

- [ ] **Step 8: ゴールデンが無変更であることを確認**

```bash
git diff --stat HEAD -- chord_midi/tests/golden/
```

Expected: **空。1件でも動いたら `ChordMap::apply` か `split_into` の写し損ない。**

- [ ] **Step 9: Commit**

```
feat: ノードに掛かる関数を ChordExpr と ChordMap で型付けする
```

---

## Task 2: Display と round-trip

**Files:**
- Modify: `chord_midi/src/export/rechord.rs`

- [ ] **Step 1: 失敗するテストを書く**

`chord_midi/src/export/rechord.rs` の `mod tests` に足す (`roundtrip` ヘルパは既にある):

```rust
    #[test]
    fn test_nested_transform_roundtrip() {
        assert_eq!(roundtrip("arp(blk(C)) |"), "arp(blk(C)) |");
        assert_eq!(roundtrip("blk(sub(G7)) |"), "blk(sub(G7)) |");
        assert_eq!(roundtrip("arp(blk(sub(G7))) |"), "arp(blk(sub(G7))) |");
        assert_eq!(roundtrip("s25(blk(C)) |"), "s25(blk(C)) |");
    }

    #[test]
    fn test_flat_transform_roundtrip_unchanged() {
        // 既存の書き方が変わらないこと
        assert_eq!(roundtrip("arp(C) |"), "arp(C) |");
        assert_eq!(roundtrip("blk(F) |"), "blk(F) |");
        assert_eq!(roundtrip("s25(G7) |"), "s25(G7) |");
        assert_eq!(roundtrip("sub(G7) |"), "sub(G7) |");
        assert_eq!(roundtrip("dom(Dm7) |"), "dom(Dm7) |");
    }
```

- [ ] **Step 2: 落ちることを確認**

- [ ] **Step 3: Display を書く**

```rust
impl Display for ChordExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChordExpr::Chord(c) => write!(f, "{}", c),
            ChordExpr::Map(m, inner) => write!(f, "{}({})", m.tag(), inner),
        }
    }
}

impl ChordMap {
    fn tag(self) -> &'static str {
        match self {
            ChordMap::Blk => "blk",
            ChordMap::Sub => "sub",
            ChordMap::Dom => "dom",
        }
    }
}
```

`Node` の Display:

```rust
            Node::Chord(e) => write!(f, "{}", e),
            Node::Brush(e) => write!(f, "{}", e),
            Node::Arpeggio(e) => write!(f, "arp({})", e),
            Node::Split(e) => write!(f, "s25({})", e),
```

**`Node::Brush` が中身をそのまま出すのは既存の挙動** (ブラッシングは `pat` が作るもので記法には無い)。変えないこと。

- [ ] **Step 4〜6: 検証とコミット**

```bash
cargo test --workspace && git diff --stat HEAD -- chord_midi/tests/golden/
```

```
feat: 入れ子の変換を書き戻せるようにする
```

---

## Task 3: 等価テストとゴールデン

**Files:**
- Modify: `chord_midi/tests/golden.rs`

- [ ] **Step 1: 等価テストを書く**

```rust
/// 入れ子の変換が、中を手で置き換えた形と同じ MIDI を出すこと。
///
/// `blk(F) ≡ Baug/F` / `sub(G7) ≡ Db7` / `dom(Dm7) ≡ D7` は実測で確認済み。
/// 入れ子は「中の変換を既知の等価形に置き換えたもの」と一致するはず。
#[test]
fn nested_transforms_match_hand_substitution() {
    let pairs: &[(&str, &str, &str)] = &[
        ("arp_blk", "arp(blk(F))\n", "arp(Baug/F)\n"),
        ("arp_sub", "arp(sub(G7))\n", "arp(Db7)\n"),
        ("arp_dom", "arp(dom(Dm7))\n", "arp(D7)\n"),
        ("blk_sub", "blk(sub(G7))\n", "blk(Db7)\n"),
        ("arp_blk_sub", "arp(blk(sub(G7)))\n", "arp(blk(Db7))\n"),
        ("s25_blk", "s25(blk(F))\n", "s25(Baug/F)\n"),
        ("pat_arp_blk", "pat(\"o.ox\", arp(blk(F)))\n", "pat(\"o.ox\", arp(Baug/F))\n"),
    ];
    let mut failures = Vec::new();
    for (name, nested, substituted) in pairs {
        match (midi_bytes(name, nested), midi_bytes(name, substituted)) {
            (Ok(a), Ok(b)) if a == b => {}
            (Ok(a), Ok(b)) => failures.push(format!(
                "[{}] 入れ子と手で置き換えた形の MIDI が違う ({} bytes vs {} bytes)",
                name, a.len(), b.len()
            )),
            (Err(e), _) | (_, Err(e)) => failures.push(format!("[{}] {}", name, e)),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
```

**このテストが Phase 4 の中核の検証。** 一致しなかったら `ChordExpr::eval` の畳み方が間違っている。

- [ ] **Step 2: ゴールデンを足す**

`SAMPLES` に:

```rust
    // 入れ子の変換
    ("nested_arp_blk", "arp(blk(F))\n"),
    ("nested_blk_sub", "blk(sub(G7))\n"),
    ("nested_s25_blk", "s25(blk(F))\n"),
```

```bash
UPDATE_GOLDEN=1 cargo test -p chord_midi --test golden
git status --short chord_midi/tests/golden/
```

Expected: **新規追加 (`??`) だけ。既存に `M` が付いたら後方互換の破壊。**

- [ ] **Step 3: 検証とコミット**

```
test: 入れ子の変換が手で置き換えた形と同じ MIDI を出すことを固定する
```

---

## Task 4: 記法表・README・整形

**Files:**
- Modify: `chord_midi/src/reference.rs`
- Modify: `README.md`
- 全体 (整形)

- [ ] **Step 1: 記法表に入れ子を書く**

`chord_midi/src/reference.rs` の `FUNCTIONS` の説明か、`SYMBOLS` の後ろに入れ子の説明を足すこと。**`every_example_parses` が通る形で書くこと** (例は実際にパースできるもの)。

`Scope` の見せ方も考えること: `blk`/`sub`/`dom` は「コード → コード」、`s25` は「コード → 尺を2分割」、`arp` は「コード → 演奏法」。今の `Scope { Measures, Chord }` では3種類を表せない。**`Scope` を増やすか、説明文で書くかを判断して報告すること。**

- [ ] **Step 2: README に書く**

`## 関数` の節に入れ子のことを足す。**書ける組み合わせと書けない組み合わせを表で出すこと。**

**README に載せた例は実際に CLI に通して確認すること。**

- [ ] **Step 3: 整形**

```bash
cargo fmt --all
(cd chord_midi_web && cargo fmt)
```

差分が整形だけであることを、空白を落とした比較で確認すること:

```bash
for f in $(git diff --name-only); do
  a=$(git show HEAD:$f | tr -d '[:space:]' | shasum | cut -d' ' -f1)
  b=$(tr -d '[:space:]' < $f | shasum | cut -d' ' -f1)
  [ "$a" = "$b" ] && echo "OK  $f" || echo "DIFF $f <-- 要確認"
done
```

`DIFF` が出たら、変わった文字が `{`/`}`/`,` だけかを確認すること (rustfmt は末尾カンマとブロックを足し引きする)。

- [ ] **Step 4: 最終確認**

```bash
cargo test --workspace
cargo fmt --all -- --check
(cd chord_midi_web && cargo fmt -- --check && trunk build)
git diff --stat main -- chord_midi/tests/golden/
```

**ゴールデンは新規追加のみで、既存に `M` が付かないこと。**

- [ ] **Step 5: Commit** (記法表/README と整形で別コミット)

---

## 完了条件

- `cargo test --workspace` が通る
- **既存のゴールデン27件が1バイトも変わっていない** (追加のみ)
- `nested_transforms_match_hand_substitution` が通る
- `arp(s25(V7))` / `blk(arp(I))` / `s25(s25(V7))` / `blk(s25(V7))` / `arp(arp(C))` がパースエラー
- `cargo fmt --check` root / web 両方、`trunk build` が通る
- README の例が動く
