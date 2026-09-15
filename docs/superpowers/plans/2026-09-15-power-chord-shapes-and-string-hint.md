# 可動パワーコードと弦指定記法 @N Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 可動パワーコードの押さえ方24形を押さえ方データに足し、rechord 記法に「最低音をこの弦で取る」という弦指定 `@N` を足す。

**Architecture:** `@N` は音程に影響しない押さえ方への指示なので、`ChordNode` → `ChordRef` → `find` と持ち回るだけで MIDI 出力には触らない。可動パワーコード形は2音しか無く、無条件に候補へ入れると3rdや7thが欠けた形が本来のコードを押しのけるため、`find` に「ターゲットがルート+5thちょうど」か「`@N` 指定あり」のときだけ候補にするガードを入れる。あわせて `Fingering.tab`（`frets` を1弦1文字にした冗長な文字列）を消す。可動パワーコードには10フレット以上が出て1文字に収まらないため。

**Tech Stack:** Rust 2021 / nom 7 (パーサ) / Python 3 (生成スクリプト)

**Spec:** `docs/superpowers/specs/2026-09-15-power-chord-shapes-and-string-hint-design.md`

**Issue:** https://github.com/wakame-tech/chord-midi/issues/24

---

## File Structure

| ファイル | 変更 |
| --- | --- |
| `chord_midi/src/model/ast.rs` | `ChordNode` に `root_string: Option<u8>` |
| `chord_midi/src/import/chord.rs` | `@N` のパーサ |
| `chord_midi/src/import/sexp.rs` | `ChordNode` リテラルに `root_string: None` |
| `chord_midi/src/model/transform.rs` | `ChordNode` リテラルに `root_string` を持ち回す |
| `chord_midi/src/export/rechord.rs` | `ChordNode` の `Display` に `@N` |
| `chord_midi/src/fingering/mod.rs` | `Fingering` から `tab` を削除、`lowest_string` / `is_power` を追加、`ChordRef` に `root_string`、`find` のガード |
| `docs/power_chords.json` | 新規。可動パワーコード形24件 |
| `scripts/gen_fingerings.py` | 2つの JSON を読む、`tab` の出力をやめる、運指の特例、新フィールドの出力 |
| `chord_midi/src/fingering/data.rs` | 生成物。67 + 24 = 91件になる |
| `README.md` | `@N` の記法を追記 |

---

## 前提知識（このリポジトリを知らない人向け）

- **ワークスペース**: ルートの `Cargo.toml` のメンバーは `chord_midi` と `chord_midi_cli`。`chord_midi_web` は `exclude` されており、`cd chord_midi_web` して別ビルドする。
- **`chord_midi_cli` は現在ビルドできない**（依存の `simplelog` → `time 0.3.34` が現行ツールチェインで `error[E0282]`）。このブランチとは無関係な既知の壊れ。テストは必ず `cargo test -p chord_midi` で走らせること。`cargo test`（ワークスペース全体）は使わない。
- **押さえ方データ**: `docs/easy_guiter_chords.json` が原本で、`scripts/gen_fingerings.py` が `chord_midi/src/fingering/data.rs` を生成する（`just gen-fingerings`）。`data.rs` は生成物なので手で編集しない。
- **弦の並び**: 押さえ方の配列は一貫して **6弦→1弦**。添字 0 が6弦、5 が1弦。
- **標準チューニングの開放弦のピッチクラス**: 6弦→1弦で `[4, 9, 2, 7, 11, 4]`（E A D G B E）。
- **パーサ**: nom 7 + `nom_locate` + `nom_tracable`。`Span` は `import/parser_util.rs` の型エイリアス。
- **`pow(...)` は小節ではない**ので、後ろに `|` を付けると（`pow(C) |`）パースエラーになる。テスト入力は `pow(C)\n` と書く。

---

## Task 1: 記法 `@N`

**Files:**
- Modify: `chord_midi/src/model/ast.rs`
- Modify: `chord_midi/src/import/chord.rs`
- Modify: `chord_midi/src/import/sexp.rs`
- Modify: `chord_midi/src/model/transform.rs`
- Modify: `chord_midi/src/export/rechord.rs`
- Modify: `chord_midi/src/edit.rs`

- [ ] **Step 1: 失敗するテストを書く**

`chord_midi/src/import/chord.rs` の `mod tests` の末尾に足す。

```rust
    #[test]
    fn test_root_string_parser() {
        use crate::model::key::Key;
        use crate::model::pitch::Pitch;

        let (_r, node) = chord_node_parser(span("A@6")).unwrap();
        assert_eq!(node.root_string, Some(6));

        // @N は /on より後ろ
        let (_r, node) = chord_node_parser(span("A/E@6")).unwrap();
        assert_eq!(node.on, Some(Key::Absolute(Pitch::E)));
        assert_eq!(node.root_string, Some(6));

        let (_r, node) = chord_node_parser(span("A")).unwrap();
        assert_eq!(node.root_string, None);
    }

    #[test]
    fn test_root_string_out_of_range_is_rejected() {
        use crate::import::{Importer, RechordImporter};
        // 1〜6 以外は読めず、余りが残るのでパース全体が失敗する
        assert!(RechordImporter.import("A@0 |\n").is_err());
        assert!(RechordImporter.import("A@7 |\n").is_err());
    }
```

