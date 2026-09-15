# 押さえ方ダイアグラム表示 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `chord_midi_web` の入力欄に書いたコード進行の各コードについて、ギターの押さえ方と運指を HTML/CSS のダイアグラムとして表示する。

**Architecture:** 押さえ方データ (`docs/easy_guiter_chords.json`, 67件) から Python スクリプトで Rust の const テーブル `chord_midi/src/fingering/data.rs` を生成し、コンパイル時に埋め込む。入力コードとの照合は構成音（ピッチクラス集合）で行い、`chord_midi` クレート側に置いて host 上でテストする。`chord_midi_web` は Yew コンポーネントで描画するだけ。依存の向きは web → core の一方向。

**Tech Stack:** Rust 2021 / Yew 0.21 (CSR) / Trunk 0.21 / Python 3 (生成スクリプトのみ、ビルド時には走らない)

**Spec:** `docs/superpowers/specs/2026-09-15-guitar-chord-diagram-design.md`

**Issue:** https://github.com/wakame-tech/chord-midi/issues/17

---

## File Structure

| ファイル | 役割 |
| --- | --- |
| `scripts/gen_fingerings.py` | 新規。JSON を読んで運指を導出し、`data.rs` / JSON の `fingers` / Markdown を書き出す |
| `chord_midi/src/fingering/data.rs` | 新規・**生成物**。`FINGERINGS` const テーブル。手で編集しない |
| `chord_midi/src/fingering/mod.rs` | 新規。`Finger` / `Fingering` 型、AST 走査 (`chord_refs`)、照合 (`find`) |
| `chord_midi/src/lib.rs` | 変更。`pub mod fingering;` を追加 |
| `Justfile` | 変更。`gen-fingerings` レシピを追加 |
| `docs/easy_guiter_chords.json` | 変更（スクリプトが `fingers` を追記） |
| `docs/easy_guiter_chords.md` | 変更（スクリプトが運指の列を追記） |
| `chord_midi_web/src/diagram.rs` | 新規。`ChordDiagram` コンポーネント |
| `chord_midi_web/style.css` | 新規。ダイアグラムの CSS |
| `chord_midi_web/index.html` | 変更。`style.css` を Trunk のパイプラインに乗せる |
| `chord_midi_web/src/main.rs` | 変更。`compile` の戻り値に `Vec<ChordRef>` を足し、カードを描画 |

依存の追加は無い。`chord_midi` にも `chord_midi_web` にも新しい crate を入れない。

---

## 前提知識（このリポジトリを知らない人向け）

- **ワークスペース**: ルートの `Cargo.toml` は `chord_midi` と `chord_midi_cli` だけをメンバーに持ち、`chord_midi_web` は `exclude` されている。web は `cd chord_midi_web` してから別ビルドする。
- **AST**: `RechordImporter.import(text)` がテキストを `Ast` にする。`Ast::Score(Vec<Box<Ast>>)` → `Ast::Measure(Vec<Node>, bool)` → `Node::Chord(ChordNode)` という入れ子。
- **`Ast::resolve()`**: `key:` / `scale:` ディレクティブを読んで度数表記 (`IVM7` など) を実音に直し、ディレクティブとコメントのノードを落とす。`self` を consume する。
- **`ChordNode::to_chord()`**: `Chord { key, semitones: BTreeSet<u8>, on, .. }` を返す。`semitones` は**ルートからの半音数**（ルート自身が 0）。実際の音は `12 * octave + root + semitone` で作られる（`chord_midi/src/export/midi.rs:27-32` 参照）。なので**ピッチクラスは `(root + semitone) % 12`**。
- **弦の並び**: このコードベースでは押さえ方の配列を一貫して **6弦→1弦**（低い方から）で持つ。図を描くときだけ 1弦が上に来るよう逆順にする。
- **標準チューニングの開放弦のピッチクラス**: 6弦→1弦で `[4, 9, 2, 7, 11, 4]`（E A D G B E）。
- CI (`.github/workflows/gh-pages.yml`) は `trunk build` しか走らせない。`cargo fmt` / `clippy` のチェックは無い。

---

## Task 1: 押さえ方データのテーブルを作る

**Files:**
- Create: `chord_midi/src/fingering/mod.rs`
- Create: `scripts/gen_fingerings.py`
- Create (生成物): `chord_midi/src/fingering/data.rs`
- Modify: `chord_midi/src/lib.rs`
- Modify: `Justfile`
- Modify (生成物): `docs/easy_guiter_chords.json`, `docs/easy_guiter_chords.md`

- [ ] **Step 1: 型とデータ整合テストを書く**

`chord_midi/src/fingering/mod.rs` を新規作成する。この時点ではまだ `data.rs` が無いのでコンパイルは通らない。

