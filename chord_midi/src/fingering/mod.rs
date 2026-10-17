//! ギターの押さえ方データと、コードから押さえ方を引く処理。
//!
//! データの出典は daichan easy guitar「超簡単コード一覧」
//! <https://daichan-easyguitar.com/easychord-list/>

mod data;
mod identify;

pub use data::FINGERINGS;
pub use identify::{identify, Candidate, Source};

use crate::model::modifier::Modifier;
use crate::model::{
    ast::{ChordNode, Expr, Func, Node, Pattern},
    key::Key,
    pitch::Pitch,
    scale::Scale,
};
use anyhow::Result;

/// 標準チューニングの開放弦のピッチクラス。6弦→1弦の順
pub const OPEN_STRINGS: [u8; 6] = [4, 9, 2, 7, 11, 4];

/// 標準チューニングの開放弦の MIDI ノート番号。6弦→1弦の順。
/// ピッチクラスだけでは弦をまたいだ音の高低が比べられないので併せて持つ
pub const OPEN_STRING_PITCHES: [u8; 6] = [40, 45, 50, 55, 59, 64];

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

/// 1つの押さえ方。配列はすべて 6弦→1弦 の順
#[derive(Debug, Clone, PartialEq)]
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
    /// 最低音が鳴る弦。0 = 6弦, 5 = 1弦
    pub lowest_string: u8,
    /// 押さえ方の種類
    pub kind: ShapeKind,
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

/// 図に描ける押さえ方。原典の形 (`Fingering`) と `tab()` が書いた形を
/// 同じ型で扱うための所有型。`tab()` の形は実行時の値で `'static` に
/// できないので、図の側は `&'static Fingering` を受けられない
#[derive(Debug, Clone, PartialEq)]
pub struct Shape {
    /// 6弦→1弦。None = ミュート、Some(0) = 開放
    pub frets: [Option<u8>; 6],
    /// 押さえる指。`tab()` 由来なら全部 None
    pub fingers: [Option<Finger>; 6],
    /// 原典での呼び名。`tab()` 由来なら None。
    /// 図はこれが Some でコード名と違うときだけ「≒」を併記する
    pub name: Option<&'static str>,
}

impl Shape {
    /// 開放弦を除いた一番低いフレット。図の窓の開始位置に使う
    pub fn min_fret(&self) -> Option<u8> {
        self.frets
            .iter()
            .flatten()
            .copied()
            .filter(|f| *f > 0)
            .min()
    }

    /// 開放弦を除いた一番高いフレット
    pub fn max_fret(&self) -> Option<u8> {
        self.frets
            .iter()
            .flatten()
            .copied()
            .filter(|f| *f > 0)
            .max()
    }
}

impl From<&'static Fingering> for Shape {
    fn from(f: &'static Fingering) -> Self {
        Shape {
            frets: f.frets,
            fingers: f.fingers,
            name: Some(f.name),
        }
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
    /// `@N` の指定。`Fingering.lowest_string` に合わせて 0 = 6弦 に正規化する
    pub root_string: Option<u8>,
    /// `pow()` / `oct()` で明示された押さえ方の種類
    pub wanted: Option<ShapeKind>,
    /// `tab()` が直接書いた押さえ方。書かれていれば `find` を通さず
    /// この形をそのまま図に出す
    pub tab: Option<[Option<u8>; 6]>,
}

/// AST に現れるコードを出現順に取り出す。
///
/// 呼ぶ前に [`Expr::resolve`] を通しておくこと。度数表記のままだと
/// ピッチクラスが決まらずエラーになる。
pub fn chord_refs(ast: &Expr) -> Result<Vec<ChordRef>> {
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
    /// この小節を囲んでいる `pat()` のリズム。無ければ None
    pub pattern: Option<Pattern>,
    /// この小節が由来する `$name`。定義の外なら None。
    /// 入れ子の定義では外側 (曲構成の単位) の名前が付く
    pub section: Option<String>,
}

/// AST に現れるコードを小節ごとにまとめて取り出す。
///
/// 呼ぶ前に [`Expr::resolve`] を通しておくこと。
pub fn measure_refs(ast: &Expr) -> Result<Vec<MeasureRef>> {
    let mut out = Vec::new();
    collect_ast(ast, None, None, &mut out)?;
    Ok(out)
}