`chord_midi/src/export/rechord.rs` の `mod tests` に足す。

```rust
    #[test]
    fn test_root_string_roundtrip() {
        assert_eq!(roundtrip("A@6 |"), "A@6 |");
        assert_eq!(roundtrip("A/E@6 |"), "A/E@6 |");
        assert_eq!(roundtrip("pow(A@6 | D@5)"), "pow(A@6 | D@5)");
    }
```

`chord_midi/src/edit.rs` の `mod tests` に足す。

```rust
    #[test]
    fn test_shift_key_keeps_root_string() {
        assert_eq!(shift_key("pow(A@6)", 1), "pow(A#@6)");
        assert_eq!(shift_key("pow(A#@6)", -1), "pow(A@6)");
    }
```

- [ ] **Step 2: テストが失敗することを確認する**

Run: `cargo test -p chord_midi`
Expected: コンパイルエラー。`no field `root_string` on type `ChordNode``

- [ ] **Step 3: `ChordNode` にフィールドを足す**

`chord_midi/src/model/ast.rs` の `pub struct ChordNode` を差し替える。

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct ChordNode {
    pub key: Key,
    pub modifiers: BTreeSet<Modifier>,
    pub on: Option<Key>,
    /// `@N` で指定された「最低音を取る弦」。1 = 1弦, 6 = 6弦。
    /// 押さえ方検索にだけ効き、音程には影響しない
    pub root_string: Option<u8>,
}
```

同じファイルの `impl ChordNode` の2つのコンストラクタに足す。

```rust
impl ChordNode {
    pub fn absolute(pitch: Pitch) -> Self {
        ChordNode {
            key: Key::Absolute(pitch),
            modifiers: BTreeSet::new(),
            on: None,
            root_string: None,
        }
    }