```rust
//! ギターの押さえ方データと、コードから押さえ方を引く処理。
//!
//! データの出典は daichan easy guitar「超簡単コード一覧」
//! <https://daichan-easyguitar.com/easychord-list/>

mod data;

pub use data::FINGERINGS;

/// 標準チューニングの開放弦のピッチクラス。6弦→1弦の順
pub const OPEN_STRINGS: [u8; 6] = [4, 9, 2, 7, 11, 4];

/// 押さえる指。原典が「小指は使わない」を条件にしているので小指は無い
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Finger {
    Index,
    Middle,
    Ring,
    Thumb,
}

impl Finger {
    /// 図に描く日本語1文字
    pub fn label(&self) -> &'static str {
        match self {
            Finger::Index => "人",
            Finger::Middle => "中",
            Finger::Ring => "薬",
            Finger::Thumb => "親",
        }
    }
}

/// 1つの押さえ方。配列はすべて 6弦→1弦 の順
#[derive(Debug, Clone, PartialEq)]
pub struct Fingering {
    /// 原典でのコード名。表示用で、照合には使わない
    pub name: &'static str,
    /// 原典のセクション名 ("Cコード系" など)
    pub group: &'static str,
    /// "x3201x" 形式
    pub tab: &'static str,
    /// None = ミュート、Some(0) = 開放
    pub frets: [Option<u8>; 6],
    /// 押さえる弦だけ Some。開放弦とミュート弦は None
    pub fingers: [Option<Finger>; 6],
    /// 鳴る音のピッチクラス集合。bit n が立っていればピッチクラス n を含む
    pub pitch_classes: u16,
    /// 最低音（鳴る弦のうち一番低い弦）のピッチクラス
    pub bass: u8,
}

impl Fingering {
    /// 押さえるフレットの最小値。開放とミュートだけなら None
    pub fn min_fret(&self) -> Option<u8> {
        self.frets.iter().flatten().copied().filter(|f| *f > 0).min()
    }

    /// 押さえるフレットの最大値。開放とミュートだけなら None
    pub fn max_fret(&self) -> Option<u8> {
        self.frets.iter().flatten().copied().filter(|f| *f > 0).max()
    }

    /// 鳴る弦の数（ミュートでない弦）
    pub fn sounding_strings(&self) -> usize {
        self.frets.iter().filter(|f| f.is_some()).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_is_consistent() {
        assert_eq!(FINGERINGS.len(), 67);
        for f in FINGERINGS {
            let tab: String = f
                .frets
                .iter()
                .map(|v| match v {
                    None => "x".to_string(),
                    Some(n) => n.to_string(),
                })
                .collect();
            assert_eq!(tab, f.tab, "tab mismatch: {}", f.name);

            let mut pcs = 0u16;
            for (i, v) in f.frets.iter().enumerate() {
                if let Some(n) = *v {
                    pcs |= 1u16 << ((OPEN_STRINGS[i] + n) % 12);
                }
            }
            assert_eq!(pcs, f.pitch_classes, "pitch_classes mismatch: {}", f.name);

            let bass = f
                .frets
                .iter()
                .enumerate()
                .find_map(|(i, v)| v.map(|n| (OPEN_STRINGS[i] + n) % 12))
                .expect("no sounding string");
            assert_eq!(bass, f.bass, "bass mismatch: {}", f.name);
        }
    }

    #[test]
    fn fingers_cover_pressed_strings() {
        for f in FINGERINGS {
            let mut used: Vec<Finger> = Vec::new();
            for (i, v) in f.frets.iter().enumerate() {
                match *v {
                    Some(n) if n > 0 => {
                        let g = f.fingers[i]
                            .unwrap_or_else(|| panic!("no finger: {} string {}", f.name, 6 - i));
                        assert!(!used.contains(&g), "finger used twice: {}", f.name);
                        used.push(g);
                    }
                    _ => assert!(
                        f.fingers[i].is_none(),
                        "finger on open/muted string: {}",
                        f.name
                    ),
                }
            }
        }
    }

    #[test]
    fn known_fingerings() {
        // C = x3201x。人=2弦1F / 中=4弦2F / 薬=5弦3F
        let c = FINGERINGS.iter().find(|f| f.tab == "x3201x").unwrap();
        assert_eq!(c.fingers[4], Some(Finger::Index));
        assert_eq!(c.fingers[2], Some(Finger::Middle));
        assert_eq!(c.fingers[1], Some(Finger::Ring));

        // F#m7-5 = 2x221x。6弦は親指、2弦1F が人差し指
        let fsm = FINGERINGS.iter().find(|f| f.tab == "2x221x").unwrap();
        assert_eq!(fsm.fingers[0], Some(Finger::Thumb));
        assert_eq!(fsm.fingers[4], Some(Finger::Index));

        // G = 3x000x。1本指のときは中指
        let g = FINGERINGS.iter().find(|f| f.tab == "3x000x").unwrap();
        assert_eq!(g.fingers[0], Some(Finger::Middle));
    }
}
```

`chord_midi/src/lib.rs` にモジュールを足す。

```rust
pub mod edit;
pub mod export;
pub mod fingering;
pub mod import;
pub mod model;
```

- [ ] **Step 2: テストが失敗することを確認する**

Run: `cargo test -p chord_midi fingering`
Expected: コンパイルエラー。`error[E0583]: file not found for module `data``

- [ ] **Step 3: 生成スクリプトを書く**

`scripts/gen_fingerings.py` を新規作成する。

