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
    Ok(measure_refs(ast)?
        .into_iter()
        .flat_map(|m| m.chords)
        .collect())
}

/// 入力の1小節分。コードを含まない小節（休符だけなど）は `chords` が空になる
#[derive(Debug, Clone, PartialEq)]
pub struct MeasureRef {
    pub chords: Vec<ChordRef>,
    /// この小節の後ろで入力が改行されている
    pub line_break: bool,
}

/// AST に現れるコードを小節ごとにまとめて取り出す。
///
/// 呼ぶ前に [`Ast::resolve`] を通しておくこと。
pub fn measure_refs(ast: &Ast) -> Result<Vec<MeasureRef>> {
    let mut out = Vec::new();
    collect_ast(ast, &mut out)?;
    Ok(out)
}

fn collect_ast(ast: &Ast, out: &mut Vec<MeasureRef>) -> Result<()> {
    match ast {
        Ast::Score(items) => {
            for item in items {
                collect_ast(item, out)?;
            }
        }
        Ast::Measure(nodes, line_break) => {
            let mut chords = Vec::new();
            for node in nodes {
                collect_node(node, &mut chords)?;
            }
            out.push(MeasureRef {
                chords,
                line_break: *line_break,
            });
        }
        Ast::Apply(_, items, _) => {
            for item in items {
                collect_ast(item, out)?;
            }
        }
        Ast::Comment(_) | Ast::Directive(_) => {}
    }
    Ok(())
}

fn collect_node(node: &Node, out: &mut Vec<ChordRef>) -> Result<()> {
    match node {
        Node::Chord(c) | Node::Arpeggio(c) | Node::Brush(c) => out.push(chord_ref(c)?),
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

    fn measures(input: &str) -> Vec<MeasureRef> {
        use crate::import::{Importer, RechordImporter};
        let ast = RechordImporter.import(input).unwrap().resolve();
        measure_refs(&ast).unwrap()
    }

    #[test]
    fn measure_refs_groups_chords_by_measure() {
        let ms = measures("C | Am F |\n");
        assert_eq!(ms.len(), 2);
        assert_eq!(ms[0].chords.len(), 1);
        assert_eq!(ms[1].chords.len(), 2);
        assert_eq!(ms[1].chords[0].label, "Am");
        assert_eq!(ms[1].chords[1].label, "F");
    }

    #[test]
    fn measure_refs_records_line_breaks() {
        // 1行に2小節。改行は2小節目の後ろだけ
        let one_line = measures("C | F |\n");
        assert_eq!(
            one_line.iter().map(|m| m.line_break).collect::<Vec<_>>(),
            vec![false, true]
        );
        // 1行1小節。両方の後ろで改行される
        let two_lines = measures("C |\nF |\n");
        assert_eq!(
            two_lines.iter().map(|m| m.line_break).collect::<Vec<_>>(),
            vec![true, true]
        );
    }

    #[test]
    fn measure_refs_keeps_chordless_measures() {
        // 休符だけの小節も小節数を保つために残す
        let ms = measures("C | N.C. |\n");
        assert_eq!(ms.len(), 2);
        assert!(ms[1].chords.is_empty());
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

    #[test]
    fn find_handles_power_chords() {
        // pow(C) は C と G の2音になる
        let r = refs("pow(C)\n");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].pitch_classes, mask(&[0, 7]));
        assert!(find(&r[0]).is_some());
    }

    #[test]
    fn measure_refs_ignores_rhythm() {
        // リズムでカードが増えないこと
        let ms = measures("pat(\"o.o.oxox\", C)\n");
        assert_eq!(ms.len(), 1);
        assert_eq!(ms[0].chords.len(), 1);
        assert_eq!(ms[0].chords[0].label, "C");
    }
}
