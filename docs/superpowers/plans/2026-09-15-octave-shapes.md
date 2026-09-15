# オクターブ奏法 oct() Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `pow()` と対称な `oct()` を足し、オクターブ奏法の押さえ方24形を押さえ方データに加える。

**Architecture:** `oct()` は `pow()` と同じ経路を通る。`Func::Oct` を足し、`resolve()` の脱糖で `Modifier::Octave`（構成音をルート1音にする）を付ける。押さえ方の選択は、今の「構成音がルート+5thちょうどか」というピッチクラスからの推測をやめ、`ChordRef.wanted` に書かれた関数の種類をそのまま持ち回す方式に変える。`Fingering.is_power: bool` は `kind: ShapeKind` に置き換える。

**Tech Stack:** Rust 2021 / nom 7 (パーサ) / Python 3 (生成スクリプト)

**Spec:** `docs/superpowers/specs/2026-09-15-octave-shapes-design.md`

**Issue:** https://github.com/wakame-tech/chord-midi/issues/28

---

## File Structure

| ファイル | 変更 |
| --- | --- |
| `chord_midi/src/model/ast.rs` | `Func::Oct` |
| `chord_midi/src/model/modifier.rs` | `Modifier::Octave`、`Display` |
| `chord_midi/src/model/chord.rs` | `modify` に `Octave` の腕 |
| `chord_midi/src/import/rechord.rs` | `oct(` のパーサ |
| `chord_midi/src/export/rechord.rs` | `Ast::Apply` の `Display` に `Func::Oct` |
| `chord_midi/src/model/transform.rs` | `desugar_pow` を `Pow` / `Oct` 共通の `desugar_shapes` にする |
| `chord_midi/src/fingering/mod.rs` | `ShapeKind`、`ChordRef` に `wanted`、`find` のガード |
| `docs/movable_shapes.json` | `power_chords.json` から改名、`kind` 追加、オクターブ形24件追加 |
| `scripts/gen_fingerings.py` | 新ファイル名、`kind` の出力 |
| `README.md` | `oct()` の記法 |

`chord_midi/src/export/midi.rs` は**触らない**。`interpret` の `Ast::Apply(_, items, _)` のワイルドカードが `Func::Oct` も拾うため。MIDI の音程は Issue #26 で押さえ方に追従するようになっているので、データが入れば自動で正しくなる。

---

## 前提知識（このリポジトリを知らない人向け）

- **ワークスペース**: ルートの `Cargo.toml` のメンバーは `chord_midi` と `chord_midi_cli`。`chord_midi_web` は `exclude` されており、`cd chord_midi_web` して別ビルドする。
- **`chord_midi_cli` は現在ビルドできない**（依存の `simplelog` → `time 0.3.34` が現行ツールチェインで `error[E0282]`）。このブランチとは無関係な既知の壊れ。テストは必ず `cargo test -p chord_midi` で走らせること。`cargo test`（ワークスペース全体）は使わない。
- **押さえ方データ**: `docs/easy_guiter_chords.json`（原典67件）と `docs/power_chords.json`（可動パワーコード24件）を `scripts/gen_fingerings.py` が読んで `chord_midi/src/fingering/data.rs` を生成する（`just gen-fingerings`）。`data.rs` は生成物なので手で編集しない。
- **弦の並び**: 押さえ方の配列は一貫して **6弦→1弦**。添字 0 が6弦、5 が1弦。
- **開放弦**: ピッチクラスは 6弦→1弦で `[4, 9, 2, 7, 11, 4]`、MIDI ノート番号は `[40, 45, 50, 55, 59, 64]`。
- **`Modifier` の順序**: `BTreeSet<Modifier>` に入っており、derive した `Ord` は **enum の宣言順**。後ろに宣言した修飾ほど後から適用され、前のものを上書きできる。`Power` は末尾にあり構成音を `{0, 7}` に置き換えている。
- **`pow(...)` は小節ではない**ので後ろに `|` を付けるとパースエラーになる。テスト入力は `"pow(C)"` と書く。`oct(...)` も同じ。
- **テストヘルパ**: `fingering/mod.rs` の `mod tests` に `refs(input)`（`ChordRef` の列）、`tab(f)`（`"x-3-2-0-1-x"` の形）、`shape(frets)`（フレットの配列で引く）、`tab_of(input)`（引いた押さえ方を文字列に）がある。`export/midi.rs` には `notes_of` / `resolved_notes_of` / `pitches_of` がある。

---

## Task 1: 記法 `oct()`

**Files:**
- Modify: `chord_midi/src/model/ast.rs`
- Modify: `chord_midi/src/model/modifier.rs`
- Modify: `chord_midi/src/model/chord.rs`
- Modify: `chord_midi/src/import/rechord.rs`
- Modify: `chord_midi/src/export/rechord.rs`
- Modify: `chord_midi/src/model/transform.rs`
- Modify: `chord_midi/src/edit.rs`

- [ ] **Step 1: 失敗するテストを書く**

`chord_midi/src/model/transform.rs` の `mod tests` に足す。