```python
#!/usr/bin/env python3
"""docs/easy_guiter_chords.json から運指を導出し、Rust のテーブルと docs を生成する。

使い方:
    just gen-fingerings      (= python3 scripts/gen_fingerings.py)

出力:
    chord_midi/src/fingering/data.rs   Rust の const テーブル
    docs/easy_guiter_chords.json       fingers フィールドを追記
    docs/easy_guiter_chords.md         運指の列を追記
"""
import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parent.parent
JSON_PATH = ROOT / "docs" / "easy_guiter_chords.json"
MD_PATH = ROOT / "docs" / "easy_guiter_chords.md"
RS_PATH = ROOT / "chord_midi" / "src" / "fingering" / "data.rs"

PC = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"]
# 標準チューニングの開放弦のピッチクラス。6弦→1弦
OPEN_STRINGS = [4, 9, 2, 7, 11, 4]
STRING_LABEL = ["6弦", "5弦", "4弦", "3弦", "2弦", "1弦"]
FINGER_RS = {
    "index": "Finger::Index",
    "middle": "Finger::Middle",
    "ring": "Finger::Ring",
    "thumb": "Finger::Thumb",
}
FINGER_JP = {"index": "人", "middle": "中", "ring": "薬", "thumb": "親"}


def assign_fingers(frets):
    """frets は 6弦→1弦。None=ミュート、0=開放。同じ並びの指名リストを返す。

    規則:
      1. 押さえる音をフレット昇順 → 弦番号 6弦→1弦 の順に並べる
      2. 4音のときは6弦を親指にし、残り3音を 人 → 中 → 薬
      3. 2〜3音のときは 人 → 中 → 薬
      4. 1音だけのときは中指（人差し指はミュート用に空ける）
    """
    fingers = [None] * 6
    pressed = [(i, f) for i, f in enumerate(frets) if f not in (None, 0)]
    if not pressed:
        return fingers
    if len(pressed) > 4:
        raise ValueError(f"too many pressed strings: {frets}")
    if len(pressed) == 1:
        fingers[pressed[0][0]] = "middle"
        return fingers
    rest = pressed
    if len(pressed) == 4:
        if pressed[0][0] != 0:
            raise ValueError(f"4 pressed strings without the 6th string: {frets}")
        fingers[0] = "thumb"
        rest = pressed[1:]
    ordered = sorted(rest, key=lambda p: (p[1], p[0]))
    for finger, (string, _) in zip(["index", "middle", "ring"], ordered):
        fingers[string] = finger
    return fingers


def pitch_classes(frets):
    pcs = 0
    for i, f in enumerate(frets):
        if f is not None:
            pcs |= 1 << ((OPEN_STRINGS[i] + f) % 12)
    return pcs


def bass(frets):
    for i, f in enumerate(frets):
        if f is not None:
            return (OPEN_STRINGS[i] + f) % 12
    raise ValueError(f"no sounding string: {frets}")


def notes(frets):
    return [PC[(OPEN_STRINGS[i] + f) % 12] for i, f in enumerate(frets) if f is not None]


def detail(frets):
    parts = []
    for i, f in enumerate(frets):
        if f is None:
            parts.append(f"{STRING_LABEL[i]}ミュート")
        elif f == 0:
            parts.append(f"{STRING_LABEL[i]}開放")
        else:
            parts.append(f"{STRING_LABEL[i]}{f}F")
    return " / ".join(parts)


def finger_detail(frets, fingers):
    parts = [
        f"{FINGER_JP[fingers[i]]}={STRING_LABEL[i]}{f}F"
        for i, f in enumerate(frets)
        if f not in (None, 0)
    ]
    return " / ".join(parts) if parts else "—"


def opt_u8(v):
    return "None" if v is None else f"Some({v})"


def opt_finger(v):
    return "None" if v is None else f"Some({FINGER_RS[v]})"


def build_markdown(data):
    md = []
    md.append("# 超簡単コード一覧（押さえ方）\n")
    md.append("出典: <https://daichan-easyguitar.com/easychord-list/>\n")
    md.append(
        "> このファイルは `scripts/gen_fingerings.py` が "
        "`easy_guiter_chords.json` から生成する。手で編集しない。\n"
    )
    md.append("## 条件\n")
    md.append("- バレーコードなし\n- 指1~3本で抑えられる\n- 小指は使わない\n")
    md.append("## 記法\n")
    md.append(
        "- 押さえ方は **6弦→1弦** の順で表記（例: `x32010`）\n"
        "- 数字はフレット番号、`0` は開放弦、`x` はミュート\n"
        "- チューニングは標準 (E A D G B E)\n"
        "- 運指は 人=人差し指 / 中=中指 / 薬=薬指 / 親=親指。"
        "原典に指の指定が無いため規則で導出している\n"
    )
    md.append(
        "同じコード名が複数ある場合は、原典に載っている複数の押さえ方"
        "（難易度・響きの違い）を並べたもの。\n"
    )
    md.append(
        "HTML 表示用の同じデータは "
        "[`easy_guiter_chords.json`](./easy_guiter_chords.json) にある。\n"
    )
    for group in data["groups"]:
        md.append(f"## {group['name']}\n")
        md.append("| コード | 押さえ方 | 構成音 | 運指 | 詳細 |")
        md.append("| --- | --- | --- | --- | --- |")
        for c in group["chords"]:
            fr = c["frets"]
            md.append(
                f"| {c['name']} | `{c['tab']}` | {', '.join(notes(fr))} "
                f"| {finger_detail(fr, c['fingers'])} | {detail(fr)} |"
            )
        md.append("")
    md.append("## 参考文献\n")
    md.append(
        "- <https://daichan-easyguitar.com/what-is-easychord/>\n"
        "- <https://daichan-easyguitar.com/easychord-list/>\n"
    )
    return "\n".join(md)


def build_rust(data):
    lines = [
        "// @generated by scripts/gen_fingerings.py from docs/easy_guiter_chords.json",
        "// 手で編集しない。`just gen-fingerings` で再生成する。",
        "use super::{Finger, Fingering};",
        "",
        "pub static FINGERINGS: &[Fingering] = &[",
    ]
    for group in data["groups"]:
        for c in group["chords"]:
            fr = c["frets"]
            b = bass(fr)
            lines.append("    Fingering {")
            lines.append(f'        name: "{c["name"]}",')
            lines.append(f'        group: "{group["name"]}",')
            lines.append(f'        tab: "{c["tab"]}",')
            lines.append("        frets: [" + ", ".join(opt_u8(v) for v in fr) + "],")
            lines.append(
                "        fingers: [" + ", ".join(opt_finger(v) for v in c["fingers"]) + "],"
            )
            lines.append(
                f"        pitch_classes: 0b{pitch_classes(fr):012b}, // {', '.join(notes(fr))}"
            )
            lines.append(f"        bass: {b}, // {PC[b]}")
            lines.append("    },")
    lines.append("];")
    return "\n".join(lines) + "\n"


def main():
    data = json.loads(JSON_PATH.read_text(encoding="utf-8"))

    for group in data["groups"]:
        for chord in group["chords"]:
            chord["fingers"] = assign_fingers(chord["frets"])

    data["fingersNotation"] = (
        "fingers は frets と同じ 6弦→1弦 の順。"
        "index=人差し指 / middle=中指 / ring=薬指 / thumb=親指、null は押さえない弦。"
    )

    JSON_PATH.write_text(
        json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    MD_PATH.write_text(build_markdown(data), encoding="utf-8")
    RS_PATH.parent.mkdir(parents=True, exist_ok=True)
    RS_PATH.write_text(build_rust(data), encoding="utf-8")

    total = sum(len(g["chords"]) for g in data["groups"])
    print(f"wrote {total} fingerings")


if __name__ == "__main__":
    main()
```

