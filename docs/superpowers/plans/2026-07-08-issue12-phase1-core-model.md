# Issue #12 フェーズ1: コアモデル基盤 実装計画

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** キー/スケール(旋法)対応の土台を作る。`Key::Relative` を度数保持にし、`Scale` に旋法を追加し、`key:`/`scale:` ディレクティブと `Ast::resolve`(度数→実音 transpose)を実装、MIDI 生成に統合する。変換関数(フェーズ2)と web(フェーズ3)は対象外。

**Architecture:** パーサは旋法非依存に度数(`Degree`)だけ記録。旋法解決は `into_pitch(tonic, scale)` で行う。`key:`/`scale:` ディレクティブを AST に持ち、`Ast::resolve()` が抽出して transpose、`MidiExporter::export` が resolve を呼ぶ。

**Tech Stack:** Rust, nom, midi_file. 対象クレート `chord_midi`(`chord_midi_web` は触らない)。

参照 spec: `docs/superpowers/specs/2026-07-08-progression-presets-and-transforms-design.md`

**既知の環境事情:** `cargo test -p chord_midi` は本作業前で 19 passed。`chord_midi_cli` は外部 `time` crate でビルド不可(無関係)。テスト出力が hook で握られる場合は `cargo test -p chord_midi > /tmp/t.txt 2>&1; grep "test result" /tmp/t.txt` で確認。

---

## Task 1: `Scale` に旋法追加 + Copy + FromStr + Display

**Files:**
- Modify: `chord_midi/src/model/scale.rs`

- [ ] **Step 1: 失敗するテストを書く**

`chord_midi/src/model/scale.rs` のテストモジュールに追加:

```rust
    #[test]
    fn test_modes() {
        // Dorian: [2,1,2,2,2,1,2]  III=minor 3rd=3
        assert_eq!(Scale::Dorian.semitone(3), 3);
        assert_eq!(Scale::Dorian.semitone(7), 10);
        // Lydian: #IV  IV=6
        assert_eq!(Scale::Lydian.semitone(4), 6);
        // Phrygian: bII  II=1
        assert_eq!(Scale::Phrygian.semitone(2), 1);
    }

    #[test]
    fn test_scale_fromstr_display() {
        use std::str::FromStr;
        assert_eq!(Scale::from_str("dorian").unwrap(), Scale::Dorian);
        assert_eq!(Scale::from_str("MAJOR").unwrap(), Scale::Major);
        assert_eq!(Scale::from_str("aeolian").unwrap(), Scale::Minor);
        assert_eq!(format!("{}", Scale::Dorian), "dorian");
        assert_eq!(format!("{}", Scale::Major), "major");
    }
```

- [ ] **Step 2: 失敗を確認**

Run: `cargo test -p chord_midi test_modes` — Expected: コンパイルエラー(`Scale::Dorian` なし)

- [ ] **Step 3: Scale に旋法・Copy を追加**

`chord_midi/src/model/scale.rs` の enum と `degrees()`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Scale {
    Major,
    Minor,
    Dorian,
    Phrygian,
    Lydian,
    Mixolydian,
    Locrian,
}

impl Scale {
    pub fn degrees(&self) -> Vec<u8> {
        match self {
            Scale::Major => vec![2, 2, 1, 2, 2, 2, 1],
            Scale::Minor => vec![2, 1, 2, 2, 1, 2, 2],
            Scale::Dorian => vec![2, 1, 2, 2, 2, 1, 2],
            Scale::Phrygian => vec![1, 2, 2, 2, 1, 2, 2],
            Scale::Lydian => vec![2, 2, 2, 1, 2, 2, 1],
            Scale::Mixolydian => vec![2, 2, 1, 2, 2, 1, 2],
            Scale::Locrian => vec![1, 2, 2, 1, 2, 2, 2],
        }
    }
    // semitone(), semitones() は現状維持
}
```

- [ ] **Step 4: FromStr / Display を追加**

`chord_midi/src/model/scale.rs`(既存 import に `std::str::FromStr` が無ければ追加):

```rust
impl std::str::FromStr for Scale {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> anyhow::Result<Self> {
        match s.to_lowercase().as_str() {
            "major" | "ionian" => Ok(Scale::Major),
            "minor" | "aeolian" => Ok(Scale::Minor),
            "dorian" => Ok(Scale::Dorian),
            "phrygian" => Ok(Scale::Phrygian),
            "lydian" => Ok(Scale::Lydian),
            "mixolydian" => Ok(Scale::Mixolydian),
            "locrian" => Ok(Scale::Locrian),
            _ => Err(anyhow::anyhow!("invalid scale: {}", s)),
        }
    }
}