    pub fn relative(degree: Degree) -> Self {
        ChordNode {
            key: Key::Relative(degree),
            modifiers: BTreeSet::new(),
            on: None,
            root_string: None,
        }
    }
```

同じファイルの `transform_chord` を差し替える。

```rust
fn transform_chord(key: Key, extra: &[Modifier], on: Option<Key>) -> ChordNode {
    let mut modifiers = BTreeSet::new();
    modifiers.insert(Modifier::Major(5));
    for m in extra {
        modifiers.insert(m.clone());
    }
    ChordNode {
        key,
        modifiers,
        on,
        root_string: None,
    }
}
```

同じファイルの `mod tests` に3箇所ある `ChordNode { ... }` リテラルすべてで、`on: None,` の下に `root_string: None,` を1行足す。

- [ ] **Step 4: 残りの構造体リテラルを直す**

`chord_midi/src/model/transform.rs` の2つの関数を差し替える。ここは値を持ち回す。

```rust
fn chordnode_into_degree(chord: ChordNode, tonic: Pitch, scale: Scale) -> ChordNode {
    ChordNode {
        key: chord.key.into_degree(tonic, scale),
        modifiers: chord.modifiers,
        on: chord.on.map(|on| on.into_degree(tonic, scale)),
        root_string: chord.root_string,
    }
}

fn chordnode_into_pitch(chord: ChordNode, tonic: Pitch, scale: Scale) -> ChordNode {
    ChordNode {
        key: chord.key.into_pitch(tonic, scale),
        modifiers: chord.modifiers,
        on: chord.on.map(|on| on.into_pitch(tonic, scale)),
        root_string: chord.root_string,
    }
}
```

`chord_midi/src/import/sexp.rs` の2つのリテラルに足す。sexp 記法には弦指定が無いので `None`。

```rust
        Sexp::String(key) => Ok(Node::Chord(ChordNode {
            key: parse_key(&key)?,
            modifiers: BTreeSet::new(),
            on: None,
            root_string: None,
        })),
        Sexp::List(list) if starts_with(&sexp, "chord") => Ok(Node::Chord(ChordNode {
            key: parse_key(list[1].string()?)?,
            modifiers: BTreeSet::new(),
            on: None,
            root_string: None,
        })),
```

- [ ] **Step 5: パーサを直す**

`chord_midi/src/import/chord.rs` の `use nom::` の並びに1行足す。

```rust
use nom::character::complete::one_of;
```

同じファイルの `chord_node_parser` を、直前の `#[tracable_parser]` の行も含めて差し替える（属性を二重に書かないこと）。

```rust
#[tracable_parser]
fn chord_node_parser(s: Span) -> IResult<Span, ChordNode> {
    map(
        tuple((
            key_parser,
            many0(modifier_parser),
            opt(tensions_parser),
            opt(preceded(tag("/"), key_parser)),
            // `@N` は押さえ方への指示。1〜6 以外は読まないので、`A@7` は
            // "@7" が余って measure のパースごと失敗する
            opt(preceded(tag("@"), one_of("123456"))),
        )),
        |(key, modifiers, tensions, on, root_string)| ChordNode {
            key,
            modifiers: BTreeSet::from_iter(
                vec![Modifier::Major(5)]
                    .into_iter()
                    .chain(modifiers.into_iter())
                    .chain(tensions.into_iter().flatten()),
            ),
            on,
            root_string: root_string.map(|c| c as u8 - b'0'),
        },
    )(s)
}
```

- [ ] **Step 6: Display を直す**

`chord_midi/src/export/rechord.rs` の `impl Display for ChordNode` の最後、`let on = ...` から `write!` までを差し替える。

```rust
        let on = self
            .on
            .as_ref()
            .map(|p| format!("/{}", p))
            .unwrap_or("".to_string());
        // `@N` を再出力する。これでキー移調がテキストを書き戻しても消えない
        let root_string = self
            .root_string
            .map(|n| format!("@{}", n))
            .unwrap_or_default();
        write!(f, "{}{}{}{}{}", self.key, mods, tensions, on, root_string)
    }
}
```

- [ ] **Step 7: テストが通ることを確認する**

Run: `cargo test -p chord_midi`
Expected: PASS。既存94件 + 今回の4件 = 98件

- [ ] **Step 8: コミット**

```bash
git add chord_midi/src/model/ast.rs chord_midi/src/import/chord.rs chord_midi/src/import/sexp.rs chord_midi/src/model/transform.rs chord_midi/src/export/rechord.rs chord_midi/src/edit.rs
git commit -m "feat(notation): 弦指定 @N を足す

コードの末尾に @N (1〜6) を書けるようにする。「最低音をこの弦で取る」
という押さえ方への指示で、音程には影響しない。/on より後ろに置く。
Display が再出力するので、キー移調の書き戻しで消えない。"
```

---

## Task 2: `Fingering.tab` を消す

可動パワーコードには10フレット以上が出る（6弦ルートの `D5` は 10fr + 12fr）。`tab` は `"x3201x"` のように1弦1文字なので2桁が入らず、`"1012xxxx"` と並べると読み分けられない。

`tab` は `frets` と同じ情報を文字列にしただけの冗長なフィールドで、参照しているのは `fingering/mod.rs` のテストと生成スクリプトだけ。web の描画は `frets` しか見ていない。先に消しておく。

**Files:**
- Modify: `chord_midi/src/fingering/mod.rs`
- Modify: `scripts/gen_fingerings.py`
- Modify (生成物): `chord_midi/src/fingering/data.rs`

このタスクは振る舞いを変えないリファクタなので、先に失敗するテストを書く形は取らない。**各ステップの後で必ずテストが緑のままであることを確認する**。

- [ ] **Step 1: テスト側のヘルパを差し替える**

`chord_midi/src/fingering/mod.rs` の `mod tests` にある `tab_of` を差し替える。

```rust
    /// テスト用。押さえ方を "x-3-2-0-1-x" の形にする。
    /// 10フレット以上でも読めるよう区切る
    fn tab(f: &Fingering) -> String {
        f.frets
            .iter()
            .map(|v| v.map_or("x".to_string(), |n| n.to_string()))
            .collect::<Vec<_>>()
            .join("-")
    }

    /// 押さえ方をフレットの配列で引く
    fn shape(frets: [Option<u8>; 6]) -> &'static Fingering {
        FINGERINGS
            .iter()
            .find(|f| f.frets == frets)
            .unwrap_or_else(|| panic!("no fingering with frets {:?}", frets))
    }

    fn tab_of(input: &str) -> String {
        let r = refs(input);
        tab(find(&r[0]).expect("no fingering found"))
    }
```

- [ ] **Step 2: 既存テストの期待値を書き換える**

`data_is_consistent` から `tab` の検査を落とす。冒頭から `assert_eq!(tab, f.tab, ...)` までを次に差し替える。

```rust
    #[test]
    fn data_is_consistent() {
        assert_eq!(FINGERINGS.len(), 67);
        for f in FINGERINGS {
            let mut pcs = 0u16;
```

`known_fingerings` を差し替える。

```rust
    #[test]
    fn known_fingerings() {
        // C = x-3-2-0-1-x。人=2弦1F / 中=4弦2F / 薬=5弦3F
        let c = shape([None, Some(3), Some(2), Some(0), Some(1), None]);
        assert_eq!(c.fingers[4], Some(Finger::Index));
        assert_eq!(c.fingers[2], Some(Finger::Middle));
        assert_eq!(c.fingers[1], Some(Finger::Ring));

        // F#m7-5 = 2-x-2-2-1-x。6弦は親指、2弦1F が人差し指
        let fsm = shape([Some(2), None, Some(2), Some(2), Some(1), None]);
        assert_eq!(fsm.fingers[0], Some(Finger::Thumb));
        assert_eq!(fsm.fingers[4], Some(Finger::Index));

        // G = 3-x-0-0-0-x。1本指のときは中指
        let g = shape([Some(3), None, Some(0), Some(0), Some(0), None]);
        assert_eq!(g.fingers[0], Some(Finger::Middle));
    }
```

`find_picks_the_matching_shape` を差し替える。

```rust
    #[test]
    fn find_picks_the_matching_shape() {
        assert_eq!(tab_of("C |\n"), "x-3-2-0-1-x");
        assert_eq!(tab_of("D |\n"), "x-x-0-2-3-2");
        assert_eq!(tab_of("E |\n"), "0-2-2-1-0-0");
        assert_eq!(tab_of("F |\n"), "x-x-3-2-1-x");
        assert_eq!(tab_of("G |\n"), "3-x-0-0-0-x");
        assert_eq!(tab_of("A |\n"), "x-0-2-2-2-x");
        assert_eq!(tab_of("Am7 |\n"), "x-0-2-0-1-x");
        assert_eq!(tab_of("Em7 |\n"), "0-2-0-0-0-0");
        assert_eq!(tab_of("Dm7 |\n"), "x-x-0-2-1-1");
    }
```

`find_prefers_the_fuller_chord` を差し替える。

```rust
    #[test]
    fn find_prefers_the_fuller_chord() {
        // CM7 には C の形ではなく C△7 の形が選ばれる
        assert_eq!(tab_of("CM7 |\n"), "x-3-x-0-0-0");
    }
```

`find_falls_back_to_the_substitute_shape` を差し替える。

```rust
    #[test]
    fn find_falls_back_to_the_substitute_shape() {
        // 原典は B を B7、F#m を F#m7 で代用させる構成になっている
        assert_eq!(tab_of("B |\n"), "x-2-1-2-0-x");
        assert_eq!(tab_of("Am |\n"), "x-0-2-0-1-x");
        assert_eq!(tab_of("Em |\n"), "0-2-0-0-0-0");
        assert_eq!(tab_of("F#m |\n"), "2-x-2-2-x-x");
        assert_eq!(tab_of("C#m |\n"), "x-4-x-4-0-0");
    }
```

`find_rejects_two_extra_notes` を差し替える。

```rust
    #[test]
    fn find_rejects_two_extra_notes() {
        // Cm7 (x-3-x-3-4-x) は C に対して Eb と Bb の2音が余分なので候補に入らない
        assert_ne!(tab_of("C |\n"), "x-3-x-3-4-x");
    }
```

`find_honors_slash_bass` を差し替える。

```rust
    #[test]
    fn find_honors_slash_bass() {
        assert_eq!(tab_of("D/F# |\n"), "2-x-0-2-3-x");
    }
```

- [ ] **Step 3: ここでテストが通ることを確認する**

`Fingering.tab` はまだ残っているが、テスト側が参照しなくなっただけなので緑のはず。

Run: `cargo test -p chord_midi`
Expected: PASS。98件

- [ ] **Step 4: `Fingering` から `tab` を消す**

`chord_midi/src/fingering/mod.rs` の `pub struct Fingering` から1行消す。

```rust
pub struct Fingering {
    /// 原典でのコード名。表示用で、照合には使わない
    pub name: &'static str,
    /// 原典のセクション名 ("Cコード系" など)
    pub group: &'static str,
    /// None = ミュート、Some(0) = 開放
    pub frets: [Option<u8>; 6],
    /// 押さえる弦だけ Some。開放弦とミュート弦は None
    pub fingers: [Option<Finger>; 6],
    /// 鳴る音のピッチクラス集合。bit n が立っていればピッチクラス n を含む
    pub pitch_classes: u16,
    /// 最低音（鳴る弦のうち一番低い弦）のピッチクラス
    pub bass: u8,
}
```

（`pub tab: &'static str,` の行を削除する。他のフィールドは触らない）

- [ ] **Step 5: 生成スクリプトから `tab` の出力を落とす**

`scripts/gen_fingerings.py` の `build_rust` から1行消す。

```python
            lines.append(f'        name: "{c["name"]}",')
            lines.append(f'        group: "{group["name"]}",')
            lines.append("        frets: [" + ", ".join(opt_u8(v) for v in fr) + "],")
```

（`lines.append(f'        tab: "{c["tab"]}",')` の行を削除する）

`docs/easy_guiter_chords.json` の `tab` フィールドと `docs/easy_guiter_chords.md` の「押さえ方」列は人が読む用として残すので、`build_markdown` と `main` は触らない。

- [ ] **Step 6: 再生成してテストが通ることを確認する**

Run: `just gen-fingerings`
Expected: `wrote 67 fingerings`

Run: `grep -c "tab:" chord_midi/src/fingering/data.rs`
Expected: `0`

Run: `cargo test -p chord_midi`
Expected: PASS。98件

- [ ] **Step 7: コミット**

```bash
git add chord_midi/src/fingering/ scripts/gen_fingerings.py
git commit -m "refactor(fingering): 冗長な tab フィールドを消す

tab は frets と同じ情報を1弦1文字の文字列にしただけで、参照している
のはテストと生成スクリプトだけだった。web の描画は frets しか見ていない。

可動パワーコードには10フレット以上が出て1文字に収まらないため、
足す前に消しておく。テストは frets の配列で引き、表示は
\"x-3-2-0-1-x\" の形に区切るヘルパを使う。"
```

---

## Task 3: 可動パワーコードのデータ

**Files:**
- Create: `docs/power_chords.json`
- Modify: `scripts/gen_fingerings.py`
- Modify: `chord_midi/src/fingering/mod.rs`
- Modify (生成物): `chord_midi/src/fingering/data.rs`

- [ ] **Step 1: 失敗するテストを書く**

`chord_midi/src/fingering/mod.rs` の `mod tests` に足す。

```rust
    #[test]
    fn power_chord_shapes_are_present() {
        let powers: Vec<_> = FINGERINGS.iter().filter(|f| f.is_power).collect();
        // 6弦ルート12キー + 5弦ルート12キー
        assert_eq!(powers.len(), 24);
        assert_eq!(FINGERINGS.len(), 67 + 24);
    }

    #[test]
    fn power_chord_shapes_sound_root_and_fifth() {
        for f in FINGERINGS.iter().filter(|f| f.is_power) {
            // 鳴るのは2音だけ
            assert_eq!(f.pitch_classes.count_ones(), 2, "{} {}", f.name, tab(f));
            // 最低音がルート、その完全5度上がもう1音
            let fifth = (f.bass + 7) % 12;
            let expected = (1u16 << f.bass) | (1u16 << fifth);
            assert_eq!(f.pitch_classes, expected, "{} {}", f.name, tab(f));
        }
    }

    #[test]
    fn power_chord_fingers_are_index_and_ring() {
        for f in FINGERINGS.iter().filter(|f| f.is_power) {
            let used: Vec<Finger> = f.fingers.iter().flatten().copied().collect();
            match used.len() {
                // 開放弦がルートの形は押さえるのが5thだけ
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

    #[test]
    fn power_chord_shapes_reach_high_frets() {
        // 6弦ルートの D5 は 10fr + 12fr。桁あふれせずに持てていること
        let d5 = shape([Some(10), Some(12), None, None, None, None]);
        assert!(d5.is_power);
        assert_eq!(tab(d5), "10-12-x-x-x-x");
    }

    #[test]
    fn lowest_string_matches_frets() {
        for f in FINGERINGS {
            let expected = f
                .frets
                .iter()
                .position(|v| v.is_some())
                .expect("no sounding string") as u8;
            assert_eq!(f.lowest_string, expected, "{} {}", f.name, tab(f));
        }
    }
```

- [ ] **Step 2: テストが失敗することを確認する**

Run: `cargo test -p chord_midi fingering`
Expected: コンパイルエラー。`no field `is_power` on type `Fingering``

- [ ] **Step 3: `Fingering` にフィールドを足す**

`chord_midi/src/fingering/mod.rs` の `pub struct Fingering` の `bass` の下に2つ足す。

```rust
    /// 最低音（鳴る弦のうち一番低い弦）のピッチクラス
    pub bass: u8,
    /// 最低音が鳴る弦。0 = 6弦, 5 = 1弦
    pub lowest_string: u8,
    /// 可動パワーコード形かどうか。`find` のガードに使う
    pub is_power: bool,
}
```

- [ ] **Step 4: データを作る**

`docs/power_chords.json` を新規作成する。ルート弦のフレットは `(ルートのピッチクラス - 開放弦のピッチクラス) mod 12`、5th はその2フレット上で隣の弦。

```json
{
  "description": "可動パワーコード形。ルートの隣の弦の2フレット上が5th。gen_fingerings.py が easy_guiter_chords.json と併せて読む。",
  "notation": "frets は 6弦→1弦。string はルートを取る弦 (6 または 5)。",
  "shapes": [
    { "root": "E",  "string": 6, "frets": [0, 2, null, null, null, null] },
    { "root": "F",  "string": 6, "frets": [1, 3, null, null, null, null] },
    { "root": "F#", "string": 6, "frets": [2, 4, null, null, null, null] },
    { "root": "G",  "string": 6, "frets": [3, 5, null, null, null, null] },
    { "root": "G#", "string": 6, "frets": [4, 6, null, null, null, null] },
    { "root": "A",  "string": 6, "frets": [5, 7, null, null, null, null] },
    { "root": "A#", "string": 6, "frets": [6, 8, null, null, null, null] },
    { "root": "B",  "string": 6, "frets": [7, 9, null, null, null, null] },
    { "root": "C",  "string": 6, "frets": [8, 10, null, null, null, null] },
    { "root": "C#", "string": 6, "frets": [9, 11, null, null, null, null] },
    { "root": "D",  "string": 6, "frets": [10, 12, null, null, null, null] },
    { "root": "D#", "string": 6, "frets": [11, 13, null, null, null, null] },
    { "root": "A",  "string": 5, "frets": [null, 0, 2, null, null, null] },
    { "root": "A#", "string": 5, "frets": [null, 1, 3, null, null, null] },
    { "root": "B",  "string": 5, "frets": [null, 2, 4, null, null, null] },
    { "root": "C",  "string": 5, "frets": [null, 3, 5, null, null, null] },
    { "root": "C#", "string": 5, "frets": [null, 4, 6, null, null, null] },
    { "root": "D",  "string": 5, "frets": [null, 5, 7, null, null, null] },
    { "root": "D#", "string": 5, "frets": [null, 6, 8, null, null, null] },
    { "root": "E",  "string": 5, "frets": [null, 7, 9, null, null, null] },
    { "root": "F",  "string": 5, "frets": [null, 8, 10, null, null, null] },
    { "root": "F#", "string": 5, "frets": [null, 9, 11, null, null, null] },
    { "root": "G",  "string": 5, "frets": [null, 10, 12, null, null, null] },
    { "root": "G#", "string": 5, "frets": [null, 11, 13, null, null, null] }
  ]
}
```

- [ ] **Step 5: 生成スクリプトを直す**

`scripts/gen_fingerings.py` の定数部、`RS_PATH` の下に足す。

```python
POWER_PATH = ROOT / "docs" / "power_chords.json"
```

同じファイルの `def opt_u8(v):` の上に足す。

```python
def lowest_string(frets):
    for i, f in enumerate(frets):
        if f is not None:
            return i
    raise ValueError(f"no sounding string: {frets}")


def assign_power_fingers(frets):
    """可動パワーコードは ルート=人差し指 / 5th=薬指。
    開放弦がルートの形は押さえるのが5thだけなので薬指1本になる"""
    fingers = [None] * 6
    pressed = [i for i, f in enumerate(frets) if f not in (None, 0)]
    if len(pressed) == 1:
        fingers[pressed[0]] = "ring"
    elif len(pressed) == 2:
        fingers[pressed[0]] = "index"
        fingers[pressed[1]] = "ring"
    else:
        raise ValueError(f"unexpected power chord shape: {frets}")
    return fingers
```

同じファイルの `build_rust` を差し替える。daichan のデータとパワーコードのデータを1つの並びにする。

```python
def build_rust(data, powers):
    lines = [
        "// @generated by scripts/gen_fingerings.py",
        "// from docs/easy_guiter_chords.json and docs/power_chords.json",
        "// 手で編集しない。`just gen-fingerings` で再生成する。",
        "use super::{Finger, Fingering};",
        "",
        "pub static FINGERINGS: &[Fingering] = &[",
    ]

    def entry(name, group, frets, fingers, is_power):
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
        lines.append(f"        is_power: {'true' if is_power else 'false'},")
        lines.append("    },")

    for group in data["groups"]:
        for c in group["chords"]:
            entry(c["name"], group["name"], c["frets"], c["fingers"], False)
    for shape in powers["shapes"]:
        entry(
            f"{shape['root']}5",
            f"{shape['string']}弦ルート",
            shape["frets"],
            assign_power_fingers(shape["frets"]),
            True,
        )

    lines.append("];")
    return "\n".join(lines) + "\n"
```

同じファイルの `main` の2行を差し替える。

```python
def main():
    data = json.loads(JSON_PATH.read_text(encoding="utf-8"))
    powers = json.loads(POWER_PATH.read_text(encoding="utf-8"))
```

```python
    RS_PATH.write_text(build_rust(data, powers), encoding="utf-8")

    total = sum(len(g["chords"]) for g in data["groups"]) + len(powers["shapes"])
    print(f"wrote {total} fingerings")
```

- [ ] **Step 6: 生成を走らせる**

Run: `just gen-fingerings`
Expected: `wrote 91 fingerings`

Run: `grep -c "is_power: true" chord_midi/src/fingering/data.rs`
Expected: `24`

Run: `grep -B 1 -A 8 'name: "A5"' chord_midi/src/fingering/data.rs | head -11`
Expected: 最初のエントリが

```rust
    Fingering {
        name: "A5",
        group: "6弦ルート",
        frets: [Some(5), Some(7), None, None, None, None],
        fingers: [Some(Finger::Index), Some(Finger::Ring), None, None, None, None],
        pitch_classes: 0b000000010010, // A, E
        bass: 9, // A
        lowest_string: 0,
        is_power: true,
    },
```

- [ ] **Step 7: 件数のテストを直す**

`chord_midi/src/fingering/mod.rs` の `data_is_consistent` の先頭を直す。

```rust
        assert_eq!(FINGERINGS.len(), 91);
```

- [ ] **Step 8: テストが通ることを確認する**

Run: `cargo test -p chord_midi`
Expected: PASS。98 + 5 = 103件

- [ ] **Step 9: コミット**

```bash
git add docs/power_chords.json scripts/gen_fingerings.py chord_midi/src/fingering/
git commit -m "feat(fingering): 可動パワーコード形24件を足す

6弦ルート12キー + 5弦ルート12キー。docs/power_chords.json を新設し、
gen_fingerings.py が easy_guiter_chords.json と併せて読む。

Fingering に lowest_string と is_power を足した。運指はルート=人差し指
/ 5th=薬指の特例にする。"
```

---

## Task 4: find のガードと弦指定の適用

**Files:**
- Modify: `chord_midi/src/fingering/mod.rs`

- [ ] **Step 1: 失敗するテストを書く**

`chord_midi/src/fingering/mod.rs` の `mod tests` に足す。

```rust
    #[test]
    fn find_honors_the_string_hint() {
        // pow(...) は小節ではないので後ろに `|` は付けない
        assert_eq!(tab_of("pow(A@6)\n"), "5-7-x-x-x-x");
        assert_eq!(tab_of("pow(D@5)\n"), "x-5-7-x-x-x");
        assert_eq!(tab_of("pow(E@5)\n"), "x-7-9-x-x-x");
        assert_eq!(tab_of("pow(E@6)\n"), "0-2-x-x-x-x");
    }

    #[test]
    fn find_picks_low_position_without_a_hint() {
        // 弦指定が無ければローポジション優先の既存 tie-break が効く
        assert_eq!(tab_of("pow(E)\n"), "0-2-x-x-x-x");
        assert_eq!(tab_of("pow(A)\n"), "x-0-2-x-x-x");
    }

    #[test]
    fn find_returns_none_when_the_hint_has_no_shape() {
        // 最低音が1弦になる押さえ方はデータに無い
        // (既存67件の最低音は6弦/5弦/4弦、パワーコード形は6弦/5弦)
        let r = refs("C@1 |\n");
        assert!(find(&r[0]).is_none());
    }

    #[test]
    fn power_shapes_do_not_displace_normal_chords() {
        // 2音しか無いパワーコード形が3rdや7thを持つ形を押しのけないこと
        assert_eq!(tab_of("C |\n"), "x-3-2-0-1-x");
        assert_eq!(tab_of("Am7 |\n"), "x-0-2-0-1-x");
        assert_eq!(tab_of("D#m7 |\n"), "x-3-x-3-4-x");
        assert_eq!(tab_of("C#sus4 |\n"), "x-1-x-1-2-x");
        assert_eq!(tab_of("G#m |\n"), "1-x-1-1-x-x");
    }
```

- [ ] **Step 2: テストが失敗することを確認する**

Run: `cargo test -p chord_midi fingering`
Expected: FAIL。`find_honors_the_string_hint` が `assertion `left == right` failed: left: "x-0-2-x-x-x", right: "5-7-x-x-x-x"`（弦指定がまだ効かない）。`power_shapes_do_not_displace_normal_chords` も `D#m7` などで落ちる

- [ ] **Step 3: `ChordRef` に弦指定を足す**

`chord_midi/src/fingering/mod.rs` の `pub struct ChordRef` に足す。

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
}
```

同じファイルの `chord_ref` の戻り値に足す。記法は 1 = 1弦 なので `6 - n` で添字に直す。

```rust
    Ok(ChordRef {
        label,
        root,
        pitch_classes,
        bass,
        // 記法は 1 = 1弦。Fingering.lowest_string は 0 = 6弦 なので変換する
        root_string: node.root_string.map(|n| 6 - n),
    })