`Justfile` の末尾に足す。既存のレシピと同じく4スペースインデント。

```
gen-fingerings:
    python3 scripts/gen_fingerings.py
    cargo fmt -p chord_midi
```

- [ ] **Step 4: 生成を走らせる**

Run: `just gen-fingerings`
Expected: `wrote 67 fingerings` と出て、`chord_midi/src/fingering/data.rs` が生成される。

生成物を目視で確認する。Run: `head -20 chord_midi/src/fingering/data.rs`
Expected: 先頭が `// @generated by ...` で、最初のエントリが

```rust
    Fingering {
        name: "C",
        group: "Cコード系",
        tab: "x3201x",
        frets: [None, Some(3), Some(2), Some(0), Some(1), None],
        fingers: [None, Some(Finger::Ring), Some(Finger::Middle), None, Some(Finger::Index), None],
        pitch_classes: 0b000010010001, // C, E, G, C
        bass: 0, // C
    },
```

- [ ] **Step 5: テストが通ることを確認する**

Run: `cargo test -p chord_midi fingering`
Expected: PASS（`data_is_consistent` / `fingers_cover_pressed_strings` / `known_fingerings` の3つ）

- [ ] **Step 6: コミット**

```bash
git add scripts/gen_fingerings.py Justfile chord_midi/src/lib.rs chord_midi/src/fingering/ docs/easy_guiter_chords.json docs/easy_guiter_chords.md
git commit -m "feat(fingering): 押さえ方データのテーブルと生成スクリプト

docs/easy_guiter_chords.json から運指を導出して Rust の const テーブルを
生成する。運指はフレット昇順→低音弦優先で人→中→薬を割り当て、4音の
2件は原典の解説どおり6弦を親指にする。"
```

---

## Task 2: AST からコード列を取り出す

**Files:**
- Modify: `chord_midi/src/fingering/mod.rs`

- [ ] **Step 1: 失敗するテストを書く**

`chord_midi/src/fingering/mod.rs` の `mod tests` に足す。ファイル先頭の `use` にも追記する。

先頭の `use`（`mod data;` の下）:

```rust
use crate::model::{
    ast::{Ast, ChordNode, Node},
    key::Key,
    pitch::Pitch,
    scale::Scale,
};
use anyhow::Result;
```

テスト:

```rust
    fn refs(input: &str) -> Vec<ChordRef> {
        use crate::import::{Importer, RechordImporter};
        let ast = RechordImporter.import(input).unwrap().resolve();
        chord_refs(&ast).unwrap()
    }

    /// ピッチクラスの集合をビットマスクにする
    fn mask(pcs: &[u8]) -> u16 {
        pcs.iter().fold(0u16, |acc, p| acc | (1u16 << p))
    }

    #[test]
    fn chord_refs_extracts_pitch_classes() {
        let r = refs("C |\n");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].label, "C");
        assert_eq!(r[0].root, 0);
        assert_eq!(r[0].pitch_classes, mask(&[0, 4, 7]));
        assert_eq!(r[0].bass, None);
    }

    #[test]
    fn chord_refs_resolves_degrees() {
        // key: C なので IV は F
        let r = refs("key: C\nIVM7 |\n");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].root, 5);
        assert_eq!(r[0].pitch_classes, mask(&[5, 9, 0, 4]));
    }

    #[test]
    fn chord_refs_keeps_slash_bass() {
        // Am7 = A C E G、ベースは D
        let r = refs("Am7/D |\n");
        assert_eq!(r[0].root, 9);
        assert_eq!(r[0].bass, Some(2));
        assert_eq!(r[0].pitch_classes, mask(&[9, 0, 4, 7, 2]));
    }

    #[test]
    fn chord_refs_skips_non_chord_nodes() {
        // = は sustain、N.C. は rest、% は repeat
        let r = refs("C = N.C. % |\n");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].label, "C");
    }

    #[test]
    fn chord_refs_expands_transforms() {
        // s25(G7) は Dm7 G7 の2コードに展開される
        let r = refs("s25(G7) |\n");
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].root, 2);
        assert_eq!(r[1].root, 7);
    }

    #[test]
    fn chord_refs_reads_all_measures() {
        let r = refs("C | F |\n");
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].root, 0);
        assert_eq!(r[1].root, 5);
    }
```