impl std::fmt::Display for Scale {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Scale::Major => "major",
            Scale::Minor => "minor",
            Scale::Dorian => "dorian",
            Scale::Phrygian => "phrygian",
            Scale::Lydian => "lydian",
            Scale::Mixolydian => "mixolydian",
            Scale::Locrian => "locrian",
        };
        write!(f, "{}", s)
    }
}
```

(`scale.rs` 冒頭に既に `use std::fmt::Display;` があるので `impl Display for Scale` と書いてもよい。)

- [ ] **Step 5: テスト通過を確認**

Run: `cargo test -p chord_midi > /tmp/t.txt 2>&1; grep "test result" /tmp/t.txt`
Expected: 全 pass(既存 + `test_modes` + `test_scale_fromstr_display`)

- [ ] **Step 6: コミット**

```bash
git add chord_midi/src/model/scale.rs
git commit -m "feat: add church modes, Copy, FromStr, Display to Scale"
```

---

## Task 2: `Degree` Display を度数ローマに修正

**Files:**
- Modify: `chord_midi/src/model/scale.rs`

現状 `Degree(番号, 臨時記号)` の Display は半音基準の `to_roman_str(self.0)` を使い、`Degree(3, Natural)` が `II#` と誤表示される。度数番号→ローマに直す。臨時記号は前置(`bIII`)。

- [ ] **Step 1: 失敗するテストを書く**

`chord_midi/src/model/scale.rs` テストに追加:

```rust
    #[test]
    fn test_degree_display() {
        use crate::model::pitch::Accidental;
        assert_eq!(format!("{}", Degree(1, Accidental::Natural)), "I");
        assert_eq!(format!("{}", Degree(3, Accidental::Natural)), "III");
        assert_eq!(format!("{}", Degree(2, Accidental::Sharp)), "#II");
        assert_eq!(format!("{}", Degree(6, Accidental::Flat)), "bVI");
    }
```

- [ ] **Step 2: 失敗を確認**

Run: `cargo test -p chord_midi test_degree_display > /tmp/t.txt 2>&1; grep -E "test result|II#|assert" /tmp/t.txt`
Expected: FAIL(現状 `II#` 等、番号≠ローマ)

- [ ] **Step 3: Display を修正**

`chord_midi/src/model/scale.rs` の `Display for Degree` を置換。`to_roman_str`(半音基準)は使わず度数番号→ローマ:

```rust
fn degree_to_roman(n: u8) -> &'static str {
    match n {
        1 => "I",
        2 => "II",
        3 => "III",
        4 => "IV",
        5 => "V",
        6 => "VI",
        7 => "VII",
        _ => "?",
    }
}

impl Display for Degree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.1, degree_to_roman(self.0))
    }
}
```

`to_roman_str` が他で未使用なら削除(未使用警告回避)。`Degree::from_semitone` は変更しない(番号+臨時記号を返す)。

注: `modifier.rs` の `Tension(Degree(number, accidental))` Display は `Degree` の Display を呼ばず `write!("{}{}", accidental, number)` を直接使っている(前フェーズで修正済み)。よってこの変更で `b9` 等のテンション表記は壊れない。実装後 `cargo test -p chord_midi test_chord_display_roundtrip` で確認すること。