```rust
    #[test]
    fn test_oct_becomes_root_only() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::Node;

        fn semitones(code: &str) -> Vec<u8> {
            let ast = RechordImporter.import(code).unwrap().resolve();
            let Ast::Score(items) = &ast else { panic!("not a score") };
            let Ast::Measure(nodes, _) = items[0].as_ref() else {
                panic!("oct should be desugared into a measure: {:?}", items[0])
            };
            let Node::Chord(c) = &nodes[0] else { panic!("not a chord") };
            c.to_chord().unwrap().semitones.into_iter().collect()
        }

        // ルート1音だけになる
        assert_eq!(semitones("oct(A)\n"), vec![0]);
        // 元の修飾を上書きする
        assert_eq!(semitones("oct(Am7)\n"), vec![0]);
        assert_eq!(semitones("oct(AM7)\n"), vec![0]);
    }

    #[test]
    fn test_oct_keeps_slash_bass() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::Node;
        use crate::model::key::Key;
        use crate::model::pitch::Pitch;

        let ast = RechordImporter.import("oct(A/E)\n").unwrap().resolve();
        let Ast::Score(items) = &ast else { panic!("not a score") };
        let Ast::Measure(nodes, _) = items[0].as_ref() else { panic!("not a measure") };
        let Node::Chord(c) = &nodes[0] else { panic!("not a chord") };
        assert_eq!(c.on, Some(Key::Absolute(Pitch::E)));
    }

    #[test]
    fn test_oct_inside_pat_is_desugared() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::{Func, Node};

        let ast = RechordImporter
            .import("pat(\"oxox\", oct(A))\n")
            .unwrap()
            .resolve();
        let Ast::Score(items) = &ast else { panic!("not a score") };
        let Ast::Apply(Func::Pat(_), inner, _) = items[0].as_ref() else { panic!("not a pat") };
        let Ast::Measure(nodes, _) = inner[0].as_ref() else {
            panic!("oct should be desugared inside pat: {:?}", inner[0])
        };
        let Node::Chord(c) = &nodes[0] else { panic!("not a chord") };
        assert_eq!(
            c.to_chord().unwrap().semitones.into_iter().collect::<Vec<_>>(),
            vec![0]
        );
    }
```

`chord_midi/src/export/rechord.rs` の `mod tests` に足す。

```rust
    #[test]
    fn test_oct_roundtrip() {
        assert_eq!(roundtrip("oct(A@6 | D@5)"), "oct(A@6 | D@5)");
        assert_eq!(roundtrip("oct(A)"), "oct(A)");
        assert_eq!(roundtrip("pat(\"oxox\", oct(A@6))"), "pat(\"oxox\", oct(A@6))");
    }
```

`chord_midi/src/edit.rs` の `mod tests` に足す。

```rust
    #[test]
    fn test_shift_key_transposes_inside_oct() {
        assert_eq!(shift_key("oct(A@6)", 1), "oct(A#@6)");
        assert_eq!(shift_key("oct(A#@6)", -1), "oct(A@6)");
    }
```

- [ ] **Step 2: テストが失敗することを確認する**

Run: `cargo test -p chord_midi`
Expected: コンパイルエラー。`no variant or associated item named `Oct` found for enum `Func``

- [ ] **Step 3: `Func::Oct` と `Modifier::Octave` を足す**

`chord_midi/src/model/ast.rs` の `pub enum Func` を差し替える。

```rust
/// 小節列に掛ける関数
#[derive(Debug, Clone, PartialEq)]
pub enum Func {
    /// パワーコード化
    Pow,
    /// オクターブ奏法化
    Oct,
    /// リズムパターン
    Pat(Pattern),
}
```

`chord_midi/src/model/modifier.rs` の `pub enum Modifier` の**末尾**（`Power` の後ろ）に足す。

```rust
    Power,
    /// オクターブ奏法。ルートだけを鳴らす。Power と同じ理由で末尾に置き、
    /// BTreeSet の順で最後に適用させて他の修飾を上書きする
    Octave,
}
```

同じファイルの `impl Display for Modifier` の match に足す。

```rust
            Modifier::Octave => write!(f, "oct"),
```

`chord_midi/src/model/chord.rs` の `modify` の `Modifier::Power` の腕の下に足す。

```rust
            Modifier::Octave => {
                // 他の修飾を全部捨ててルートだけにする
                self.semitones.clear();
                self.semitones.insert(Scale::Major.semitone(1));
                Ok(())
            }
```

- [ ] **Step 4: パーサと Display を足す**

`chord_midi/src/import/rechord.rs` の `apply_parser` の `alt` に1行足す。`pow(` の下に置く。

```rust
            alt((
                map(tuple((tag("pow("), space0)), |_| Func::Pow),
                map(tuple((tag("oct("), space0)), |_| Func::Oct),
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
```

`chord_midi/src/export/rechord.rs` の `Ast::Apply` の腕にある `match func` に1行足す。

```rust
                match func {
                    Func::Pow => write!(f, "pow({})", inner)?,
                    Func::Oct => write!(f, "oct({})", inner)?,
                    Func::Pat(pattern) => write!(f, "pat(\"{}\", {})", pattern, inner)?,
                }
```

- [ ] **Step 5: 脱糖を `Pow` / `Oct` 共通にする**

`chord_midi/src/model/transform.rs` の `desugar_pow` / `mark_power` / `mark_power_node` / `mark_power_chord` の4つをまとめて差し替える。

