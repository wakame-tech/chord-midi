# Issue #12 フェーズ2: コード変換関数 実装計画

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** `blk(X)` / `s25(X)` / `sub(X)` / `dom(X)` の関数記法を追加する。`arp(...)` と同じくノード拡張とし、MIDI interpret 時に標準コード列へ展開する。

**Architecture:** `Node::Transform(Transform, ChordNode)` を追加。`into_pitch/into_degree` は内部 ChordNode を transpose。MIDI の `interpret_node` が `Transform::expand` で標準 `Node::Chord` 列へ展開し、各々を再帰処理する(`resolve` はフェーズ1のまま=into_pitch+ディレクティブ除去)。Display は `blk(...)` 等で記法保持。

**Tech Stack:** Rust, nom, midi_file. 対象 `chord_midi`。base ブランチ `feat/issue-12-presets-transforms`(フェーズ1)。

参照 spec: `docs/superpowers/specs/2026-07-08-progression-presets-and-transforms-design.md` の A-5/A-6。

**変換定義(確定):**
- `blk(X)` = `(X.root+6半音)aug / X` (aug トライアド[トライトーン上] on X ベース)。blk(F)=Baug/F={F,B,D#,G}
- `s25(X)` = `(X.root+7半音)m7` を前置し X を残す。s25(G7)=Dm7 G7
- `sub(X)` = `(X.root+6半音)dom7`。sub(G7)=Db7
- `dom(X)` = X のルートで dom7(X の他 modifier は破棄)。dom(Dm7)=D7

**テスト出力確認:** `cargo test -p chord_midi > /tmp/t.txt 2>&1; grep "test result" /tmp/t.txt`。

---

## Task 1: `Node::Transform` + Key::shift + expand + 全 match 対応

新バリアント追加で match が壊れるため、このタスクで型・パーサ・Display・into_pitch/degree・interpret・展開まで一括で通す。

**Files:**
- Modify: `chord_midi/src/model/ast.rs`
- Modify: `chord_midi/src/model/key.rs`
- Modify: `chord_midi/src/import/chord.rs`
- Modify: `chord_midi/src/export/rechord.rs`
- Modify: `chord_midi/src/model/transform.rs`
- Modify: `chord_midi/src/export/midi.rs`

- [ ] **Step 1: 失敗するパーサテストを書く**

`chord_midi/src/import/chord.rs` テストに追加:

```rust
    #[test]
    fn test_transform_parser() {
        for (code, ok) in [
            ("blk(F)", true),
            ("s25(G7)", true),
            ("sub(G7)", true),
            ("dom(Dm7)", true),
        ] {
            let (rest, node) = node_parser(span(code)).unwrap();
            assert_eq!(rest.into_fragment(), "", "{}", code);
            assert!(matches!(node, crate::model::ast::Node::Transform(_, _)) == ok, "{}", code);
        }
    }
```

- [ ] **Step 2: 失敗を確認**

Run: `cargo test -p chord_midi test_transform_parser` — Expected: コンパイルエラー(`Node::Transform` なし)

- [ ] **Step 3: `ast.rs` に Transform / Node::Transform / expand を追加**

`chord_midi/src/model/ast.rs`:

```rust
use crate::model::key::Key; // 既にあり
// Modifier, Pitch, Scale, Degree も既に import 済み

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transform {
    S25,
    Sub,
    Dom,
    Blk,
}

#[derive(Debug, PartialEq)]
pub enum Node {
    Chord(ChordNode),
    Arpeggio(ChordNode),
    Transform(Transform, ChordNode),
    Rest,
    Sustain,
    Repeat,
}
```

`ChordNode` を作るヘルパ(デフォルト `Major(5)` 注入)と `Transform::expand` / `Node::expand` を追加:

```rust
fn transform_chord(key: Key, extra: &[Modifier], on: Option<Key>) -> ChordNode {
    let mut modifiers = BTreeSet::new();
    modifiers.insert(Modifier::Major(5));
    for m in extra {
        modifiers.insert(m.clone());
    }
    ChordNode { key, modifiers, on }
}

impl Transform {
    /// 変換を標準 Node 列へ展開する。tonic/scale は Relative key の半音演算用
    /// (通常 into_pitch 後に呼ばれ key は Absolute なので tonic/scale は無視される)。
    pub fn expand(self, c: ChordNode, tonic: Pitch, scale: Scale) -> Vec<Node> {
        match self {
            // s25(X) = (X.root + P5)m7, then X unchanged
            Transform::S25 => {
                let ii = transform_chord(
                    c.key.clone().shift(7, tonic, scale),
                    &[Modifier::Minor(7)],
                    None,
                );
                vec![Node::Chord(ii), Node::Chord(c)]
            }
            // sub(X) = (X.root + tritone)dom7
            Transform::Sub => vec![Node::Chord(transform_chord(
                c.key.shift(6, tonic, scale),
                &[Modifier::Dominant(7)],
                None,
            ))],
            // dom(X) = (X.root)dom7  (X の他 modifier は破棄)
            Transform::Dom => vec![Node::Chord(transform_chord(
                c.key,
                &[Modifier::Dominant(7)],
                None,
            ))],
            // blk(X) = (X.root + tritone)aug / X
            Transform::Blk => vec![Node::Chord(transform_chord(
                c.key.clone().shift(6, tonic, scale),
                &[Modifier::Aug],
                Some(c.key),
            ))],
        }
    }
}
```

`use` に `Pitch`, `Scale` は既に import 済み。`BTreeSet`/`Modifier` も既に import 済み。

- [ ] **Step 4: `key.rs` に `shift` を追加**

`chord_midi/src/model/key.rs` の `impl Key`:

```rust
    /// 半音シフト。Absolute は Pitch を回し、Relative は scale で解決してシフトし度数へ戻す。
    pub fn shift(self, semitones: i8, _tonic: Pitch, scale: Scale) -> Key {
        match self {
            Key::Absolute(p) => {
                let n = (p as i8 + semitones).rem_euclid(12) as u8;
                Key::Absolute(Pitch::try_from(n).unwrap())
            }
            Key::Relative(Degree(d, acc)) => {
                let s = (scale.semitone(d) as i8 + i8::from(acc) + semitones).rem_euclid(12) as u8;
                Key::Relative(Degree::from_semitone(s))
            }
        }
    }
```

(`_tonic` は Absolute シフトには不要だが、シグネチャ一貫のため受ける。)

- [ ] **Step 5: パーサに4分岐を追加**

`chord_midi/src/import/chord.rs` の `node_parser`。`arp(` の後、`chord` 分岐の前に:

```rust
        map(delimited(tag("arp("), chord_node_parser, tag(")")), Node::Arpeggio),
        map(delimited(tag("blk("), chord_node_parser, tag(")")), |c| Node::Transform(Transform::Blk, c)),
        map(delimited(tag("s25("), chord_node_parser, tag(")")), |c| Node::Transform(Transform::S25, c)),
        map(delimited(tag("sub("), chord_node_parser, tag(")")), |c| Node::Transform(Transform::Sub, c)),
        map(delimited(tag("dom("), chord_node_parser, tag(")")), |c| Node::Transform(Transform::Dom, c)),
        map(chord_node_parser, Node::Chord),
```

`use crate::model::ast::{ChordNode, Node, Transform};`(`Transform` を追加)。

- [ ] **Step 6: Display に Transform を追加**

`chord_midi/src/export/rechord.rs`。`Display for Node` の match に追加:

```rust
            Node::Transform(t, c) => write!(f, "{}({})", transform_prefix(t), c),
```

`transform_prefix` を同ファイルに追加(または `Display for Transform` を ast.rs に置く)。ここでは関数:

```rust
fn transform_prefix(t: &crate::model::ast::Transform) -> &'static str {
    use crate::model::ast::Transform::*;
    match t {
        S25 => "s25",
        Sub => "sub",
        Dom => "dom",
        Blk => "blk",
    }
}
```

`use crate::model::ast::{Ast, ChordNode, Node, Directive};` に `Transform` は上記でフルパス参照するので追加不要(または追加してもよい)。

- [ ] **Step 7: `transform.rs` の into_pitch/into_degree に Transform arm**

`chord_midi/src/model/transform.rs` の `Ast::into_pitch` / `into_degree` の Measure map に、内部 ChordNode を transpose する arm を追加(Chord/Arpeggio と同様):

```rust
                    Node::Transform(t, c) => Node::Transform(t, chordnode_into_pitch(c, tonic, scale)),
```
(into_degree 側は `chordnode_into_degree`。`Transform` は `Copy` なので `t` はそのまま移動可。)

- [ ] **Step 8: MIDI interpret に Transform 展開 arm**

`chord_midi/src/export/midi.rs` の `interpret_node` の match に追加。展開して各サブノードを再帰処理(尺を等分):

```rust
            Node::Transform(t, c) => {
                let nodes = t.expand(c, crate::model::pitch::Pitch::C, crate::model::scale::Scale::Major);
                let k = nodes.len().max(1) as u32;
                let each = (dur / k).max(1);
                for n in nodes {
                    self.interpret_node(n, each)?;
                }
            }
```

注: `interpret_node` 冒頭の pending flush は Transform ノードでも走る(Transform は Sustain/Rest でないので pending sustain は正しく flush)。再帰呼び出しは各展開コードを通常 Chord として処理する。`t.expand` は `Transform` が `Copy` なので `t` を消費してよい。`c` は move。

- [ ] **Step 9: expand の構成音ユニットテスト**

`chord_midi/src/model/ast.rs` テストモジュール(無ければ作成)に、expand が正しい ChordNode を作るか検証:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::pitch::Pitch;
    use crate::model::scale::Scale;

    fn semis(node: &Node) -> Vec<u8> {
        match node {
            Node::Chord(c) => c.to_chord().unwrap().semitones.into_iter().collect(),
            _ => panic!("not a chord"),
        }
    }

    #[test]
    fn test_blk_expand() {
        // blk(F) = Baug/F : bass F, aug triad rooted B(=F+6). semitones from F {0,2,6,10}
        let f = ChordNode::absolute(Pitch::F);
        let nodes = Transform::Blk.expand(f, Pitch::C, Scale::Major);
        assert_eq!(nodes.len(), 1);
        assert_eq!(semis(&nodes[0]), vec![0, 2, 6, 10]);
    }

    #[test]
    fn test_sub_dom_expand() {
        // sub(G7) = Db7 {0,4,7,10} rooted Db ; dom(Dm7)=D7 {0,4,7,10} rooted D
        let g = ChordNode { key: Key::Absolute(Pitch::G), modifiers: std::collections::BTreeSet::from_iter([Modifier::Major(5), Modifier::Dominant(7)]), on: None };
        let subbed = Transform::Sub.expand(g.clone(), Pitch::C, Scale::Major);
        assert_eq!(subbed.len(), 1);
        assert!(matches!(&subbed[0], Node::Chord(c) if c.key == Key::Absolute(Pitch::Cs))); // Db == Cs
        assert_eq!(semis(&subbed[0]), vec![0, 4, 7, 10]);

        let dm7 = ChordNode { key: Key::Absolute(Pitch::D), modifiers: std::collections::BTreeSet::from_iter([Modifier::Major(5), Modifier::Minor(7)]), on: None };
        let dommed = Transform::Dom.expand(dm7, Pitch::C, Scale::Major);
        assert!(matches!(&dommed[0], Node::Chord(c) if c.key == Key::Absolute(Pitch::D)));
        assert_eq!(semis(&dommed[0]), vec![0, 4, 7, 10]);
    }

    #[test]
    fn test_s25_expand() {
        // s25(G7) = Dm7, G7 (two nodes)
        let g = ChordNode { key: Key::Absolute(Pitch::G), modifiers: std::collections::BTreeSet::from_iter([Modifier::Major(5), Modifier::Dominant(7)]), on: None };
        let nodes = Transform::S25.expand(g, Pitch::C, Scale::Major);
        assert_eq!(nodes.len(), 2);
        assert!(matches!(&nodes[0], Node::Chord(c) if c.key == Key::Absolute(Pitch::D))); // Dm7
        assert_eq!(semis(&nodes[0]), vec![0, 3, 7, 10]); // minor 7
        assert!(matches!(&nodes[1], Node::Chord(c) if c.key == Key::Absolute(Pitch::G))); // G7
    }
}
```

- [ ] **Step 10: 全体コンパイル & テスト**

Run: `cargo test -p chord_midi > /tmp/t.txt 2>&1; grep -E "test result|error" /tmp/t.txt`
Expected: 全 pass。コンパイルエラーがあれば残りの Node match 箇所(他に無いはず)を対応。

- [ ] **Step 11: コミット**

```bash
git add chord_midi/src
git commit -m "feat: add blk/s25/sub/dom chord-transform notation (Node::Transform)"
```

---

## Task 2: end-to-end 統合テスト(parse → resolve → MIDI)

**Files:**
- Modify: `chord_midi/src/export/midi.rs`(テスト追加)

- [ ] **Step 1: 失敗するかもしれない e2e テストを書く**

`chord_midi/src/export/midi.rs` テストに追加。`notes_of` ヘルパ(既存)を使い、変換を含む進行が MIDI ノードへ展開されることを確認:

```rust
    #[test]
    fn test_s25_produces_two_chords() {
        // s25(G7) は単小節で Dm7, G7 の2和音に展開される
        let notes = notes_of("s25(G7)");
        // 2つの block 和音(各複数音)
        let chord_notes: Vec<_> = notes.iter().filter(|n| n.chord.is_some()).collect();
        assert_eq!(chord_notes.len(), 2);
    }

    #[test]
    fn test_blk_produces_one_chord() {
        let notes = notes_of("blk(F)");
        let chord_notes: Vec<_> = notes.iter().filter(|n| n.chord.is_some()).collect();
        assert_eq!(chord_notes.len(), 1);
        // Baug/F: 4音(F bass + B D# G)
        assert_eq!(chord_notes[0].chord.as_ref().unwrap().len(), 4);
    }

    #[test]
    fn test_transform_with_key_directive() {
        // degree + key + transform: key C, s25(V7) -> Dm7 G7
        let ast = RechordImporter.import("key: C\ns25(V7) |\n").unwrap();
        let mut score = Score::new();
        score.interpret(ast.resolve()).unwrap();
        let chord_notes: Vec<_> = score.notes.iter().filter(|n| n.chord.is_some()).collect();
        assert_eq!(chord_notes.len(), 2);
    }
