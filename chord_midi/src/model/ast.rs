use crate::model::{
    chord::{match_pitches, Chord},
    key::Key,
    modifier::Modifier,
    pitch::Pitch,
    scale::{Degree, Scale},
};
use anyhow::Result;
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Expr {
    Comment(String),
    /// `key:` 以降を包む
    Keyed(Pitch, Box<Expr>),
    /// `scale:` 以降を包む
    Scaled(Scale, Box<Expr>),
    /// 小節。2つ目は「この小節の後ろで入力が改行されている」
    Measure(Vec<Node>, bool),
    /// 小節列に関数を適用する。中身は必ず `Seq`。
    /// 3つ目は「この関数の後ろで入力が改行されている」
    Apply(Func, Box<Expr>, bool),
    /// `|` / 改行で連結した式の列。ファイル全体もこれ
    Seq(Vec<Expr>),
    /// `"o.ox"` のリズムパターン。`pat` の第1引数になる値
    Pattern(Pattern),
    /// `$name = ...`。本体は1行。`expand` が環境に畳んで消す。
    /// 3つ目は「この定義の後ろで入力が改行されている」。行末コメント
    /// (`$a = C |  # riff`) が続くときだけ偽になる
    Define(String, Box<Expr>, bool),
    /// `$name`。`expand` が定義の本体に置き換えて消す。
    /// 2つ目は「この参照の後ろで入力が改行されている」
    Ref(String, bool),
    /// `xN`。N は 2〜32。`expand` が中身を N 個に並べて消す。
    /// 3つ目は「このリピートの後ろで入力が改行されている」。
    /// 中身 (`Measure` / `Apply` / `Ref`) が持つ行末フラグではなく、
    /// こちらが行末の真実。`xN` は必ず中身の直後に来るので、
    /// 行が終わるのは常にリピート全体の後ろになる
    Repeat(Box<Expr>, u8, bool),
    /// `$name` を展開した跡。どの定義から来た小節かを残す。
    /// パーサは作らない。`expand` だけが作る
    Section(String, Box<Expr>),
}

/// 小節列に掛ける関数
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Func {
    /// パワーコード化
    Pow,
    /// オクターブ奏法化
    Oct,
    /// リズムパターン。引数は `Expr::Pattern` か `Expr::Ref`
    Pat(Box<Expr>),
}