```

同じファイルの `mod tests` の `find_returns_none_when_nothing_fits` にある `ChordRef { ... }` リテラルに `root_string: None,` を足す。

```rust
        let c = ChordRef {
            label: "test".to_string(),
            root: 1,
            pitch_classes: 1u16 << 1,
            bass: None,
            root_string: None,
        };
```

- [ ] **Step 4: `find` にガードを入れる**

`chord_midi/src/fingering/mod.rs` の `find` を差し替える。

```rust
/// コードに一番合う押さえ方を返す。
///
/// 構成音に無い音を1つまで許容する。原典は `B` を `B7`、`F#m` を `F#m7` で
/// 代用させる構成なので、部分集合だけに絞ると素の三和音が軒並み引けなくなる。
pub fn find(chord: &ChordRef) -> Option<&'static Fingering> {
    let expected_bass = chord.bass.unwrap_or(chord.root);
    // 可動パワーコード形は2音しか無いので、無条件に候補へ入れると
    // 3rd や 7th が欠けた形が本来のコードを押しのける。ルートと5th
    // ちょうどのときか、弦指定があるときだけ候補にする
    let power_target =
        chord.pitch_classes == (1u16 << chord.root) | (1u16 << ((chord.root + 7) % 12));
    FINGERINGS
        .iter()
        .filter(|f| !f.is_power || power_target || chord.root_string.is_some())
        .filter(|f| match chord.root_string {
            Some(string) => f.lowest_string == string,
            None => true,
        })
        .filter(|f| (f.pitch_classes & !chord.pitch_classes).count_ones() <= 1)
        .min_by_key(|f| {
            let extra: u32 = relative_semitones(f.pitch_classes & !chord.pitch_classes, chord.root)
                .map(extra_cost)
                .sum();
            let missing: u32 =
                relative_semitones(chord.pitch_classes & !f.pitch_classes, chord.root)
                    .map(missing_cost)
                    .sum();
            (
                extra + missing,
                u8::from(f.bass != expected_bass),
                // 鳴る弦は多い方が良い
                std::cmp::Reverse(f.sounding_strings()),
                // 同点ならローポジション優先
                f.max_fret().unwrap_or(0),
            )
        })
}
```

- [ ] **Step 5: テストが通ることを確認する**

Run: `cargo test -p chord_midi`
Expected: PASS。103 + 4 = 107件

- [ ] **Step 6: コミット**

```bash
git add chord_midi/src/fingering/mod.rs
git commit -m "feat(fingering): 弦指定 @N を押さえ方検索に効かせる