```rust
/// `Apply(Func::Pow, ..)` と `Apply(Func::Oct, ..)` を潰し、
/// 中の全コードに対応する修飾を足す。
///
/// `Apply` は親のリストの中で複数の要素に展開されるので `Vec` を返す。
fn desugar_shapes(ast: Ast) -> Vec<Ast> {
    match ast {
        Ast::Score(items) => vec![Ast::Score(
            items
                .into_iter()
                .flat_map(|item| desugar_shapes(*item))
                .map(Box::new)
                .collect(),
        )],
        Ast::Apply(Func::Pow, items, _) => items
            .into_iter()
            .flat_map(|item| desugar_shapes(*item))
            .map(|item| mark(item, Modifier::Power))
            .collect(),
        Ast::Apply(Func::Oct, items, _) => items
            .into_iter()
            .flat_map(|item| desugar_shapes(*item))
            .map(|item| mark(item, Modifier::Octave))
            .collect(),
        Ast::Apply(func, items, br) => vec![Ast::Apply(
            func,
            items
                .into_iter()
                .flat_map(|item| desugar_shapes(*item))
                .map(Box::new)
                .collect(),
            br,
        )],
        other => vec![other],
    }
}

/// AST に含まれる全コードに修飾を足す
fn mark(ast: Ast, modifier: Modifier) -> Ast {
    match ast {
        Ast::Measure(nodes, br) => Ast::Measure(
            nodes
                .into_iter()
                .map(|node| mark_node(node, modifier.clone()))
                .collect(),
            br,
        ),
        Ast::Apply(func, items, br) => Ast::Apply(
            func,
            items
                .into_iter()
                .map(|item| Box::new(mark(*item, modifier.clone())))
                .collect(),
            br,
        ),
        other => other,
    }
}

fn mark_node(node: Node, modifier: Modifier) -> Node {
    match node {
        Node::Chord(c) => Node::Chord(mark_chord(c, modifier)),
        Node::Arpeggio(c) => Node::Arpeggio(mark_chord(c, modifier)),
        Node::Brush(c) => Node::Brush(mark_chord(c, modifier)),
        Node::Transform(t, c) => Node::Transform(t, mark_chord(c, modifier)),
        other => other,
    }
}

fn mark_chord(mut chord: ChordNode, modifier: Modifier) -> ChordNode {
    chord.modifiers.insert(modifier);
    chord
}
```

同じファイルの `resolve` の中で `desugar_pow` を呼んでいるところを `desugar_shapes` に直す。

```rust
        let desugared = desugar_shapes(transposed)
            .into_iter()
            .next()
            .expect("desugar_shapes always yields at least one node for a Score");
```

- [ ] **Step 6: テストが通ることを確認する**

Run: `cargo test -p chord_midi`
Expected: PASS。既存113件 + 今回の5件 = 118件

- [ ] **Step 7: コミット**

```bash
git add chord_midi/src/model/ast.rs chord_midi/src/model/modifier.rs chord_midi/src/model/chord.rs chord_midi/src/import/rechord.rs chord_midi/src/export/rechord.rs chord_midi/src/model/transform.rs chord_midi/src/edit.rs
git commit -m "feat(notation): オクターブ奏法の関数 oct() を足す

pow() と対称に、中のコードをルート1音だけにする oct() を足す。
Modifier::Octave は Power と同じく enum の末尾に置き、BTreeSet の順で
最後に適用させて他の修飾を上書きする。

resolve() の脱糖を Pow / Oct 共通の desugar_shapes にまとめた。"
```

---

## Task 2: `ShapeKind` と `wanted`

押さえ方の選択を、ピッチクラスからの推測ではなく書かれた関数の種類で決めるように変える。振る舞いを変えないリファクタなので、**各ステップの後でテストが緑のままであることを確認する**。

**Files:**
- Modify: `chord_midi/src/fingering/mod.rs`
- Modify: `scripts/gen_fingerings.py`
- Modify (生成物): `chord_midi/src/fingering/data.rs`

- [ ] **Step 1: `ShapeKind` を足して `is_power` を置き換える**

`chord_midi/src/fingering/mod.rs` の `pub struct Fingering` の上に足す。

```rust
/// 押さえ方の種類。`find` のガードに使う
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeKind {
    /// 原典の押さえ方
    Normal,
    /// 可動パワーコード形（ルート + 5th）
    Power,
    /// オクターブ形（ルート + 1オクターブ上のルート）
    Octave,
}
```

同じファイルの `pub struct Fingering` の `is_power` の行を差し替える。

```rust
    /// 最低音が鳴る弦。0 = 6弦, 5 = 1弦
    pub lowest_string: u8,
    /// 押さえ方の種類
    pub kind: ShapeKind,
}
```

（`pub is_power: bool,` の行を消してこの2行にする）

- [ ] **Step 2: `ChordRef` に `wanted` を足す**

同じファイルの `pub struct ChordRef` に足す。