/// リズムパターン。長さは 1/2/4/8/16 のいずれか
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Pattern(pub Vec<Beat>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Beat {
    /// `o` 打弦
    Hit,
    /// `.` 休符
    Rest,
    /// `x` ブラッシング
    Brush,
}

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

impl ChordMap {
    /// コード1つを別のコード1つに変える。
    ///
    /// `tonic`/`scale` は Relative なキーをずらすためのもので、
    /// `resolve()` 済み (= Absolute) なら無視される
    pub fn apply(self, c: ChordNode, tonic: Pitch, scale: Scale) -> ChordNode {
        match self {
            // sub/dom produce a fresh dominant chord; the source chord's slash bass
            // (`on`) is intentionally discarded.
            ChordMap::Sub => {
                transform_chord(c.key.shift(6, tonic, scale), &[Modifier::Dominant(7)], None)
            }
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

    /// 一番内側の素のコードへの参照
    pub fn innermost(&self) -> &ChordNode {
        match self {
            ChordExpr::Chord(c) => c,
            ChordExpr::Map(_, inner) => inner.innermost(),
        }
    }

    /// 一番内側の素のコードを書き換える。
    ///
    /// `pow`/`oct` の修飾や `key:` の解決は「元のコード」に掛かるもので、
    /// 変換の結果に掛かるものではない
    pub fn map_innermost(self, f: &mut impl FnMut(ChordNode) -> ChordNode) -> ChordExpr {
        match self {
            ChordExpr::Chord(c) => ChordExpr::Chord(f(c)),
            ChordExpr::Map(m, inner) => ChordExpr::Map(m, Box::new(inner.map_innermost(f))),
        }
    }

    /// 一番内側の素のコードへの可変参照
    pub fn innermost_mut(&mut self) -> &mut ChordNode {
        match self {
            ChordExpr::Chord(c) => c,
            ChordExpr::Map(_, inner) => inner.innermost_mut(),
        }
    }
}

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

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChordNode {
    pub key: Key,
    pub modifiers: BTreeSet<Modifier>,
    pub on: Option<Key>,
    /// `@N` で指定された「最低音を取る弦」。1 = 1弦, 6 = 6弦。
    /// 押さえ方検索にだけ効き、音程には影響しない
    pub root_string: Option<u8>,
}

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

    pub fn to_chord(&self) -> Result<Chord> {
        let mut chord = Chord::new(5, 0, self.key.clone());
        chord.on = self.on.clone();
        for modifier in &self.modifiers {
            chord.modify(modifier)?;
        }
        let (octave, inversion) = match_pitches(12 * chord.octave, &chord)?;
        chord.octave = octave;
        chord.inversion = inversion;
        Ok(chord)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::pitch::Pitch;
    use crate::model::scale::Scale;

    fn semis(chord: &ChordNode) -> Vec<u8> {
        chord.to_chord().unwrap().semitones.into_iter().collect()
    }

    #[test]
    fn test_blk_expand() {
        // blk(F) = Baug/F. The chord's own semitones are the aug triad rooted at B
        // ({0,4,8}, root-relative, bass excluded); the F bass is carried in `on` and
        // is appended below at MIDI export. Full 4-note output {F,B,D#,G} is checked
        // e2e in test_blk_produces_one_chord.
        let f = ChordNode::absolute(Pitch::F);
        let c = ChordMap::Blk.apply(f, Pitch::C, Scale::Major);
        assert_eq!(c.key, Key::Absolute(Pitch::B));
        assert_eq!(c.on, Some(Key::Absolute(Pitch::F)));
        assert_eq!(semis(&c), vec![0, 4, 8]);
    }

    #[test]
    fn test_sub_dom_expand() {
        use std::collections::BTreeSet;
        let g = ChordNode {
            key: Key::Absolute(Pitch::G),
            modifiers: BTreeSet::from_iter([Modifier::Major(5), Modifier::Dominant(7)]),
            on: None,
            root_string: None,
        };
        let subbed = ChordMap::Sub.apply(g.clone(), Pitch::C, Scale::Major);
        assert_eq!(subbed.key, Key::Absolute(Pitch::Cs));
        assert_eq!(semis(&subbed), vec![0, 4, 7, 10]);

        let dm7 = ChordNode {
            key: Key::Absolute(Pitch::D),
            modifiers: BTreeSet::from_iter([Modifier::Major(5), Modifier::Minor(7)]),
            on: None,
            root_string: None,
        };
        let dommed = ChordMap::Dom.apply(dm7, Pitch::C, Scale::Major);
        assert_eq!(dommed.key, Key::Absolute(Pitch::D));
        assert_eq!(semis(&dommed), vec![0, 4, 7, 10]);
    }

    #[test]
    fn test_s25_expand() {
        use std::collections::BTreeSet;
        let g = ChordNode {
            key: Key::Absolute(Pitch::G),
            modifiers: BTreeSet::from_iter([Modifier::Major(5), Modifier::Dominant(7)]),
            on: None,
            root_string: None,
        };
        let chords = Node::split_into(&ChordExpr::Chord(g), Pitch::C, Scale::Major);
        assert_eq!(chords.len(), 2);
        assert_eq!(chords[0].key, Key::Absolute(Pitch::D));
        assert_eq!(semis(&chords[0]), vec![0, 3, 7, 10]);
        assert_eq!(chords[1].key, Key::Absolute(Pitch::G));
    }

    #[test]
    fn test_ast_serializes_to_json() {
        use crate::import::{Importer, RechordImporter};
        let ast = RechordImporter.import("key: C\nC | Am |\n").unwrap();
        let json = serde_json::to_string(&ast).unwrap();
        // 構造が見えること。コード名とディレクティブが JSON に現れる
        assert!(json.contains("Measure"), "{}", json);
        assert!(json.contains("\"C\""), "{}", json);
    }
}