- [ ] **Step 2: テストが失敗することを確認する**

Run: `cargo test -p chord_midi fingering`
Expected: コンパイルエラー。`cannot find type `ChordRef` in this scope` と `cannot find function `chord_refs``

- [ ] **Step 3: 実装する**

`chord_midi/src/fingering/mod.rs` の `impl Fingering { ... }` の後ろ、`#[cfg(test)]` の前に足す。

```rust
/// 入力に現れた1つのコード。押さえ方を引くための情報だけを持つ
#[derive(Debug, Clone, PartialEq)]
pub struct ChordRef {
    /// 表示用のコード名
    pub label: String,
    /// ルートのピッチクラス。重み付けの基準にする
    pub root: u8,
    /// 構成音（スラッシュのベースを含む）のピッチクラス集合
    pub pitch_classes: u16,
    /// スラッシュコードのベース。無ければ None
    pub bass: Option<u8>,
}

/// AST に現れるコードを出現順に取り出す。
///
/// 呼ぶ前に [`Ast::resolve`] を通しておくこと。度数表記のままだと
/// ピッチクラスが決まらずエラーになる。
pub fn chord_refs(ast: &Ast) -> Result<Vec<ChordRef>> {
    let mut out = Vec::new();
    collect_ast(ast, &mut out)?;
    Ok(out)
}

fn collect_ast(ast: &Ast, out: &mut Vec<ChordRef>) -> Result<()> {
    match ast {
        Ast::Score(items) => {
            for item in items {
                collect_ast(item, out)?;
            }
        }
        Ast::Measure(nodes, _) => {
            for node in nodes {
                collect_node(node, out)?;
            }
        }
        Ast::Comment(_) | Ast::Directive(_) => {}
    }
    Ok(())
}

fn collect_node(node: &Node, out: &mut Vec<ChordRef>) -> Result<()> {
    match node {
        Node::Chord(c) | Node::Arpeggio(c) => out.push(chord_ref(c)?),
        Node::Transform(t, c) => {
            // resolve() 済みなら key は Absolute なので tonic/scale は無視される。
            // export/midi.rs の Node::Transform の扱いと同じ。
            for expanded in t.expand(c.clone(), Pitch::C, Scale::Major) {
                collect_node(&expanded, out)?;
            }
        }
        Node::Rest | Node::Sustain | Node::Repeat => {}
    }
    Ok(())
}

fn chord_ref(node: &ChordNode) -> Result<ChordRef> {
    let label = node.to_string();
    let chord = node.to_chord()?;
    let root = pitch_class(&chord.key)?;
    let mut pitch_classes = 0u16;
    for semitone in &chord.semitones {
        pitch_classes |= 1u16 << ((root as u16 + *semitone as u16) % 12);
    }
    let bass = match &chord.on {
        Some(on) => {
            let b = pitch_class(on)?;
            pitch_classes |= 1u16 << b;
            Some(b)
        }
        None => None,
    };
    Ok(ChordRef {
        label,
        root,
        pitch_classes,
        bass,
    })
}

fn pitch_class(key: &Key) -> Result<u8> {
    match key {
        Key::Absolute(p) => Ok(*p as u8),
        Key::Relative(d) => Err(anyhow::anyhow!(
            "relative key {} is not resolved; call Ast::resolve() first",
            d
        )),
    }
}
```

- [ ] **Step 4: テストが通ることを確認する**

Run: `cargo test -p chord_midi fingering`
Expected: PASS（Task 1 の3つ + 今回の6つ）

- [ ] **Step 5: コミット**

```bash
git add chord_midi/src/fingering/mod.rs
git commit -m "feat(fingering): AST からコード列を取り出す chord_refs

resolve() 済みの AST を走査して、コード名・ルート・構成音の
ピッチクラス集合・スラッシュベースを取り出す。Transform は
展開し、Rest / Sustain / Repeat は飛ばす。"
```

---

## Task 3: コードから押さえ方を引く

**Files:**
- Modify: `chord_midi/src/fingering/mod.rs`

- [ ] **Step 1: 失敗するテストを書く**

`mod tests` に足す。