```rust
pub struct ChordRef {
    /// 表示用のコード名
    pub label: String,
    /// ルートのピッチクラス。重み付けの基準にする
    pub root: u8,
    /// 構成音（スラッシュのベースを含む）のピッチクラス集合
    pub pitch_classes: u16,
    /// スラッシュコードのベース。無ければ None
    pub bass: Option<u8>,
    /// `@N` の指定。`Fingering.lowest_string` に合わせて 0 = 6弦 に正規化する
    pub root_string: Option<u8>,
    /// `pow()` / `oct()` で明示された押さえ方の種類
    pub wanted: Option<ShapeKind>,
}
```

同じファイルの `use` に `Modifier` を足す。

```rust
use crate::model::modifier::Modifier;
```

同じファイルの `chord_ref` の戻り値の直前に足し、戻り値にも足す。

```rust
    // 書かれた関数の種類をそのまま持ち回す。構成音から推測すると
    // oct(A/E) の構成音 {A, E} がパワーコードの条件と一致してしまう
    let wanted = if node.modifiers.contains(&Modifier::Octave) {
        Some(ShapeKind::Octave)
    } else if node.modifiers.contains(&Modifier::Power) {
        Some(ShapeKind::Power)
    } else {
        None
    };
    Ok(ChordRef {
        label,
        root,
        pitch_classes,
        bass,
        // 記法は 1 = 1弦。Fingering.lowest_string は 0 = 6弦 なので変換する
        root_string: node.root_string.map(|n| 6 - n),
        wanted,
    })
}
```

- [ ] **Step 3: `find` のガードを差し替える**

同じファイルの `find` の先頭から最初の `.filter(...)` までを差し替える。

```rust
pub fn find(chord: &ChordRef) -> Option<&'static Fingering> {
    let expected_bass = chord.bass.unwrap_or(chord.root);
    FINGERINGS
        .iter()
        // Normal は常に候補。可動形 (Power / Octave) は鳴る音が少ないので、
        // その種類が明示されたか @N の指定があるときだけ候補にする
        .filter(|f| match f.kind {
            ShapeKind::Normal => true,
            other => chord.wanted == Some(other) || chord.root_string.is_some(),
        })
        .filter(|f| match chord.root_string {
            Some(string) => f.lowest_string == string,
            None => true,
        })
```

（`let power_target = ...;` の2行と、`.filter(|f| !f.is_power || ...)` の行を消してこの形にする。以降の `.filter` と `.min_by_key` はそのまま）

- [ ] **Step 4: テストの `is_power` を `kind` に直す**

同じファイルの `mod tests` の3箇所を直す。

```rust
    #[test]
    fn power_chord_shapes_are_present() {
        let powers: Vec<_> = FINGERINGS
            .iter()
            .filter(|f| f.kind == ShapeKind::Power)
            .collect();
        // 6弦ルート12キー + 5弦ルート12キー
        assert_eq!(powers.len(), 24);
        assert_eq!(FINGERINGS.len(), 67 + 24);
    }
```

```rust
    #[test]
    fn power_chord_shapes_sound_root_and_fifth() {
        for f in FINGERINGS.iter().filter(|f| f.kind == ShapeKind::Power) {
```

```rust
    #[test]
    fn power_chord_fingers_are_index_and_ring() {
        for f in FINGERINGS.iter().filter(|f| f.kind == ShapeKind::Power) {
```

同じファイルの `power_chord_shapes_reach_high_frets` の `assert!(d5.is_power);` を直す。

```rust
        assert_eq!(d5.kind, ShapeKind::Power);
```

同じファイルの `find_returns_none_when_nothing_fits` の `ChordRef` リテラルに `wanted: None,` を足す。

```rust
        let c = ChordRef {
            label: "test".to_string(),
            root: 1,
            pitch_classes: 1u16 << 1,
            bass: None,
            root_string: None,
            wanted: None,
        };
```

- [ ] **Step 5: 生成スクリプトを直す**

`scripts/gen_fingerings.py` の `build_rust` の `use` 行と `entry` を差し替える。

```python
        "use super::{Finger, Fingering, ShapeKind};",
```

```python
    def entry(name, group, frets, fingers, kind):
        b = bass(frets)
        lines.append("    Fingering {")
        lines.append(f'        name: "{name}",')
        lines.append(f'        group: "{group}",')
        lines.append("        frets: [" + ", ".join(opt_u8(v) for v in frets) + "],")
        lines.append("        fingers: [" + ", ".join(opt_finger(v) for v in fingers) + "],")
        lines.append(
            f"        pitch_classes: 0b{pitch_classes(frets):012b}, // {', '.join(notes(frets))}"
        )
        lines.append(f"        bass: {b}, // {PC[b]}")
        lines.append(f"        lowest_string: {lowest_string(frets)},")
        lines.append(f"        kind: ShapeKind::{kind},")
        lines.append("    },")
```

同じファイルの2つの呼び出しを直す。

```python
    for group in data["groups"]:
        for c in group["chords"]:
            entry(c["name"], group["name"], c["frets"], c["fingers"], "Normal")
    for shape in powers["shapes"]:
        entry(
            f"{shape['root']}5",
            f"{shape['string']}弦ルート",
            shape["frets"],
            assign_power_fingers(shape["frets"]),
            "Power",
        )
```

- [ ] **Step 6: 再生成してテストが通ることを確認する**

Run: `just gen-fingerings`
Expected: `wrote 91 fingerings`

Run: `grep -c "kind: ShapeKind::Power" chord_midi/src/fingering/data.rs`
Expected: `24`