- [ ] **Step 4: テスト通過を確認**

Run: `cargo test -p chord_midi > /tmp/t.txt 2>&1; grep "test result" /tmp/t.txt`
Expected: 全 pass(`test_chord_display_roundtrip` 含む)

- [ ] **Step 5: コミット**

```bash
git add chord_midi/src/model/scale.rs
git commit -m "fix: Degree Display renders degree number as roman numeral"
```

---

## Task 3: `Key::Relative(Degree)` 化 + `into_pitch/into_degree(tonic, scale)`

型変更が全体に波及するので、このタスクで型変更・全コンパイル箇所修正・既存テスト更新まで一括で行い緑にする。

**Files:**
- Modify: `chord_midi/src/model/key.rs`
- Modify: `chord_midi/src/model/ast.rs`
- Modify: `chord_midi/src/model/chord.rs`
- Modify: `chord_midi/src/model/transform.rs`
- Modify: `chord_midi/src/import/chord.rs`
- Modify: `chord_midi/src/import/sexp.rs`

- [ ] **Step 1: 失敗するテストを書く**

`chord_midi/src/model/key.rs` にテストモジュールを追加(無ければ):

```rust
#[cfg(test)]
mod tests {
    use super::Key;
    use crate::model::pitch::{Accidental, Pitch};
    use crate::model::scale::{Degree, Scale};

    #[test]
    fn test_into_pitch_with_scale() {
        // III in C major = E (4 semitones)
        let k = Key::Relative(Degree(3, Accidental::Natural));
        assert_eq!(k.into_pitch(Pitch::C, Scale::Major), Key::Absolute(Pitch::E));
        // III in C dorian = D# (3 semitones)
        let k = Key::Relative(Degree(3, Accidental::Natural));
        assert_eq!(k.into_pitch(Pitch::C, Scale::Dorian), Key::Absolute(Pitch::Ds));
        // bIII in C major = D# (3 semitones)
        let k = Key::Relative(Degree(3, Accidental::Flat));
        assert_eq!(k.into_pitch(Pitch::C, Scale::Major), Key::Absolute(Pitch::Ds));
    }

    #[test]
    fn test_into_degree_roundtrip() {
        // G in C major -> V (chromatic from_semitone)
        let k = Key::Absolute(Pitch::G);
        assert_eq!(
            k.into_degree(Pitch::C, Scale::Major),
            Key::Relative(Degree(5, Accidental::Natural))
        );
    }
}
```

- [ ] **Step 2: 失敗を確認**

Run: `cargo test -p chord_midi test_into_pitch_with_scale`
Expected: コンパイルエラー(`Key::Relative(Degree)` 未対応、`into_pitch` シグネチャ違い)

- [ ] **Step 3: `key.rs` を書き換え**

`chord_midi/src/model/key.rs` 全体:

```rust
use super::{
    pitch::Pitch,
    scale::{Degree, Scale},
};
use std::fmt::Display;

#[derive(Debug, PartialEq, Clone)]
pub enum Key {
    Absolute(Pitch),
    /// scale degree (number + accidental). Resolved to a pitch via into_pitch(tonic, scale).
    Relative(Degree),
}

impl Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Key::Absolute(pitch) => write!(f, "{}", pitch),
            Key::Relative(degree) => write!(f, "{}", degree),
        }
    }
}

impl Key {
    pub fn into_pitch(self, tonic: Pitch, scale: Scale) -> Key {
        match self {
            Key::Absolute(_) => self,
            Key::Relative(Degree(n, acc)) => {
                let semitone = scale.semitone(n) as i8 + i8::from(acc);
                let p = (semitone + tonic as i8).rem_euclid(12) as u8;
                Key::Absolute(Pitch::try_from(p).unwrap())
            }
        }
    }

    pub fn into_degree(self, tonic: Pitch, _scale: Scale) -> Key {
        match self {
            Key::Absolute(p) => {
                let semitone = (p as i8 - tonic as i8).rem_euclid(12) as u8;
                Key::Relative(Degree::from_semitone(semitone))
            }
            Key::Relative(_) => self,
        }
    }
}
```