```

注: `notes_of` は既存のテストヘルパ(midi.rs のテストモジュール内、`arp` テストで使用)。無ければ既存実装を確認して流用。`resolve()` が into_pitch で `V7` を `G7` に、`s25` の内部 ChordNode も transpose 済みにする → interpret が展開。

- [ ] **Step 2: 実行**

Run: `cargo test -p chord_midi > /tmp/t.txt 2>&1; grep "test result" /tmp/t.txt`
Expected: 全 pass。失敗したら実際の note 数を確認し、期待値が誤りなら実挙動に合わせて修正(ただし「s25=2和音, blk=1和音4音」の意図は保つ)。

- [ ] **Step 3: Display roundtrip 確認**

`chord_midi/src/import/chord.rs` テストに追加(parse→Display→parse で Transform 保持):

```rust
    #[test]
    fn test_transform_display_roundtrip() {
        for code in ["blk(F)", "s25(G7)", "sub(G7)", "dom(Dm7)"] {
            let (_r, node) = node_parser(span(code)).unwrap();
            let text = format!("{}", NodeDisplay(&node)); // ※ Node の Display が無ければ下記参照
        }
    }
```
注: `Node` は `Display` を実装している(export/rechord.rs)。テストから使うには `use` が要る。`format!("{}", node)` で可。もし `Node` の Display が `pub` 経由で使えない場合は、`ChordNode` 単位でなく `node_parser`→`format!`→`node_parser` の往復で `Transform` バリアントが保持されることを assert する形にする:
```rust
    #[test]
    fn test_transform_display_roundtrip() {
        for code in ["blk(F)", "s25(G7)", "sub(G7)", "dom(Dm7)"] {
            let (_r, node) = node_parser(span(code)).unwrap();
            let text = format!("{}", node);
            let (_r2, node2) = node_parser(span(&text)).unwrap();
            assert!(matches!(node2, crate::model::ast::Node::Transform(_, _)), "{} -> {}", code, text);
        }
    }