```rust
    fn tab_of(input: &str) -> &'static str {
        let r = refs(input);
        find(&r[0]).expect("no fingering found").tab
    }

    #[test]
    fn find_picks_the_matching_shape() {
        assert_eq!(tab_of("C |\n"), "x3201x");
        assert_eq!(tab_of("D |\n"), "xx0232");
        assert_eq!(tab_of("E |\n"), "022100");
        assert_eq!(tab_of("F |\n"), "xx321x");
        assert_eq!(tab_of("G |\n"), "3x000x");
        assert_eq!(tab_of("A |\n"), "x0222x");
        assert_eq!(tab_of("Am7 |\n"), "x0201x");
        assert_eq!(tab_of("Em7 |\n"), "020000");
        assert_eq!(tab_of("Dm7 |\n"), "xx0211");
    }

    #[test]
    fn find_prefers_the_fuller_chord() {
        // CM7 には C の形 (x3201x) ではなく C△7 の形が選ばれる
        assert_eq!(tab_of("CM7 |\n"), "x3x000");
    }

    #[test]
    fn find_falls_back_to_the_substitute_shape() {
        // 原典は B を B7、F#m を F#m7 で代用させる構成になっている
        assert_eq!(tab_of("B |\n"), "x2120x");
        assert_eq!(tab_of("Am |\n"), "x0201x");
        assert_eq!(tab_of("Em |\n"), "020000");
        assert_eq!(tab_of("F#m |\n"), "2x22xx");
        assert_eq!(tab_of("C#m |\n"), "x4x400");
    }

    #[test]
    fn find_rejects_two_extra_notes() {
        // Cm7 (x3x34x) は C に対して Eb と Bb の2音が余分なので候補に入らない
        assert_ne!(tab_of("C |\n"), "x3x34x");
    }

    #[test]
    fn find_honors_slash_bass() {
        assert_eq!(tab_of("D/F# |\n"), "2x023x");
    }

    #[test]
    fn find_returns_none_when_nothing_fits() {
        // C# 1音だけの集合に対しては、どの押さえ方も余分な音が2つ以上になる
        let c = ChordRef {
            label: "test".to_string(),
            root: 1,
            pitch_classes: 1u16 << 1,
            bass: None,
        };
        assert!(find(&c).is_none());
    }
```

- [ ] **Step 2: テストが失敗することを確認する**

Run: `cargo test -p chord_midi fingering`
Expected: コンパイルエラー。`cannot find function `find` in this scope`

- [ ] **Step 3: 実装する**

`chord_refs` の下、`fn pitch_class` の後ろに足す。

```rust
/// ビット集合に立っているピッチクラスを、root からの半音数にして返す
fn relative_semitones(mask: u16, root: u8) -> impl Iterator<Item = u8> {
    (0..12u8)
        .filter(move |p| ((mask >> *p) & 1) == 1)
        .map(move |p| (p + 12 - root) % 12)
}

/// 構成音にあるのに押さえ方に無い音のコスト。ルートからの半音数で引く
fn missing_cost(semitone: u8) -> u32 {
    match semitone {
        0 => 4,       // ルート
        3 | 4 => 3,   // 3rd
        10 | 11 => 2, // 7th
        _ => 1,       // 5th とテンション
    }
}

/// 押さえ方にあるのに構成音に無い音のコスト。ルートからの半音数で引く
fn extra_cost(semitone: u8) -> u32 {
    match semitone {
        10 | 11 | 2 | 5 | 9 => 1, // b7 / M7 / 9th / 11th / 13th は自然に足せる
        0 | 7 => 2,               // ルート / 5th
        3 | 4 => 3,               // 3rd
        _ => 4,                   // b9 / b5 / b6 はコードの性格を壊す
    }
}

/// コードに一番合う押さえ方を返す。
///
/// 構成音に無い音を1つまで許容する。原典は `B` を `B7`、`F#m` を `F#m7` で
/// 代用させる構成なので、部分集合だけに絞ると素の三和音が軒並み引けなくなる。
pub fn find(chord: &ChordRef) -> Option<&'static Fingering> {
    let expected_bass = chord.bass.unwrap_or(chord.root);
    FINGERINGS
        .iter()
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

`min_by_key` は同点のとき最初の要素を返すので、`FINGERINGS` の並び順（原典の掲載順 = 易しい順）が最後の tie-break になる。

- [ ] **Step 4: テストが通ることを確認する**

Run: `cargo test -p chord_midi`
Expected: PASS。`fingering` の15テストに加えて既存のテストも全部通ること。

- [ ] **Step 5: コミット**

```bash
git add chord_midi/src/fingering/mod.rs
git commit -m "feat(fingering): 構成音からの押さえ方照合 find

構成音に無い音を1音まで許容し、欠け音と余分音に重みを付けて
コスト最小の押さえ方を選ぶ。同点はベース一致 → 鳴る弦の多さ →
ローポジション → 原典の掲載順で決める。"
```

---

## Task 4: web にダイアグラムを描く

**Files:**
- Create: `chord_midi_web/src/diagram.rs`
- Create: `chord_midi_web/style.css`
- Modify: `chord_midi_web/index.html`
- Modify: `chord_midi_web/src/main.rs`

このタスクには自動テストが無い（`chord_midi_web` はワークスペース除外の wasm バイナリで、DOM を触るテストの土台が無い）。照合と運指のロジックは Task 1〜3 で host 側にテスト済み。ここは `cargo check --target wasm32-unknown-unknown` が通ることと、ブラウザでの目視確認で担保する。

- [ ] **Step 1: コンポーネントを書く**

`chord_midi_web/src/diagram.rs` を新規作成する。