Run: `cargo test -p chord_midi`
Expected: PASS。118件。振る舞いを変えていないので件数も結果も変わらない

- [ ] **Step 7: コミット**

```bash
git add chord_midi/src/fingering/ scripts/gen_fingerings.py
git commit -m "refactor(fingering): 押さえ方の種類を ShapeKind で持つ

is_power: bool を kind: ShapeKind (Normal / Power / Octave) に変える。

あわせて find のガードを、構成音がルート+5thちょうどかという推測から、
ChordRef.wanted に書かれた関数の種類を持ち回す方式に変える。推測のままだと
oct(A/E) の構成音 {A} にスラッシュのベース E が足されて {A, E} になり、
パワーコードの条件と一致してしまう。"
```

---

## Task 3: オクターブ形のデータ

**Files:**
- Create: `docs/movable_shapes.json`（`docs/power_chords.json` から改名）
- Delete: `docs/power_chords.json`
- Modify: `scripts/gen_fingerings.py`
- Modify: `chord_midi/src/fingering/mod.rs`
- Modify (生成物): `chord_midi/src/fingering/data.rs`

- [ ] **Step 1: 失敗するテストを書く**

`chord_midi/src/fingering/mod.rs` の `mod tests` に足す。

```rust
    #[test]
    fn octave_shapes_are_present() {
        let octaves: Vec<_> = FINGERINGS
            .iter()
            .filter(|f| f.kind == ShapeKind::Octave)
            .collect();
        // 6弦ルート12キー + 5弦ルート12キー
        assert_eq!(octaves.len(), 24);
        assert_eq!(FINGERINGS.len(), 67 + 24 + 24);
    }

    #[test]
    fn octave_shapes_sound_one_pitch_class_twice() {
        for f in FINGERINGS.iter().filter(|f| f.kind == ShapeKind::Octave) {
            // 鳴る音は1種類だけ
            assert_eq!(f.pitch_classes.count_ones(), 1, "{} {}", f.name, tab(f));
            // 鳴る弦は2本で、ちょうど12半音離れている
            let sounding: Vec<u8> = f
                .frets
                .iter()
                .enumerate()
                .filter_map(|(i, v)| v.map(|n| OPEN_STRING_PITCHES[i] + n))
                .collect();
            assert_eq!(sounding.len(), 2, "{} {}", f.name, tab(f));
            assert_eq!(sounding[1] - sounding[0], 12, "{} {}", f.name, tab(f));
        }
    }

    #[test]
    fn octave_shape_fingers_are_index_and_ring() {
        for f in FINGERINGS.iter().filter(|f| f.kind == ShapeKind::Octave) {
            let used: Vec<Finger> = f.fingers.iter().flatten().copied().collect();
            match used.len() {
                // 開放弦がルートの形は押さえるのがオクターブ側だけ
                1 => assert_eq!(used, vec![Finger::Ring], "{} {}", f.name, tab(f)),
                2 => assert_eq!(
                    used,
                    vec![Finger::Index, Finger::Ring],
                    "{} {}",
                    f.name,
                    tab(f)
                ),
                n => panic!("unexpected pressed count {} for {}", n, f.name),
            }
        }
    }
```

同じファイルの `mod tests` の先頭あたりに、実音を求める定数を足す。

```rust
    /// 開放弦の MIDI ノート番号。6弦→1弦
    const OPEN_STRING_PITCHES: [u8; 6] = [40, 45, 50, 55, 59, 64];
```

- [ ] **Step 2: テストが失敗することを確認する**

Run: `cargo test -p chord_midi fingering`
Expected: FAIL。`octave_shapes_are_present` が `assertion `left == right` failed: left: 0, right: 24`

- [ ] **Step 3: データファイルを作る**

`docs/power_chords.json` を削除し、`docs/movable_shapes.json` を新規作成する。パワーコード形24件は既存と同じ値で、`kind` を足した形。