注: `Pitch` は `Copy`(既に derive 済み)なので `tonic as i8` 可。`Accidental` の `i8` 変換は `i8::from(acc)`(既存 `From<Accidental> for i8`)。

- [ ] **Step 4: `ast.rs` の `ChordNode::relative` を Degree 化**

`chord_midi/src/model/ast.rs`:

```rust
use crate::model::scale::Degree; // 追加(未 import なら)

// ChordNode::relative を変更
    pub fn relative(degree: Degree) -> Self {
        ChordNode {
            key: Key::Relative(degree),
            modifiers: BTreeSet::new(),
            on: None,
        }
    }
```

- [ ] **Step 5: `chord.rs` の Relative 参照を修正**

`chord_midi/src/model/chord.rs`:
- `root_pitch`(`chord.rs` 内): `Key::Relative(d)` の `d` は `Degree`。エラーメッセージのみ:
```rust
            Key::Relative(d) => Err(anyhow::anyhow!("relative key: {}", d)),
```
`Degree` の Display があるので `{}` で可。
- `distance`(`chord.rs` 内): `(Key::Relative(sa), Key::Relative(sb))` アームで `*sa`/`*sb` を半音として使っている。`Degree` になるので、`Degree` を代表半音へ変換して比較する:
```rust
            (Key::Relative(da), Key::Relative(db)) => {
                let sa = Scale::Major.semitone(da.0) as i8 + i8::from(da.1.clone());
                let sb = Scale::Major.semitone(db.0) as i8 + i8::from(db.1.clone());
                let a = self.octave as i8 * 12 + sa;
                let b = other.octave as i8 * 12 + sb;
                Ok::<_, anyhow::Error>(a.abs_diff(b) as u8)
            }
```
(`Degree` フィールドは `pub`。`Scale` は `chord.rs` で既に import 済み。)

- [ ] **Step 6: `import/chord.rs` の degree パーサを Degree 化**

`chord_midi/src/import/chord.rs`:
```rust
// degree_parser: Degree を返す。臨時記号は任意(前置)
#[tracable_parser]
fn degree_parser(s: Span) -> IResult<Span, Degree> {
    map(
        tuple((opt(accidental_parser), degree_name_parser)),
        |(a, d)| Degree(d, a.unwrap_or(Accidental::Natural)),
    )(s)
}
// key_parser: Relative(Degree)
    alt((
        map(pitch_parser, Key::Absolute),
        map(degree_parser, Key::Relative),
    ))(s)
```

`degree_name_parser`(ローマ→u8)は流用。`Degree`/`Accidental` は import 済み(`use crate::model::scale::{Degree, Scale};`, `use crate::model::pitch::{Accidental, Pitch};`)。`opt` も import 済み。

- [ ] **Step 7: `import/sexp.rs` の degree を Degree 化**

`chord_midi/src/import/sexp.rs` の `degree_from_str`(現状 `Scale::Major.semitone(d) as u8` を返す)を `Degree` を返すよう変更し、`parse_key` の `Key::Relative(...)` に合わせる:

```rust
// degree_from_str: ローマ(+任意の #/b)-> Degree
fn degree_from_str(s: &str) -> Result<Degree> {
    // 末尾の #/b を分離
    let (roman, acc) = if let Some(stripped) = s.strip_suffix('#') {
        (stripped, Accidental::Sharp)
    } else if let Some(stripped) = s.strip_suffix('b') {
        (stripped, Accidental::Flat)
    } else {
        (s, Accidental::Natural)
    };
    let d = parser_roman_num(roman)?;
    Ok(Degree(d, acc))
}
```

`sexp.rs` の import に `use crate::model::scale::Degree;`、`use crate::model::pitch::Accidental;`、`use crate::import::chord::parser_roman_num;`(既に参照しているはず。無ければ追加)を確認。`parse_key` は `Key::Relative(degree_from_str(degree)?)` のままで型が合う。

