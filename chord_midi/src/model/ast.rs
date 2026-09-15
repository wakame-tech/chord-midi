use crate::model::{
    chord::{match_pitches, Chord},
    key::Key,
    modifier::Modifier,
    pitch::Pitch,
    scale::{Degree, Scale},
};
use anyhow::Result;
use std::collections::BTreeSet;

#[derive(Debug, PartialEq)]
pub enum Directive {
    Key(Pitch),
    Scale(Scale),
}

#[derive(Debug, PartialEq)]
pub enum Ast {
    Comment(String),
    Directive(Directive),
    // nodes, br?
    Measure(Vec<Node>, bool),
    /// 小節列に関数を適用する。中身は Measure と Apply（ネスト）だけ。
    /// 3つ目は「この関数の後ろで入力が改行されている」
    Apply(Func, Vec<Box<Ast>>, bool),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transform {
    S25,
    Sub,
    Dom,
    Blk,
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

impl Transform {
    /// Expand into standard nodes. tonic/scale are for Relative-key shift math
    /// (usually called after into_pitch so keys are Absolute and tonic/scale are ignored).
    pub fn expand(self, c: ChordNode, tonic: Pitch, scale: Scale) -> Vec<Node> {
        match self {
            Transform::S25 => {
                let ii = transform_chord(
                    c.key.clone().shift(7, tonic, scale),
                    &[Modifier::Minor(7)],
                    None,
                );
                vec![Node::Chord(ii), Node::Chord(c)]
            }
            // sub/dom produce a fresh dominant chord; the source chord's slash bass
            // (`on`) is intentionally discarded.
            Transform::Sub => vec![Node::Chord(transform_chord(
                c.key.shift(6, tonic, scale),
                &[Modifier::Dominant(7)],
                None,
            ))],
            Transform::Dom => vec![Node::Chord(transform_chord(
                c.key,
                &[Modifier::Dominant(7)],
                None,
            ))],
            Transform::Blk => vec![Node::Chord(transform_chord(
                c.key.clone().shift(6, tonic, scale),
                &[Modifier::Aug],
                Some(c.key),
            ))],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
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

    fn semis(node: &Node) -> Vec<u8> {
        match node {
            Node::Chord(c) => c.to_chord().unwrap().semitones.into_iter().collect(),
            _ => panic!("not a chord"),
        }
    }

    #[test]
    fn test_blk_expand() {
        // blk(F) = Baug/F. The chord's own semitones are the aug triad rooted at B
        // ({0,4,8}, root-relative, bass excluded); the F bass is carried in `on` and
        // is appended below at MIDI export. Full 4-note output {F,B,D#,G} is checked
        // e2e in test_blk_produces_one_chord.
        let f = ChordNode::absolute(Pitch::F);
        let nodes = Transform::Blk.expand(f, Pitch::C, Scale::Major);
        assert_eq!(nodes.len(), 1);
        let Node::Chord(c) = &nodes[0] else {
            panic!("not a chord")
        };
        assert_eq!(c.key, Key::Absolute(Pitch::B));
        assert_eq!(c.on, Some(Key::Absolute(Pitch::F)));
        assert_eq!(semis(&nodes[0]), vec![0, 4, 8]);
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
        let subbed = Transform::Sub.expand(g.clone(), Pitch::C, Scale::Major);
        assert_eq!(subbed.len(), 1);
        assert!(matches!(&subbed[0], Node::Chord(c) if c.key == Key::Absolute(Pitch::Cs)));
        assert_eq!(semis(&subbed[0]), vec![0, 4, 7, 10]);

        let dm7 = ChordNode {
            key: Key::Absolute(Pitch::D),
            modifiers: BTreeSet::from_iter([Modifier::Major(5), Modifier::Minor(7)]),
            on: None,
            root_string: None,
        };
        let dommed = Transform::Dom.expand(dm7, Pitch::C, Scale::Major);
        assert!(matches!(&dommed[0], Node::Chord(c) if c.key == Key::Absolute(Pitch::D)));
        assert_eq!(semis(&dommed[0]), vec![0, 4, 7, 10]);
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
        let nodes = Transform::S25.expand(g, Pitch::C, Scale::Major);
        assert_eq!(nodes.len(), 2);
        assert!(matches!(&nodes[0], Node::Chord(c) if c.key == Key::Absolute(Pitch::D)));
        assert_eq!(semis(&nodes[0]), vec![0, 3, 7, 10]);
        assert!(matches!(&nodes[1], Node::Chord(c) if c.key == Key::Absolute(Pitch::G)));
    }
}