```json
{
  "description": "可動の押さえ方。power はルート + 5th、octave はルート + 1オクターブ上のルート。gen_fingerings.py が easy_guiter_chords.json と併せて読む。",
  "notation": "frets は 6弦→1弦。string はルートを取る弦 (6 または 5)。power は隣の弦の2フレット上、octave は2本細い弦の2フレット上が相方。",
  "shapes": [
    { "kind": "power", "root": "E",  "string": 6, "frets": [0, 2, null, null, null, null] },
    { "kind": "power", "root": "F",  "string": 6, "frets": [1, 3, null, null, null, null] },
    { "kind": "power", "root": "F#", "string": 6, "frets": [2, 4, null, null, null, null] },
    { "kind": "power", "root": "G",  "string": 6, "frets": [3, 5, null, null, null, null] },
    { "kind": "power", "root": "G#", "string": 6, "frets": [4, 6, null, null, null, null] },
    { "kind": "power", "root": "A",  "string": 6, "frets": [5, 7, null, null, null, null] },
    { "kind": "power", "root": "A#", "string": 6, "frets": [6, 8, null, null, null, null] },
    { "kind": "power", "root": "B",  "string": 6, "frets": [7, 9, null, null, null, null] },
    { "kind": "power", "root": "C",  "string": 6, "frets": [8, 10, null, null, null, null] },
    { "kind": "power", "root": "C#", "string": 6, "frets": [9, 11, null, null, null, null] },
    { "kind": "power", "root": "D",  "string": 6, "frets": [10, 12, null, null, null, null] },
    { "kind": "power", "root": "D#", "string": 6, "frets": [11, 13, null, null, null, null] },
    { "kind": "power", "root": "A",  "string": 5, "frets": [null, 0, 2, null, null, null] },
    { "kind": "power", "root": "A#", "string": 5, "frets": [null, 1, 3, null, null, null] },
    { "kind": "power", "root": "B",  "string": 5, "frets": [null, 2, 4, null, null, null] },
    { "kind": "power", "root": "C",  "string": 5, "frets": [null, 3, 5, null, null, null] },
    { "kind": "power", "root": "C#", "string": 5, "frets": [null, 4, 6, null, null, null] },
    { "kind": "power", "root": "D",  "string": 5, "frets": [null, 5, 7, null, null, null] },
    { "kind": "power", "root": "D#", "string": 5, "frets": [null, 6, 8, null, null, null] },
    { "kind": "power", "root": "E",  "string": 5, "frets": [null, 7, 9, null, null, null] },
    { "kind": "power", "root": "F",  "string": 5, "frets": [null, 8, 10, null, null, null] },
    { "kind": "power", "root": "F#", "string": 5, "frets": [null, 9, 11, null, null, null] },
    { "kind": "power", "root": "G",  "string": 5, "frets": [null, 10, 12, null, null, null] },
    { "kind": "power", "root": "G#", "string": 5, "frets": [null, 11, 13, null, null, null] },

    { "kind": "octave", "root": "E",  "string": 6, "frets": [0, null, 2, null, null, null] },
    { "kind": "octave", "root": "F",  "string": 6, "frets": [1, null, 3, null, null, null] },
    { "kind": "octave", "root": "F#", "string": 6, "frets": [2, null, 4, null, null, null] },
    { "kind": "octave", "root": "G",  "string": 6, "frets": [3, null, 5, null, null, null] },
    { "kind": "octave", "root": "G#", "string": 6, "frets": [4, null, 6, null, null, null] },
    { "kind": "octave", "root": "A",  "string": 6, "frets": [5, null, 7, null, null, null] },
    { "kind": "octave", "root": "A#", "string": 6, "frets": [6, null, 8, null, null, null] },
    { "kind": "octave", "root": "B",  "string": 6, "frets": [7, null, 9, null, null, null] },
    { "kind": "octave", "root": "C",  "string": 6, "frets": [8, null, 10, null, null, null] },
    { "kind": "octave", "root": "C#", "string": 6, "frets": [9, null, 11, null, null, null] },
    { "kind": "octave", "root": "D",  "string": 6, "frets": [10, null, 12, null, null, null] },
    { "kind": "octave", "root": "D#", "string": 6, "frets": [11, null, 13, null, null, null] },
    { "kind": "octave", "root": "A",  "string": 5, "frets": [null, 0, null, 2, null, null] },
    { "kind": "octave", "root": "A#", "string": 5, "frets": [null, 1, null, 3, null, null] },
    { "kind": "octave", "root": "B",  "string": 5, "frets": [null, 2, null, 4, null, null] },
    { "kind": "octave", "root": "C",  "string": 5, "frets": [null, 3, null, 5, null, null] },
    { "kind": "octave", "root": "C#", "string": 5, "frets": [null, 4, null, 6, null, null] },
    { "kind": "octave", "root": "D",  "string": 5, "frets": [null, 5, null, 7, null, null] },
    { "kind": "octave", "root": "D#", "string": 5, "frets": [null, 6, null, 8, null, null] },
    { "kind": "octave", "root": "E",  "string": 5, "frets": [null, 7, null, 9, null, null] },
    { "kind": "octave", "root": "F",  "string": 5, "frets": [null, 8, null, 10, null, null] },
    { "kind": "octave", "root": "F#", "string": 5, "frets": [null, 9, null, 11, null, null] },
    { "kind": "octave", "root": "G",  "string": 5, "frets": [null, 10, null, 12, null, null] },
    { "kind": "octave", "root": "G#", "string": 5, "frets": [null, 11, null, 13, null, null] }
  ]
}
```

- [ ] **Step 4: 生成スクリプトを直す**

`scripts/gen_fingerings.py` の定数を差し替える。

```python
SHAPES_PATH = ROOT / "docs" / "movable_shapes.json"
```

（`POWER_PATH = ROOT / "docs" / "power_chords.json"` の行をこれにする）

同じファイルの `assign_power_fingers` の名前と docstring を差し替える。中身は変えない（オクターブ形も「押さえる音のうち低い弦が人差し指、高い弦が薬指」で同じ）。

```python
def assign_movable_fingers(frets):
    """可動形は ルート=人差し指 / 相方=薬指。
    開放弦がルートの形は押さえるのが相方だけなので薬指1本になる"""
    fingers = [None] * 6
    pressed = [i for i, f in enumerate(frets) if f not in (None, 0)]
    if len(pressed) == 1:
        fingers[pressed[0]] = "ring"
    elif len(pressed) == 2:
        fingers[pressed[0]] = "index"
        fingers[pressed[1]] = "ring"
    else:
        raise ValueError(f"unexpected movable shape: {frets}")
    return fingers
```