- [ ] **Step 8: `transform.rs` に scale を通す + 既存テスト更新**

`chord_midi/src/model/transform.rs`:
- `chordnode_into_degree` / `chordnode_into_pitch` に `tonic`/`scale` を追加:
```rust
fn chordnode_into_degree(chord: ChordNode, tonic: Pitch, scale: Scale) -> ChordNode {
    ChordNode {
        key: chord.key.into_degree(tonic, scale),
        modifiers: chord.modifiers,
        on: chord.on.map(|on| on.into_degree(tonic, scale)),
    }
}
fn chordnode_into_pitch(chord: ChordNode, tonic: Pitch, scale: Scale) -> ChordNode {
    ChordNode {
        key: chord.key.into_pitch(tonic, scale),
        modifiers: chord.modifiers,
        on: chord.on.map(|on| on.into_pitch(tonic, scale)),
    }
}
```
- `Ast::into_degree`/`Ast::into_pitch` シグネチャに `scale: Scale` を追加し、再帰で伝播。`Node::Chord`/`Node::Arpeggio` 両方を処理(既存パターン踏襲):
```rust
impl Ast {
    pub fn into_degree(self, tonic: Pitch, scale: Scale) -> Ast {
        match self {
            Ast::Score(nodes) => Ast::Score(
                nodes.into_iter().map(|ast| Box::new(ast.into_degree(tonic, scale))).collect(),
            ),
            Ast::Measure(nodes, br) => Ast::Measure(
                nodes.into_iter().map(|node| match node {
                    Node::Chord(c) => Node::Chord(chordnode_into_degree(c, tonic, scale)),
                    Node::Arpeggio(c) => Node::Arpeggio(chordnode_into_degree(c, tonic, scale)),
                    other => other,
                }).collect(),
                br,
            ),
            other => other,
        }
    }
    pub fn into_pitch(self, tonic: Pitch, scale: Scale) -> Ast {
        match self {
            Ast::Score(nodes) => Ast::Score(
                nodes.into_iter().map(|ast| Box::new(ast.into_pitch(tonic, scale))).collect(),
            ),
            Ast::Measure(nodes, br) => Ast::Measure(
                nodes.into_iter().map(|node| match node {
                    Node::Chord(c) => Node::Chord(chordnode_into_pitch(c, tonic, scale)),
                    Node::Arpeggio(c) => Node::Arpeggio(chordnode_into_pitch(c, tonic, scale)),
                    other => other,
                }).collect(),
                br,
            ),
            other => other,
        }
    }
}
```
`use` に `crate::model::scale::Scale;` を追加。`Box<Ast>` の deref に注意(`*ast` 相当。上記は `ast: Box<Ast>` を `*ast` して渡す形にする: `.map(|ast| Box::new((*ast).into_pitch(tonic, scale)))`)。

- 既存テスト `test_transpose` / `test_into_degree_is_inverse` を Degree・scale 対応に更新:
```rust
    #[test]
    fn test_transpose() {
        use crate::model::pitch::Accidental;
        use crate::model::scale::{Degree, Scale};
        // I (degree 1) in C major == C
        let i = Node::Chord(ChordNode::relative(Degree(1, Accidental::Natural)));
        let c = Node::Chord(ChordNode::absolute(Pitch::C));
        assert_eq!(
            Ast::Measure(vec![i], false).into_pitch(Pitch::C, Scale::Major),
            Ast::Measure(vec![c], false)
        );
        // IV (degree 4) in C major == F
        let iv = Node::Chord(ChordNode::relative(Degree(4, Accidental::Natural)));
        let f = Node::Chord(ChordNode::absolute(Pitch::F));
        assert_eq!(
            Ast::Measure(vec![iv], false).into_pitch(Pitch::C, Scale::Major),
            Ast::Measure(vec![f], false)
        );
    }

    #[test]
    fn test_into_degree_is_inverse() {
        use crate::model::pitch::Accidental;
        use crate::model::scale::{Degree, Scale};
        let g = Node::Chord(ChordNode::absolute(Pitch::G));
        let v = Node::Chord(ChordNode::relative(Degree(5, Accidental::Natural)));
        assert_eq!(
            Ast::Measure(vec![g], false).into_degree(Pitch::C, Scale::Major),
            Ast::Measure(vec![v], false)
        );
    }
```