ChordRef に root_string を足し、find が最低音の弦で候補を絞るように
する。あわせて、可動パワーコード形はターゲットがルート+5thちょうどか
弦指定があるときだけ候補にするガードを入れる。これが無いと D#m7 が
3rd も b7 も無い D#5 に化ける。"
```

---

## Task 5: 仕上げ

**Files:**
- Modify: `README.md`

- [ ] **Step 1: README に記法を足す**

`README.md` の `## 関数` の節の末尾に足す。既存の内容は消さない。

```markdown
### 弦指定

コードの末尾に `@N` (N = 1〜6) を書くと「最低音をこの弦で取る」という
押さえ方への指示になる。音程には影響しない。`/on` より後ろに置く。

    pow(A@6 | D@5 | E@5)    6弦5fr / 5弦5fr / 5弦7fr のパワーコード
    A/E@6 |

指定した弦を最低音にする押さえ方がデータに無いときは「データなし」になる。
今のデータで最低音になり得るのは 6弦 / 5弦 / 4弦 だけなので、実質 `@6` と
`@5`（一部 `@4`）が使える。
```

- [ ] **Step 2: 全部のビルドとテストを流す**

Run: `cargo test -p chord_midi`
Expected: 107件 PASS

Run: `cd chord_midi_web && cargo check --target wasm32-unknown-unknown`
Expected: error ゼロ