同じファイルの `build_rust` の第2引数と、可動形のループを差し替える。

```python
def build_rust(data, shapes):
```

```python
    for shape in shapes["shapes"]:
        if shape["kind"] == "power":
            name = f"{shape['root']}5"
            group = f"{shape['string']}弦ルート"
            kind = "Power"
        else:
            name = f"{shape['root']}oct"
            group = f"{shape['string']}弦ルート (オクターブ)"
            kind = "Octave"
        entry(
            name,
            group,
            shape["frets"],
            assign_movable_fingers(shape["frets"]),
            kind,
        )
```

同じファイルの `main` の3箇所を差し替える。

```python
    shapes = json.loads(SHAPES_PATH.read_text(encoding="utf-8"))
```

```python
    RS_PATH.write_text(build_rust(data, shapes), encoding="utf-8")

    total = sum(len(g["chords"]) for g in data["groups"]) + len(shapes["shapes"])
    print(f"wrote {total} fingerings")
```

- [ ] **Step 5: 生成を走らせる**

Run: `just gen-fingerings`
Expected: `wrote 115 fingerings`

Run: `grep -c "kind: ShapeKind::Octave" chord_midi/src/fingering/data.rs`
Expected: `24`

Run: `grep -B 1 -A 8 'name: "Aoct"' chord_midi/src/fingering/data.rs | head -11`
Expected: 最初のエントリが

```rust
    Fingering {
        name: "Aoct",
        group: "6弦ルート (オクターブ)",
        frets: [Some(5), None, Some(7), None, None, None],
        fingers: [Some(Finger::Index), None, Some(Finger::Ring), None, None, None],
        pitch_classes: 0b000000000010, // A, A
        bass: 9, // A
        lowest_string: 0,
        kind: ShapeKind::Octave,
    },
```

- [ ] **Step 6: 件数のテストを直す**

`chord_midi/src/fingering/mod.rs` の `data_is_consistent` と `power_chord_shapes_are_present` の件数を直す。

```rust
        assert_eq!(FINGERINGS.len(), 115);
```

```rust
        assert_eq!(powers.len(), 24);
        assert_eq!(FINGERINGS.len(), 67 + 24 + 24);
```

- [ ] **Step 7: テストが通ることを確認する**

Run: `cargo test -p chord_midi`
Expected: PASS。118 + 3 = 121件

- [ ] **Step 8: コミット**

```bash
git add docs/ scripts/gen_fingerings.py chord_midi/src/fingering/
git commit -m "feat(fingering): オクターブ形24件を足す

6弦ルート12キー (相方は4弦) + 5弦ルート12キー (相方は3弦)。どちらも
ルートの2フレット上でちょうど1オクターブ上になる。

power_chords.json を movable_shapes.json に改名し、各形に kind を
持たせた。運指は可動形共通で ルート=人差し指 / 相方=薬指。"
```

---

## Task 4: find と MIDI の結合テスト

**Files:**
- Modify: `chord_midi/src/fingering/mod.rs`
- Modify: `chord_midi/src/export/midi.rs`

- [ ] **Step 1: 失敗するかもしれないテストを書く**

`chord_midi/src/fingering/mod.rs` の `mod tests` に足す。

```rust
    #[test]
    fn find_picks_octave_shapes_for_oct() {
        assert_eq!(tab_of("oct(A@6)\n"), "5-x-7-x-x-x");
        assert_eq!(tab_of("oct(G#@6)\n"), "4-x-6-x-x-x");
        assert_eq!(tab_of("oct(C#@6)\n"), "9-x-11-x-x-x");
        assert_eq!(tab_of("oct(A@5)\n"), "x-0-x-2-x-x");
    }

    #[test]
    fn find_keeps_power_and_octave_apart() {
        // 同じ弦指定でも、書かれた関数どおりの種類が選ばれる
        assert_eq!(tab_of("pow(A@6)\n"), "5-7-x-x-x-x");
        assert_eq!(tab_of("oct(A@6)\n"), "5-x-7-x-x-x");
    }

    #[test]
    fn find_picks_octave_shape_for_slash_bass() {
        // スラッシュのベースが構成音に足されてパワーコードの条件と一致しても、
        // 書かれた関数が oct なのでオクターブ形が選ばれる
        let r = refs("oct(A/E@6)\n");
        let f = find(&r[0]).expect("no fingering found");
        assert_eq!(f.kind, ShapeKind::Octave);
    }

    #[test]
    fn octave_shapes_do_not_displace_normal_chords() {
        // 鳴る音が1種類しかないオクターブ形が通常のコードを押しのけないこと
        assert_eq!(tab_of("C |\n"), "x-3-2-0-1-x");
        assert_eq!(tab_of("Am7 |\n"), "x-0-2-0-1-x");
        assert_eq!(tab_of("D#m7 |\n"), "x-3-x-3-4-x");
        assert_eq!(tab_of("C#sus4 |\n"), "x-1-x-1-2-x");
        assert_eq!(tab_of("G#m |\n"), "1-x-1-1-x-x");
    }
```

`chord_midi/src/export/midi.rs` の `mod tests` に足す。

