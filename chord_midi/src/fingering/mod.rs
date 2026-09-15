//! ギターの押さえ方データと、コードから押さえ方を引く処理。
//!
//! データの出典は daichan easy guitar「超簡単コード一覧」
//! <https://daichan-easyguitar.com/easychord-list/>

mod data;

pub use data::FINGERINGS;

use crate::model::{
    ast::{Ast, ChordNode, Node},
    key::Key,
    pitch::Pitch,
    scale::Scale,
};
use anyhow::Result;

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
        self.frets
            .iter()
            .flatten()
            .copied()
            .filter(|f| *f > 0)
            .min()
    }

    /// 押さえるフレットの最大値。開放とミュートだけなら None
    pub fn max_fret(&self) -> Option<u8> {
        self.frets
            .iter()
            .flatten()
            .copied()
            .filter(|f| *f > 0)
            .max()
    }

    /// 鳴る弦の数（ミュートでない弦）
    pub fn sounding_strings(&self) -> usize {
        self.frets.iter().filter(|f| f.is_some()).count()
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

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