Run: `cd chord_midi_web && trunk build --release --public-url /chord-midi/`
Expected: `✅ success`（CI と同じコマンド。`trunk` は `~/.cargo/bin/trunk` にある）

- [ ] **Step 3: ブラウザで動きを見る**

Run: `cd chord_midi_web && trunk serve --port 8088 --no-autoreload`

`http://127.0.0.1:8088/` を開いて次を入力する。

```
pow(A@6 | D@5 | E@5)
```

確認する点:
1. エラーが出ないこと
2. プレビュータブに3枚のカードが出て、それぞれ
   - `A5` = 6弦5fr(人) + 5弦7fr(薬)、`5fr` のフレット番号付き
   - `D5` = 5弦5fr(人) + 4弦7fr(薬)、`5fr` 付き
   - `E5` = 5弦7fr(人) + 4弦9fr(薬)、`7fr` 付き
3. `♯` を押すと `pow(A#@6 | D#@5 | F@5)` になり、`@N` が消えないこと
4. `pow(A | D | E)`（弦指定なし）だと開放弦を使うローポジションの形になること
5. `C | Am7 | F | G |` を入れて、押さえ方が今までどおりであること
6. `pow(D@6)` を入れると 10fr + 12fr の形が `10fr` 表示付きで描かれること

- [ ] **Step 4: コミットして PR を出す**

```bash
git add README.md
git commit -m "docs: 弦指定 @N の記法を README に追記"
git push -u origin feat/power-chord-shapes
gh pr create --fill
```

PR 本文には `Closes #24` を入れる。