```
(`Node` は `export/rechord.rs` で `Display` 実装済みなので `format!("{}", node)` が使える。)

- [ ] **Step 4: web 回帰**

Run: `cd chord_midi_web && cargo build --target wasm32-unknown-unknown 2>&1 | tail -2`
Expected: 成功。

- [ ] **Step 5: コミット**

```bash
cd /Users/kmt/dev/chord-midi && git add chord_midi/src
git commit -m "test: end-to-end chord-transform expansion through resolve and MIDI"
```

---

## Self-Review メモ

- **Spec coverage:** Transform enum/Node(A-5)、Key::shift+expand(A-6)、パーサ・Display・into_pitch・interpret 展開。cs は対象外(spec 通り)。
- **Type consistency:** `Node::Transform(Transform, ChordNode)`、`Transform::expand(self, ChordNode, Pitch, Scale) -> Vec<Node>`、`Key::shift(self, i8, Pitch, Scale) -> Key`。`Transform: Copy`。
- **非破壊:** resolve はフェーズ1のまま(into_pitch に Transform arm を足すのみ)。既存 Chord/Arpeggio/Dominant/tension 不変。展開は interpret 時のみ、Display は記法保持。
- **リスク:** interpret_node 再帰の flush 相互作用 — s25 の2和音がそれぞれ正しい尺を持つか e2e テストで確認。