```rust
    #[test]
    fn test_octave_shape_sounds_the_fingering() {
        // 6弦5fr=A2(45) + 4弦7fr=A3(57)
        assert_eq!(pitches_of("oct(A@6)"), vec![45, 57]);
        // 6弦4fr=G#2(44) + 4弦6fr=G#3(56)
        assert_eq!(pitches_of("oct(G#@6)"), vec![44, 56]);
        // 6弦9fr=C#3(49) + 4弦11fr=C#4(61)
        assert_eq!(pitches_of("oct(C#@6)"), vec![49, 61]);
    }
```

- [ ] **Step 2: テストを走らせる**

Run: `cargo test -p chord_midi`
Expected: Task 1〜3 が正しく入っていればこの5件はそのまま PASS する。121 + 5 = 126件

**落ちたものがあれば Step 3 に進む。全部通ったら Step 3 は飛ばして Step 4 へ。**

- [ ] **Step 3: 落ちたテストを直す**

`find_picks_octave_shapes_for_oct` が落ちる場合、`chord_ref` の `wanted` が `Modifier::Octave` を拾えているか確認する。`oct(...)` は `resolve()` で脱糖されて `Modifier::Octave` が付くので、`refs()` ヘルパが `resolve()` を通していることも確認する。

`octave_shapes_do_not_displace_normal_chords` が落ちる場合、`find` のガードで `ShapeKind::Octave` が `chord.wanted == Some(ShapeKind::Octave) || chord.root_string.is_some()` の条件で絞られているか確認する。

`test_octave_shape_sounds_the_fingering` が落ちる場合、`export/midi.rs` の `voicing` が `fingering::find` の結果を使っているか確認する（Issue #26 で入れた経路）。

- [ ] **Step 4: テストが通ることを確認する**

Run: `cargo test -p chord_midi`
Expected: PASS。126件

- [ ] **Step 5: コミット**

```bash
git add chord_midi/src/fingering/mod.rs chord_midi/src/export/midi.rs
git commit -m "test: oct() が押さえ方と MIDI に反映されることを固定する

弦指定つきの oct が正しいオクターブ形を引くこと、pow と取り違えない
こと、スラッシュのベースがあってもオクターブ形が選ばれること、通常の
コードの結果が変わらないこと、MIDI が押さえ方どおりの実音になること
をテストで固定する。"
```

---

## Task 5: 仕上げ

**Files:**
- Modify: `README.md`

- [ ] **Step 1: README に記法を足す**

`README.md` の `## 関数` の節、`pow` の説明の並びに足す。既存の内容は消さない。

```markdown
    oct(A@6 | D@5)            オクターブ奏法（ルートと1オクターブ上のルート）
```

さらに `### 弦指定` の節の末尾に足す。

```markdown
`oct()` はルートと1オクターブ上のルートだけを鳴らす。間の弦はミュートする。

    oct(A@6)    6弦5fr + 4弦7fr    (A2 + A3)
    oct(A@5)    5弦0fr + 3弦2fr    (A2 + A3)
```

- [ ] **Step 2: 全部のビルドとテストを流す**

Run: `cargo test -p chord_midi`
Expected: 126件 PASS

Run: `cd chord_midi_web && cargo check --target wasm32-unknown-unknown`
Expected: error ゼロ

Run: `cd chord_midi_web && trunk build --release --public-url /chord-midi/`
Expected: `✅ success`（CI と同じコマンド。`trunk` は `~/.cargo/bin/trunk` にある）

- [ ] **Step 3: ブラウザで確かめる**

Run: `cd chord_midi_web && trunk serve --port 8088 --no-autoreload`

`http://127.0.0.1:8088/` を開いて次を入力する。

```
oct(G#@6 | A@6 | C#@6)
```

確認する点:
1. エラーが出ないこと
2. プレビュータブに3枚のカードが出て、それぞれ
   - `G#oct` = 6弦4fr(人) + 4弦6fr(薬)、5弦は `×`
   - `Aoct` = 6弦5fr(人) + 4弦7fr(薬)、5弦は `×`
   - `C#oct` = 6弦9fr(人) + 4弦11fr(薬)、5弦は `×`
3. カード名に `≒` の併記が出ないこと（ラベルと押さえ方の名前が一致するため）
4. 再生して、押さえ方どおりのオクターブが鳴ること。ブラウザのコンソールで確認する

```js
const p = document.querySelector('midi-player');
await p.start(); await new Promise(r => setTimeout(r, 700)); p.stop();
const ns = document.querySelector('#visualizer').noteSequence;
const byStart = {};
for (const n of ns.notes) (byStart[+n.startTime.toFixed(3)] ||= []).push(n.pitch);
[0, 2, 4].map(t => byStart[t].slice().sort((a, b) => a - b));
```

Expected: `[[44, 56], [45, 57], [49, 61]]`

5. `pow(A@6 | D@5 | E@5)` を入れて、従来どおりパワーコード形が出ること
6. `C | Am7 | F | G |` を入れて、押さえ方が今までどおりであること

- [ ] **Step 4: コミットして PR を出す**

```bash
git add README.md
git commit -m "docs: oct() の記法を README に追記"
git push -u origin feat/octave-shapes
gh pr create --fill
```

PR 本文には `Closes #28` を入れる。