/// `pattern` は外側の `pat()` から降ってきたリズム。
/// 内側の `pat()` が優先されるのは export/midi.rs の解釈と同じ。
///
/// `section` は外側の `$name` から降ってきた定義の名前。リズムとは逆に
/// **外側が優先される**。`$verse = $riff | $riff | F | G` のとき、カードに
/// 出したいのは曲構成の単位である `$verse` であって、その部品の `$riff` ではない
fn collect_ast(
    ast: &Expr,
    pattern: Option<&Pattern>,
    section: Option<&str>,
    out: &mut Vec<MeasureRef>,
) -> Result<()> {
    match ast {
        Expr::Seq(items) => {
            for item in items {
                collect_ast(item, pattern, section, out)?;
            }
        }
        Expr::Measure(nodes, line_break) => {
            let mut chords = Vec::new();
            for node in nodes {
                collect_node(node, &mut chords)?;
            }
            out.push(MeasureRef {
                chords,
                line_break: *line_break,
                pattern: pattern.cloned(),
                section: section.map(str::to_string),
            });
        }
        Expr::Apply(func, inner, _) => {
            let owned;
            let next = match func {
                Func::Pat(arg) => {
                    owned = arg.as_pattern()?.clone();
                    Some(&owned)
                }
                Func::Pow | Func::Oct => pattern,
            };
            collect_ast(inner, next, section, out)?;
        }
        // 展開の跡。まだ名前が付いていなければ付ける。既に付いているなら
        // そちらが外側なので触らない
        Expr::Section(name, inner) => {
            collect_ast(inner, pattern, Some(section.unwrap_or(name)), out)?
        }
        // resolve() 済みなら包みは消えている。念のため中身へ潜る
        Expr::Keyed(_, inner) | Expr::Scaled(_, inner) => {
            collect_ast(inner, pattern, section, out)?
        }
        Expr::Comment(_) => {}
        // `Func::Pat` の引数としてしか現れない。Apply の腕で処理済みなので
        // ここに直接来ることはない
        Expr::Pattern(_) => {}
        // 定義と参照は expand が消す。ここに来るのは expand を通していないとき
        Expr::Define(name, _, _) => {
            return Err(anyhow::anyhow!(
                "定義 ${} が展開されていない。expand を通すこと",
                name
            ))
        }
        Expr::Ref(name, _) => {
            return Err(anyhow::anyhow!(
                "参照 ${} が展開されていない。expand を通すこと",
                name
            ))
        }
        // リピートも expand が消す
        Expr::Repeat(_, n, _) => {
            return Err(anyhow::anyhow!(
                "リピート x{} が展開されていない。expand を通すこと",
                n
            ))
        }
    }
    Ok(())
}

fn collect_node(node: &Node, out: &mut Vec<ChordRef>) -> Result<()> {
    match node {
        Node::Chord(e) | Node::Arpeggio(e) | Node::Brush(e) => {
            // resolve() 済みなら key は Absolute なので tonic/scale は無視される
            out.push(chord_ref(&e.eval(Pitch::C, Scale::Major))?)
        }
        Node::Split(e) => {
            // export/midi.rs の Node::Split の扱いと同じ
            for c in Node::split_into(e, Pitch::C, Scale::Major) {
                out.push(chord_ref(&c)?);
            }
        }
        Node::Tab(frets, _) => out.push(tab_ref(*frets)),
        Node::Rest | Node::Sustain | Node::Repeat => {}
    }
    Ok(())
}

/// `ChordNode` から押さえ方検索用の情報を取り出す。
/// 呼ぶ前に [`Expr::resolve`] を通しておくこと
pub fn chord_ref(node: &ChordNode) -> Result<ChordRef> {
    // ラベルに `@N` は含めない。弦の指定は押さえ方への指示であって
    // コード名の一部ではなく、どの弦で取ったかは図を見れば分かる。
    // 含めると押さえ方の名前と食い違い、「≒」の併記まで誤って出る
    let label = {
        let mut named = node.clone();
        named.root_string = None;
        named.to_string()
    };
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
        // コード名から引く経路。押さえ方は `find` が選ぶ
        tab: None,
    })
}