```rust
use chord_midi::fingering::Fingering;
use yew::prelude::*;

/// 一度に描くフレット数
const FRET_COUNT: u8 = 5;

#[derive(Properties, PartialEq)]
pub struct ChordDiagramProps {
    /// 入力に書かれたコード名
    pub label: String,
    /// 見つかった押さえ方。無ければ None
    pub fingering: Option<&'static Fingering>,
    /// カードの色 (CSS の色文字列)
    pub color: String,
}

#[function_component(ChordDiagram)]
pub fn chord_diagram(props: &ChordDiagramProps) -> Html {
    let body = match props.fingering {
        Some(f) => diagram(f),
        None => html! { <div class="diagram-empty">{"データなし"}</div> },
    };
    // 代替の押さえ方が選ばれたときだけ原典での名前を出す
    let alias = props.fingering.and_then(|f| {
        if f.name == props.label {
            None
        } else {
            Some(html! { <div class="chord-alias">{format!("≈ {}", f.name)}</div> })
        }
    });
    html! {
        <div class="chord-card" style={format!("--chord-color: {}", props.color)}>
            <div class="chord-name">{&props.label}</div>
            {alias}
            {body}
        </div>
    }
}

/// 描き始めるフレット。5フレットに収まらないときだけ最小フレットから始める
fn window_start(f: &Fingering) -> u8 {
    match (f.min_fret(), f.max_fret()) {
        (Some(min), Some(max)) if max > FRET_COUNT => min,
        _ => 1,
    }
}

fn diagram(f: &Fingering) -> Html {
    let start = window_start(f);
    // 1弦を上に描く。frets は 6弦→1弦 の順なので逆から回す
    let rows = (0..6usize)
        .rev()
        .map(|s| {
            let marker = match f.frets[s] {
                None => "×",
                Some(0) => "○",
                Some(_) => "",
            };
            let cells = (0..FRET_COUNT)
                .map(|i| {
                    let fret = start + i;
                    let dot = match f.frets[s] {
                        Some(v) if v == fret => html! {
                            <span class="dot">
                                {f.fingers[s].map(|g| g.label()).unwrap_or("")}
                            </span>
                        },
                        _ => Html::default(),
                    };
                    // 各行の最初のセルの左端がナット
                    let class = if i == 0 { "cell nut" } else { "cell" };
                    html! { <div {class}>{dot}</div> }
                })
                .collect::<Html>();
            html! {
                <>
                    <div class="marker">{marker}</div>
                    {cells}
                </>
            }
        })
        .collect::<Html>();

    html! {
        <div class="diagram">
            <div class="grid">{rows}</div>
            if start > 1 {
                <div class="fret-label">{format!("{}fr", start)}</div>
            }
        </div>
    }
}
```

- [ ] **Step 2: CSS を書く**

`chord_midi_web/style.css` を新規作成する。

```css
body {
    font-family: sans-serif;
    margin: 16px;
}

.fingerings {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    margin: 12px 0;
}

.chord-card {
    border: 2px solid var(--chord-color, #333);
    border-radius: 6px;
    padding: 6px;
    background: #fff;
}

.chord-name {
    background: var(--chord-color, #333);
    color: #fff;
    font-weight: bold;
    font-size: 13px;
    text-align: center;
    border-radius: 3px;
    padding: 2px 8px;
    white-space: nowrap;
}

.chord-alias {
    font-size: 10px;
    color: #666;
    text-align: center;
    margin-top: 2px;
}

.diagram {
    display: flex;
    align-items: flex-start;
    gap: 3px;
    margin-top: 6px;
}

.grid {
    display: grid;
    grid-template-columns: 14px repeat(5, 22px);
    grid-auto-rows: 20px;
}

.marker {
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 11px;
    line-height: 1;
}

/* 弦はセルの中央を横切る線、フレットはセルの右の縦線 */
.cell {
    position: relative;
    border-right: 1px solid #666;
    background: linear-gradient(#666, #666) 0 50% / 100% 1px no-repeat;
}

.cell.nut {
    border-left: 3px solid #000;
}

.dot {
    position: absolute;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: #000;
    color: #fff;
    font-size: 10px;
    line-height: 16px;
    text-align: center;
}

.fret-label {
    font-size: 10px;
    color: #333;
    margin-top: 2px;
}

/* 押さえ方が無いときも幅と高さを揃えて並びを崩さない */
.diagram-empty {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 124px;
    height: 120px;
    margin-top: 6px;
    color: #999;
    font-size: 11px;
    border: 1px dashed #bbb;
}
```

`chord_midi_web/index.html` の `<head>` に `<link>` を足す。他の行は触らない。

```html
<!DOCTYPE html>
<html lang="en">
    <head>
        <link data-trunk rel="css" href="style.css" />
    </head>
    <body></body>
    <script src="https://cdn.jsdelivr.net/combine/npm/tone@14.7.58,npm/@magenta/music@1.23.1/es6/core.js,npm/focus-visible@5,npm/html-midi-player@1.5.0"></script>
</html>
```

- [ ] **Step 3: `main.rs` を繋ぐ**

`chord_midi_web/src/main.rs` を4箇所直す。

(a) ファイル冒頭の `use` とモジュール宣言。`chord_midi::` の import に `fingering` を足し、`mod presets;` の隣に `mod diagram;` を足す。

```rust
use chord_midi::{
    edit::{set_scale, shift_key},
    export::{Exporter, MidiExporter},
    fingering::{self, ChordRef},
    import::{Importer, RechordImporter},
    model::scale::Scale,
};
use diagram::ChordDiagram;
```

```rust
mod diagram;
mod presets;
```

(b) `compile` の戻り値にコード列を足す。関数まるごと差し替える。

```rust
fn compile(input: &str, bpm: u8) -> Result<(String, Vec<ChordRef>, Vec<u8>), String> {
    let ast = RechordImporter
        .import(&format!("{}\n", input))
        .map_err(|e| e.to_string())?;
    // AST 表示は度数表記のまま出したいので resolve する前に文字列化する
    let ast_str = format!("{}", ast);
    // resolve() も export() も self を consume するので、解決済みの AST を
    // 使い回す。MidiExporter は内部でもう一度 resolve() を呼ぶが冪等。
    let resolved = ast.resolve();
    let chords = fingering::chord_refs(&resolved).map_err(|e| e.to_string())?;
    let mut writer = BufWriter::new(Vec::new());
    MidiExporter { bpm }
        .export(&mut writer, resolved)
        .map_err(|e| e.to_string())?;
    let bytes = writer.into_inner().map_err(|e| e.to_string())?;
    Ok((ast_str, chords, bytes))
}
```