- [ ] **Step 9: `sexp.rs` の `into_pitch` 呼び出しに scale を渡す**

`chord_midi/src/import/sexp.rs` の `keyed` 処理(`parse_ast` 内)で `into_pitch(pitch)` を呼んでいる箇所を `into_pitch(pitch, Scale::Major)` に変更(sexp のキー付けは既定 Major)。`use crate::model::scale::Scale;` を確認。

- [ ] **Step 10: 全体コンパイル & テスト**

Run: `cargo test -p chord_midi > /tmp/t.txt 2>&1; grep "test result" /tmp/t.txt; grep -E "error\[|^error" /tmp/t.txt | head`
Expected: 全 pass。コンパイルエラー無し。他のコンパイル箇所(`ChordNode::relative(1)` 等)が残っていれば全て Degree に更新すること。特に `chord_midi/src/import/sexp.rs` のテスト内 `ChordNode::relative(1)`/`relative(5)` を `relative(Degree(1, Accidental::Natural))`/`relative(Degree(5, Accidental::Natural))`… に更新(sexp テストの期待値は元の半音 1,5 が「度数」でなく「半音」だった点に注意 — sexp テストが検証している内容を読み、度数として正しい値に修正するか、テストの意図を保持する形で Degree へ変換する。判断に迷えば DONE_WITH_CONCERNS で報告)。

- [ ] **Step 11: コミット**

```bash
git add chord_midi/src
git commit -m "refactor: Key::Relative holds a scale Degree; thread scale through transpose"
```

---

## Task 4: `key:` / `scale:` ディレクティブ

**Files:**
- Modify: `chord_midi/src/model/ast.rs`
- Modify: `chord_midi/src/import/rechord.rs`
- Modify: `chord_midi/src/export/rechord.rs`

- [ ] **Step 1: 失敗するテストを書く**

`chord_midi/src/import/rechord.rs` テストに追加:

```rust
    #[test]
    fn test_directive_parser() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::{Ast, Directive};
        use crate::model::pitch::Pitch;
        use crate::model::scale::Scale;
        let ast = RechordImporter.import("key: C\nscale: dorian\nC |\n").unwrap();
        let Ast::Score(items) = &ast else { panic!() };
        // 先頭2つが Directive
        assert!(matches!(items[0].as_ref(), Ast::Directive(Directive::Key(Pitch::C))));
        assert!(matches!(items[1].as_ref(), Ast::Directive(Directive::Scale(Scale::Dorian))));
    }
```

- [ ] **Step 2: 失敗を確認**

Run: `cargo test -p chord_midi test_directive_parser`
Expected: コンパイルエラー(`Directive` 未定義)

- [ ] **Step 3: AST に Directive 追加**

`chord_midi/src/model/ast.rs`:

```rust
use crate::model::scale::Scale; // 追加

#[derive(Debug, PartialEq)]
pub enum Directive {
    Key(Pitch),
    Scale(Scale),
}

#[derive(Debug, PartialEq)]
pub enum Ast {
    Comment(String),
    Directive(Directive),
    Measure(Vec<Node>, bool),
    Score(Vec<Box<Ast>>),
}
```

- [ ] **Step 4: directive_parser を追加**

`chord_midi/src/import/rechord.rs`:

```rust
use crate::model::ast::{Ast, Directive};
use crate::model::pitch::Pitch;
use crate::model::scale::Scale;
use std::str::FromStr;

#[tracable_parser]
fn directive_parser(s: Span) -> IResult<Span, Ast> {
    alt((
        map(
            tuple((tag("key:"), space0, not_line_ending, line_ending)),
            |(_, _, val, _): (Span, Span, Span, Span)| {
                Ast::Directive(Directive::Key(Pitch::from_str(val.trim()).unwrap()))
            },
        ),
        map(
            tuple((tag("scale:"), space0, not_line_ending, line_ending)),
            |(_, _, val, _): (Span, Span, Span, Span)| {
                Ast::Directive(Directive::Scale(Scale::from_str(val.trim()).unwrap()))
            },
        ),
    ))(s)
}
```

`ast_parser` の `alt` に `directive_parser` を追加(`comment_parser` の前):

```rust
        tuple((many1(alt((directive_parser, comment_parser, measure_parser))), eof)),
```

注: `not_line_ending` は `Span`。`val.trim()` するため `val.fragment().trim()` か、`Span` を `&str` 化して trim する必要。`not_line_ending` の結果 `Span` は `Deref<Target=&str>` ではないので `val.fragment().trim()` を使う(`nom_locate::LocatedSpan::fragment()`)。`Pitch::from_str`/`Scale::from_str` の `unwrap()` は不正入力でパニックするが、初期実装は許容(将来 `map_res` 化)。`space0` は既に import 済み。

- [ ] **Step 5: Display for Ast に Directive を追加**

`chord_midi/src/export/rechord.rs` の `Display for Ast` の match に追加:

```rust
            Ast::Directive(Directive::Key(p)) => writeln!(f, "key: {}", p),
            Ast::Directive(Directive::Scale(s)) => writeln!(f, "scale: {}", s),
```

`use crate::model::ast::{Ast, ChordNode, Node, Directive};` に `Directive` を追加。

- [ ] **Step 6: テスト通過を確認**

Run: `cargo test -p chord_midi > /tmp/t.txt 2>&1; grep "test result" /tmp/t.txt`
Expected: 全 pass

- [ ] **Step 7: コミット**

```bash
git add chord_midi/src
git commit -m "feat: parse key:/scale: directives into the AST"
```

---

## Task 5: `Ast::resolve` + MidiExporter 統合

**Files:**
- Modify: `chord_midi/src/model/transform.rs`(resolve 追加)
- Modify: `chord_midi/src/export/midi.rs`(resolve 呼び出し)

- [ ] **Step 1: 失敗するテストを書く**

`chord_midi/src/model/transform.rs` テストに追加:

```rust
    #[test]
    fn test_resolve_transposes_degrees() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::{Ast, Node};
        // key: C, V7 -> G7 の実音になる
        let ast = RechordImporter.import("key: C\nV7 |\n").unwrap();
        let resolved = ast.resolve();
        // Directive は除去され Measure だけ残る
        let Ast::Score(items) = &resolved else { panic!() };
        let mut found_g = false;
        for it in items {
            if let Ast::Measure(nodes, _) = it.as_ref() {
                for n in nodes {
                    if let Node::Chord(c) = n {
                        // G は Absolute
                        assert!(matches!(c.key, crate::model::key::Key::Absolute(_)));
                        found_g = true;
                    }
                }
            }
        }
        assert!(found_g);
    }
```

- [ ] **Step 2: 失敗を確認**

Run: `cargo test -p chord_midi test_resolve_transposes_degrees`
Expected: コンパイルエラー(`resolve` 未定義)

- [ ] **Step 3: `Ast::resolve` を実装**

`chord_midi/src/model/transform.rs` に追加。ディレクティブから tonic/scale を抽出 → `into_pitch` → Directive/Comment を除去:

```rust
use crate::model::ast::Directive;
use crate::model::pitch::Pitch;

impl Ast {
    /// key/scale ディレクティブを適用し、度数を実音へ transpose、
    /// Directive/Comment を除いた演奏用 AST を返す。
    /// (フェーズ2で Transform 展開をここに追加する)
    pub fn resolve(self) -> Ast {
        // ディレクティブ抽出(最後の指定を採用)
        let mut tonic: Option<Pitch> = None;
        let mut scale = Scale::Major;
        if let Ast::Score(items) = &self {
            for it in items {
                match it.as_ref() {
                    Ast::Directive(Directive::Key(p)) => tonic = Some(*p),
                    Ast::Directive(Directive::Scale(s)) => scale = *s,
                    _ => {}
                }
            }
        }
        // transpose(tonic があれば)
        let transposed = match tonic {
            Some(t) => self.into_pitch(t, scale),
            None => self,
        };
        // Directive/Comment を除去
        match transposed {
            Ast::Score(items) => Ast::Score(
                items
                    .into_iter()
                    .filter(|it| !matches!(it.as_ref(), Ast::Directive(_) | Ast::Comment(_)))
                    .collect(),
            ),
            other => other,
        }
    }
}
```

注: `into_pitch` は Directive/Comment/Measure を含む Score をそのまま再帰(Directive/Comment は `other => other` で不変)。`Scale` は既に import 済み(Task 3)。`Pitch` import を確認。

- [ ] **Step 4: MidiExporter で resolve を呼ぶ**

`chord_midi/src/export/midi.rs` の `impl Exporter for MidiExporter` の `export` 冒頭で resolve:

```rust
impl Exporter for MidiExporter {
    fn export(&self, f: &mut impl Write, ast: Ast) -> anyhow::Result<()> {
        let ast = ast.resolve();
        let mut score = Score::new();
        score.interpret(ast)?;
        dump_notes(f, &score.notes, self.bpm)
    }
}
```

- [ ] **Step 5: 統合テストを追加(degree 進行が MIDI 生成できる)**

`chord_midi/src/export/midi.rs` テストに追加:

```rust
    #[test]
    fn test_degree_progression_midi() {
        // key 指定があれば degree 進行が MIDI 化できる(以前は relative で失敗)
        let ast = RechordImporter.import("key: C\nI | IV | V | I |\n").unwrap();
        let mut score = Score::new();
        score.interpret(ast.resolve()).unwrap();
        assert!(!score.notes.is_empty());
    }
```

(`RechordImporter`/`Importer` は既存テストで import 済み。無ければ `use crate::import::{Importer, RechordImporter};`)

- [ ] **Step 6: テスト通過を確認**

Run: `cargo test -p chord_midi > /tmp/t.txt 2>&1; grep "test result" /tmp/t.txt`
Expected: 全 pass

- [ ] **Step 7: web の回帰確認(resolve 統合で壊れないこと)**

Run: `cd chord_midi_web && cargo build --target wasm32-unknown-unknown 2>&1 | tail -3`
Expected: コンパイル成功(compile→export が resolve を通す。既存の絶対コード進行は tonic 無しで素通り)

- [ ] **Step 8: コミット**

```bash
git add chord_midi/src
git commit -m "feat: Ast::resolve applies key/scale and MidiExporter resolves before export"
```

---

## Self-Review メモ

- **Spec coverage:** Scale旋法(Task1)、Degree Display修正(Task2)、Relative(Degree)+into_pitch(scale)(Task3)、ディレクティブ(Task4)、resolve+MIDI統合(Task5)。フェーズ1 spec 項目を網羅。変換関数(フェーズ2)・web(フェーズ3)は別計画。
- **Type consistency:** `Key::Relative(Degree)`、`into_pitch(tonic: Pitch, scale: Scale)`、`ChordNode::relative(Degree)` を全タスクで統一。`Scale: Copy` で値渡し。
- **既存挙動の非破壊:** 絶対コード(`C | Am |`)は Relative 非経由・tonic 無しで resolve 素通り、従来どおり。`Chord::scale()` 不変。arp/Dominant/tension は無変更。
- **リスク:** Task3 の型変更は広域。sexp テストの `relative(u8)` の意味(半音 vs 度数)に注意 — 実装時に該当テストの意図を読んで正しい Degree に変換。