fn pitch_class(key: &Key) -> Result<u8> {
    match key {
        Key::Absolute(p) => Ok(*p as u8),
        Key::Relative(d) => Err(anyhow::anyhow!(
            "relative key {} is not resolved; call Expr::resolve() first",
            d
        )),
    }
}

/// 図に出す押さえ方。`tab()` が書いた形があればそれを、無ければ `find` の
/// 結果を返す。見つからなければ None
pub fn shape_of(chord: &ChordRef) -> Option<Shape> {
    if let Some(frets) = chord.tab {
        return Some(Shape {
            frets,
            fingers: [None; 6],
            name: None,
        });
    }
    find(chord).map(Shape::from)
}

/// `tab()` の押さえ方から `ChordRef` を作る。
///
/// 見出しは逆引きの先頭候補。候補が無ければ記法そのものを見出しにする。
/// `pitch_classes` と `bass` も鳴る音から出しておく。`tab` が立っている
/// 限り `find` には流れないが、空の集合を配って事故るのを避ける
fn tab_ref(frets: [Option<u8>; 6]) -> ChordRef {
    let mut pitch_classes = 0u16;
    let mut lowest: Option<(u16, u8)> = None;
    for (s, fret) in frets.iter().enumerate() {
        let Some(fret) = fret else { continue };
        let midi = u16::from(OPEN_STRING_PITCHES[s]) + u16::from(*fret);
        let pc = (midi % 12) as u8;
        pitch_classes |= 1u16 << pc;
        if lowest.is_none_or(|(m, _)| midi < m) {
            lowest = Some((midi, pc));
        }
    }
    let label = match identify(frets).first() {
        Some(c) => c.name.clone(),
        None => Node::Tab(frets, false).to_string(),
    };
    ChordRef {
        label,
        root: lowest.map(|(_, pc)| pc).unwrap_or(0),
        pitch_classes,
        bass: lowest.map(|(_, pc)| pc),
        root_string: None,
        wanted: None,
        tab: Some(frets),
    }
}

/// ビット集合に立っているピッチクラスを、root からの半音数にして返す
pub(crate) fn relative_semitones(mask: u16, root: u8) -> impl Iterator<Item = u8> {
    (0..12u8)
        .filter(move |p| ((mask >> *p) & 1) == 1)
        .map(move |p| (p + 12 - root) % 12)
}

/// 構成音にあるのに押さえ方に無い音のコスト。ルートからの半音数で引く。
///
/// ルートと 3rd はコードの素性そのものなので、どの「余計な音」より重くする。
/// 落としてよいのは 5th とテンションだけ。押さえ方は MIDI の声部配置にも
/// 使われる (`export::midi::voicing`) ので、3rd が抜けると `Cm` が
/// パワーコードで鳴ってしまう
pub(crate) fn missing_cost(semitone: u8) -> u32 {
    match semitone {
        0 => 6,       // ルート
        3 | 4 => 5,   // 3rd
        10 | 11 => 2, // 7th
        _ => 1,       // 5th とテンション
    }
}

/// 押さえ方にあるのに構成音に無い音のコスト。ルートからの半音数で引く。
///
/// 5th とテンションの不足 (1点) より重くする。音が足りないのは「薄い同じ
/// コード」で済むが、余計な音は別のコードに聞こえてしまうため
/// (`G#/C` に 11th の F が乗ると `Fm` に化ける)。
/// 逆に、ルートと 3rd の不足 (6 / 5点) よりは軽くする。そちらを削る方が
/// コードの素性を失う
pub(crate) fn extra_cost(semitone: u8) -> u32 {
    match semitone {
        11 => 2,    // M7。足しても元のコードの性格は残る
        10 => 3,    // b7。ドミナントに化ける
        2 | 9 => 3, // 9th / 13th
        0 | 7 => 3, // ルート / 5th
        5 => 4,     // 11th。3rd とぶつかって sus に聞こえる
        3 | 4 => 6, // 3rd。長短が決まってしまうので、3rd の不足より重い
        _ => 7,     // b9 / b5 / b6 はコードの性格を壊す
    }
}

/// ベースが合わないときに足すコスト。
///
/// スラッシュコードは最低音こそが指定なので、合わない形は別物に聞こえる。
/// 以前は同点のときのタイブレークだったが、それだと `G#/C` に最低音 F の
/// `Fm` が選ばれてしまう
const BASS_MISMATCH_COST: u32 = 3;