(c) state を1つ足し、再計算の effect で更新する。`let result = use_state(String::new);` の下に足す。

```rust
    let chords = use_state(Vec::<ChordRef>::new);
```

effect のブロック（`// input か bpm が変わるたびに再計算` のところ）で、クローンの列に `chords` を足す。

```rust
        let result = result.clone();
        let chords = chords.clone();
        let error = error.clone();
```

空入力の分岐（`if input.trim().is_empty() {` の中）で、`result.set(String::new());` の直後に足す。

```rust
                chords.set(Vec::new());
```

`match compile(...)` を新しいタプルに合わせる。

```rust
            match compile(input, *bpm) {
                Ok((ast_str, cs, bytes)) => {
                    error.set(String::new());
                    result.set(ast_str);
                    chords.set(cs);
                    let blob = new_blob(&bytes, "audio/midi");
```

エラー分岐も差し替える。

```rust
                Err(e) => {
                    error.set(e);
                    chords.set(Vec::new());
                }
```

(d) カードを描く。`html!` の中、プリセットボタンの `</div>` の直後、`MIDIを保存` の `<div>` の前に挿む。

まず `html!` の手前（コールバック定義の後ろ）でカードを作る。

```rust
    let cards = {
        // 出現するコード名ごとに色を割り当てる。同じコードは常に同じ色
        const PALETTE: [&str; 8] = [
            "#c0392b", "#d35400", "#b7950b", "#1e8449", "#1f618d", "#6c3483", "#a93226",
            "#117864",
        ];
        let mut labels: Vec<String> = Vec::new();
        let mut cards: Vec<Html> = Vec::new();
        for c in chords.iter() {
            let index = match labels.iter().position(|l| l == &c.label) {
                Some(i) => i,
                None => {
                    labels.push(c.label.clone());
                    labels.len() - 1
                }
            };
            cards.push(html! {
                <ChordDiagram
                    label={c.label.clone()}
                    fingering={fingering::find(c)}
                    color={PALETTE[index % PALETTE.len()].to_string()}
                />
            });
        }
        cards
    };
```

`html!` の中に足す。

```rust
            <div class="fingerings">{cards}</div>
```

- [ ] **Step 4: wasm 向けにビルドが通ることを確認する**

Run: `cd chord_midi_web && cargo check --target wasm32-unknown-unknown`
Expected: `Finished` で終わる。warning は出ても良いが error はゼロ。

- [ ] **Step 5: 実際の画面で確認する**

Run: `cd chord_midi_web && trunk serve`
（`trunk` が入っていなければ `cargo install --locked trunk` か、CI と同じ `trunk build` だけでも良い）

ブラウザで開いて、入力欄に次を貼って目視で確認する。

```
key: C
C | Am | F | G |
```

確認する点:
1. 4枚のカードが左から `C` `Am` `F` `G` の順に並ぶ
2. `C` のダイアグラムが 6弦×、5弦3F(薬)、4弦2F(中)、3弦開放○、2弦1F(人)、1弦× になっている（1弦が一番上の行）
3. `Am` のカードに `≈ Am7` と小さく出る
4. 4枚の枠とヘッダの色が全部違う
5. `C | C | F |` と入れると1枚目と2枚目の `C` が同じ色になる
6. 入力を空にするとカードが消える
7. 壊れた入力（例 `zzz |`）でエラーが出ている間はカードが消える
8. `C#M7 |` を入れると、5フレットに収まらない押さえ方 (`x4x56x`) なので `4fr` とフレット番号が出る

- [ ] **Step 6: コミット**

```bash
git add chord_midi_web/src/diagram.rs chord_midi_web/style.css chord_midi_web/index.html chord_midi_web/src/main.rs
git commit -m "feat(web): 入力コードの押さえ方をダイアグラムで表示

入力に現れたコードを出現順にカードで並べ、押さえ方と運指を
HTML/CSS で描く。コードの種類ごとに色を割り当て、原典と違う
名前の押さえ方が選ばれたときは ≈ 付きで併記する。"
```

---

## Task 5: 仕上げ

**Files:**
- Modify: `README.md`

- [ ] **Step 1: README に生成コマンドを書く**

`README.md` の末尾に足す。既存の内容は消さない。

```markdown
## 押さえ方データ

ギターの押さえ方は `docs/easy_guiter_chords.json` が原本。
運指の導出と Rust テーブルの生成は次で行う。

    just gen-fingerings

`chord_midi/src/fingering/data.rs` は生成物なので手で編集しない。
```

- [ ] **Step 2: 全部のテストを流す**

Run: `cargo test`
Expected: 全部 PASS

Run: `cd chord_midi_web && cargo check --target wasm32-unknown-unknown`
Expected: error ゼロ

- [ ] **Step 3: コミットして PR を出す**

```bash
git add README.md
git commit -m "docs: 押さえ方データの生成コマンドを README に追記"
git push -u origin feat/chord-fingering-diagram
gh pr create --fill
```

PR 本文には `Closes #17` を入れる。