/// コードに一番合う押さえ方を返す。
///
/// 構成音に無い音を1つまで許容する。原典は `B` を `B7`、`F#m` を `F#m7` で
/// 代用させる構成なので、部分集合だけに絞ると素の三和音が軒並み引けなくなる。
pub fn find(chord: &ChordRef) -> Option<&'static Fingering> {
    let expected_bass = chord.bass.unwrap_or(chord.root);
    FINGERINGS
        .iter()
        // Normal は常に候補。可動形 (Power / Octave) は、書かれた関数で
        // 種類が明示されていればその種類だけ、明示が無ければ @N の指定が
        // あるときだけ候補にする
        // pow() / oct() で種類が明示されたら、その種類の押さえ方だけを候補にする。
        // 明示が無ければ原典の押さえ方だけ。ただし @N があれば可動形も開放する
        .filter(|f| match chord.wanted {
            Some(wanted) => f.kind == wanted,
            None => f.kind == ShapeKind::Normal || chord.root_string.is_some(),
        })
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
            let bass = if f.bass == expected_bass {
                0
            } else {
                BASS_MISMATCH_COST
            };
            (
                extra + missing + bass,
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
        let ast = RechordImporter.import(input).unwrap().resolve().unwrap();
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
        let ast = RechordImporter.import(input).unwrap().resolve().unwrap();
        measure_refs(&ast).unwrap()
    }

    #[test]
    fn measure_refs_carries_pattern_of_enclosing_pat() {
        let ms = measures("pat(\"o.oo..oo\", C | Am F)\n");
        assert_eq!(ms.len(), 2);
        for m in &ms {
            assert_eq!(m.pattern.as_ref().unwrap().to_string(), "o.oo..oo");
        }
    }

    #[test]
    fn measure_refs_has_no_pattern_without_pat() {
        let ms = measures("C | Am F |\n");
        assert!(ms.iter().all(|m| m.pattern.is_none()));
    }

    #[test]
    fn measure_refs_inner_pattern_wins() {
        let ms = measures("pat(\"oxox\", pat(\"o.oo..oo\", C))\n");
        assert_eq!(ms.len(), 1);
        assert_eq!(ms[0].pattern.as_ref().unwrap().to_string(), "o.oo..oo");
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
    fn measure_refs_carry_section_name() {
        let ms = measures("$A = C | Am |\n$A | F |\n");
        assert_eq!(ms.len(), 3);
        assert_eq!(ms[0].section.as_deref(), Some("A"));
        assert_eq!(ms[1].section.as_deref(), Some("A"));
        // $A の外の小節には名前が付かない
        assert_eq!(ms[2].section, None);
    }

    #[test]
    fn measure_refs_name_both_repeated_sets() {
        // xN は $A の中身を並べ直すだけ。どちらのセットも $A 由来
        let ms = measures("$A = C | Am |\n$A x2\n");
        assert_eq!(ms.len(), 4);
        assert!(
            ms.iter().all(|m| m.section.as_deref() == Some("A")),
            "{:?}",
            ms.iter().map(|m| &m.section).collect::<Vec<_>>()
        );
    }

    #[test]
    fn measure_refs_use_the_outer_section_name() {
        // $b の中で $a を使っている。カードに出したいのは曲構成の単位である $b
        let ms = measures("$a = C\n$b = $a | $a |\n$b | F |\n");
        assert_eq!(ms.len(), 3);
        assert_eq!(ms[0].section.as_deref(), Some("b"));
        assert_eq!(ms[1].section.as_deref(), Some("b"));
        assert_eq!(ms[2].section, None);
    }

    #[test]
    fn measure_refs_keep_both_pattern_and_section() {
        // pat() のリズムは内側が、定義の名前は外側が勝つ。互いに潰し合わない
        let ms = measures("$A = C | Am |\npat(\"o.o.\", $A)\n");
        assert_eq!(ms.len(), 2);
        for m in &ms {
            assert_eq!(m.section.as_deref(), Some("A"));
            assert_eq!(m.pattern.as_ref().unwrap().to_string(), "o.o.");
        }
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
        assert_eq!(FINGERINGS.len(), 115);
        for f in FINGERINGS {
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

    #[test]
    fn find_prefers_the_fuller_chord() {
        // CM7 には C の形ではなく C△7 の形が選ばれる
        assert_eq!(tab_of("CM7 |\n"), "x-3-x-0-0-0");
    }

    #[test]
    fn find_falls_back_to_the_substitute_shape() {
        // 原典は B を B7、F#m を F#m7 で代用させる構成になっている
        assert_eq!(tab_of("B |\n"), "x-2-1-2-0-x");
        assert_eq!(tab_of("Am |\n"), "x-0-2-0-1-x");
        assert_eq!(tab_of("Em |\n"), "0-2-0-0-0-0");
        assert_eq!(tab_of("F#m |\n"), "2-x-2-2-x-x");
        assert_eq!(tab_of("C#m |\n"), "x-4-x-4-0-0");
    }

    #[test]
    fn find_rejects_two_extra_notes() {
        // Cm7 (x-3-x-3-4-x) は C に対して Eb と Bb の2音が余分なので候補に入らない
        assert_ne!(tab_of("C |\n"), "x-3-x-3-4-x");
    }

    #[test]
    fn find_honors_slash_bass() {
        assert_eq!(tab_of("D/F# |\n"), "2-x-0-2-3-x");
    }

    #[test]
    fn find_returns_none_when_nothing_fits() {
        // C# 1音だけの集合に対しては、どの押さえ方も余分な音が2つ以上になる
        let c = ChordRef {
            label: "test".to_string(),
            root: 1,
            pitch_classes: 1u16 << 1,
            bass: None,
            root_string: None,
            wanted: None,
            tab: None,
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

    #[test]
    fn power_chord_shapes_are_present() {
        let powers: Vec<_> = FINGERINGS
            .iter()
            .filter(|f| f.kind == ShapeKind::Power)
            .collect();
        // 6弦ルート12キー + 5弦ルート12キー
        assert_eq!(powers.len(), 24);
        assert_eq!(FINGERINGS.len(), 67 + 24 + 24);
    }

    #[test]
    fn power_chord_shapes_sound_root_and_fifth() {
        for f in FINGERINGS.iter().filter(|f| f.kind == ShapeKind::Power) {
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
        for f in FINGERINGS.iter().filter(|f| f.kind == ShapeKind::Power) {
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
        assert_eq!(d5.kind, ShapeKind::Power);
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

    #[test]
    fn chord_ref_label_omits_the_string_hint() {
        // @N はコード名の一部ではないのでラベルに出さない
        assert_eq!(refs("A@6 |\n")[0].label, "A");
        assert_eq!(refs("pow(A@6)\n")[0].label, "A5");
        // 指定が無いときと同じラベルになる
        assert_eq!(refs("A |\n")[0].label, refs("A@6 |\n")[0].label);
    }

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

    /// 余計な音は不足より重い。音が足りない形は「薄い同じコード」だが、
    /// 余計な音が乗った形は別のコードに聞こえてしまう
    #[test]
    fn extra_notes_cost_more_than_missing_ones() {
        // Cm の 3rd を持つ形は原典に無い。かつては 13th が乗る Am7-5 が
        // 選ばれていた。今は 5th を落として m3 を残す Cm7 の形になる
        assert_eq!(tab_of("Cm |\n"), "x-3-x-3-4-x");
        // G# のトライアドも原典に無い。7th を足すなら、ドミナントに化ける
        // b7 (G#7) より性格の残る M7 を選ぶ
        assert_eq!(tab_of("G# |\n"), "4-x-5-5-x-x");
    }

    /// ルートと 3rd の不足は、どの余計な音より重い。押さえ方は MIDI の
    /// 声部配置にも使われるので、ここが抜けるとコードの素性が消える
    #[test]
    fn root_and_third_are_never_dropped_when_a_shape_has_them() {
        // 3rd を持たない形 (C5 相当) に流れないこと
        assert_eq!(tab_of("Cm |\n"), "x-3-x-3-4-x");
        // ルートの G# を持たない形 (Fm など) に流れないこと
        assert!(tab_of("G#m |\n").contains("0-2-2-1-0-0"));
    }

    /// 最低音が合わない形は減点する。構成音がちょうど一致していても、
    /// 最低音が違えば別のコードに聞こえる (`C6` を `Am7` の形で弾くなど)
    #[test]
    fn bass_mismatch_is_penalised() {
        // Am7 (= C6/A) は C6 と構成音が完全に一致するが最低音が A。
        // 6th を落としてでも最低音が C の形を選ぶ
        assert_eq!(tab_of("C6 |\n"), "x-3-2-0-1-x");
        // 同じく G6 と Em7
        assert_eq!(tab_of("G6 |\n"), "3-x-0-0-0-x");
    }

    /// スラッシュコードの最低音がデータに無いときの落としどころ。
    /// ルートと 3rd を持つ形に落ち、余計な音が乗る形には行かない
    #[test]
    fn slash_chord_without_the_written_bass_keeps_root_and_third() {
        // 最低音が C の A♭ トライアドは原典に無い。かつては 11th の F が
        // 乗って最低音も F の Fm (x-x-3-1-1-x) が選ばれていた
        assert_eq!(tab_of("G#/C |\n"), "4-x-5-5-x-x");
    }

    #[test]
    fn power_shapes_do_not_displace_normal_chords() {
        // 2音しか無いパワーコード形が3rdや7thを持つ形を押しのけないこと
        assert_eq!(tab_of("C |\n"), "x-3-2-0-1-x");
        assert_eq!(tab_of("Am7 |\n"), "x-0-2-0-1-x");
        // どちらも原典に形が無く、3rd を残す形が選ばれる。
        // D#m7 -> D#dim (5th が b5 に化けるが root + m3 は残る)、
        // G#m -> E (= E/G#。root + m3 を残し 5th を落とす)
        assert_eq!(tab_of("D#m7 |\n"), "x-x-1-2-x-2");
        assert_eq!(tab_of("C#sus4 |\n"), "x-1-x-1-2-x");
        assert_eq!(tab_of("G#m |\n"), "0-2-2-1-0-0");
    }

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
        // どちらも原典に形が無く、3rd を残す形が選ばれる。
        // D#m7 -> D#dim (5th が b5 に化けるが root + m3 は残る)、
        // G#m -> E (= E/G#。root + m3 を残し 5th を落とす)
        assert_eq!(tab_of("D#m7 |\n"), "x-x-1-2-x-2");
        assert_eq!(tab_of("C#sus4 |\n"), "x-1-x-1-2-x");
        assert_eq!(tab_of("G#m |\n"), "0-2-2-1-0-0");
    }

    #[test]
    fn tab_node_becomes_a_chord_ref_with_its_own_shape() {
        let r = refs("tab(8 6 6 x x x) |\n");
        assert_eq!(r.len(), 1);
        assert_eq!(
            r[0].tab,
            Some([Some(8), Some(6), Some(6), None, None, None])
        );
        let shape = shape_of(&r[0]).expect("tab は必ず形を持つ");
        assert_eq!(shape.frets, [Some(8), Some(6), Some(6), None, None, None]);
        // 原典の形ではないので「≒」用の名前は無い
        assert_eq!(shape.name, None);
        // 指番号のデータは無い
        assert_eq!(shape.fingers, [None; 6]);
    }

    #[test]
    fn shape_of_falls_back_to_the_data_shape() {
        let r = refs("C |\n");
        let shape = shape_of(&r[0]).expect("C は原典にある");
        assert_eq!(
            shape.frets,
            [None, Some(3), Some(2), Some(0), Some(1), None]
        );
        assert_eq!(shape.name, Some("C"));
    }

    #[test]
    fn tab_label_comes_from_identify() {
        // 6弦8/5弦6/4弦6 = C, D#, G# = G# のトライアドで最低音が C
        let r = refs("tab(8 6 6 x x x) |\n");
        assert_eq!(r[0].label, "G#/C");
    }

    #[test]
    fn tab_label_falls_back_to_the_notation() {
        // 全ミュートは鳴る音が無いので候補が出ない。記法をそのまま見出しにする。
        // 1音だけなら候補は出る (`tab(x x x x x 0)` は E5)
        let r = refs("tab(x x x x x x) |\n");
        assert_eq!(r[0].label, "tab(x x x x x x)");
        let one = refs("tab(x x x x x 0) |\n");
        assert_eq!(one[0].label, "E5");
    }
}
