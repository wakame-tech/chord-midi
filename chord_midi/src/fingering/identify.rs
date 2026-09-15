//! 押さえ方からコード名を引く。[`super::find`] の逆。

use super::{
    chord_ref, extra_cost, find, missing_cost, relative_semitones, ChordRef, Fingering, ShapeKind,
    FINGERINGS, OPEN_STRING_PITCHES,
};
use crate::model::{ast::ChordNode, key::Key, modifier::Modifier, pitch::Pitch};
use std::collections::BTreeSet;

/// 候補がどこから来たか
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    /// 押さえ方データにこの形があり、記法もこの形を再現する。
    /// 中身の `name` は原典での呼び名（"A♭△7" など。rechord の記法ではない）
    Data(&'static Fingering),
    /// 鳴っている音から計算しただけ
    Theory,
}

/// 逆引きの結果1件
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    /// rechord のコード名。"C" や "C/E"
    pub name: String,
    /// そのまま入力に書ける記法。"C/E@6" や "pow(A@6)"
    pub notation: String,
    pub source: Source,
    /// `notation` を書くと、押さえた形がそのまま戻ってくる。false なら音は
    /// 合っているが別の押さえ方になる（今のデータでこの形は名指しできない）
    pub reproduces_shape: bool,
    /// 小さいほど良い。表示順に使う
    pub score: u32,
}

/// 鳴っている音
struct Played {
    /// ピッチクラス集合
    pitch_classes: u16,
    /// 最低音のピッチクラス
    bass: u8,
    /// 最低音が鳴っている弦。0 = 6弦
    bass_string: u8,
}

/// 押さえ方から鳴っている音を出す。1本も鳴っていなければ None。
///
/// 最低音は弦の番号ではなく実 MIDI ピッチの最小値から取る。`Fingering.bass` は
/// 「鳴る弦のうち一番低い弦」で定義されているが、これは原典データがロー
/// ポジションばかりだから成り立つ近似で、自由に押さえられる指板では破れる
/// （6弦12fr = E3(52) より 5弦開放 = A2(45) の方が低い）。
fn played(frets: &[Option<u8>; 6]) -> Option<Played> {
    let mut pitch_classes = 0u16;
    let mut lowest: Option<(u16, u8, u8)> = None; // (midi, ピッチクラス, 弦)
    for (s, fret) in frets.iter().enumerate() {
        let Some(fret) = fret else { continue };
        // u16 で足す。identify は pub なので 200 フレットのような値でも
        // 呼べてしまい、u8 のままだと桁あふれでパニックする。大小さえ比べ
        // られればよいので幅を広げるだけで済む
        let midi = u16::from(OPEN_STRING_PITCHES[s]) + u16::from(*fret);
        // OPEN_STRINGS[s] == OPEN_STRING_PITCHES[s] % 12 なので midi から取れる
        let pc = (midi % 12) as u8;
        pitch_classes |= 1u16 << pc;
        if lowest.is_none_or(|(m, _, _)| midi < m) {
            lowest = Some((midi, pc, s as u8));
        }
    }
    lowest.map(|(_, bass, bass_string)| Played {
        pitch_classes,
        bass,
        bass_string,
    })
}

/// 返す候補の最大件数
const MAX_CANDIDATES: usize = 5;

/// ルートが1音も鳴っていない呼び方に足す点。[`score`] を見よ。
/// 測って決めた値ではなく、`missing_cost` のルート4点・3rd3点と釣り合う
/// あたりに置いた経験則
const ROOT_NOT_SOUNDING: u32 = 3;

/// 最低音がルートでないのにスラッシュで書いていない呼び方に足す点。
/// [`score`] を見よ。これも測って決めた値ではなく経験則
const BASS_NOT_WRITTEN_AS_SLASH: u32 = 2;

/// 自分のルート以外がベースに来るパワーコードに足す点。[`score`] を見よ。
///
/// この値だけは115形すべてで測って決めた。1〜4 のどれでも先頭の候補は同じに
/// なる。2 にしてあるのは、1 だと実際の名前とパワーコードが同点になり、勝敗が
/// QUALITIES の並び順のタイブレーク（`Power` が末尾）頼みになるため。
/// 2 以上なら点そのもので決まる
const POWER_ON_FOREIGN_BASS: u32 = 2;

/// 逆引きで当てにいくコードの品質。`Modifier::Major(5)` は `chord_node_parser`
/// と同じく常に種として入れるので、ここには書かない。上から「よくある」順で、
/// 同点のときはこの順が優先される。
///
/// 入れていない品質:
/// - m6: `Modifier::Minor(6)` はマイナースケールの6度 = b6 を足す。実際の m6
///   コードは長6度なので今の `Modifier` では正しく組めない
/// - oct: 構成音がルート1音だけなので、どの1音を押さえても完全一致して候補を
///   埋め尽くす。オクターブ形はデータ照合で拾う
const QUALITIES: &[&[Modifier]] = &[
    &[],                                      // C
    &[Modifier::Minor(5)],                    // Cm
    &[Modifier::Dominant(7)],                 // C7
    &[Modifier::Minor(7)],                    // Cm7
    &[Modifier::Major(7)],                    // Cmaj7
    &[Modifier::Sus4],                        // Csus4
    &[Modifier::Sus2],                        // Csus2
    &[Modifier::Dominant(6)],                 // C6
    &[Modifier::Add(9)],                      // Cadd9
    &[Modifier::Dominant(9)],                 // C9
    &[Modifier::Minor(9)],                    // Cm9
    &[Modifier::Major(9)],                    // Cmaj9
    &[Modifier::MinorMajaor7],                // CmM7
    &[Modifier::Minor(7), Modifier::Flat5th], // Cm7b5
    &[Modifier::Dim],                         // Cdim
    &[Modifier::Dim7],                        // Cdim7
    &[Modifier::Aug],                         // Caug
    &[Modifier::Power],                       // C5 (記法は render() が pow(C) に包む)
];

/// 品質から `ChordNode` を作る。`chord_node_parser` と同じく `Major(5)` を
/// 種にしてから修飾を足す。種が無いと `Sus4` のような「既存の3rdを消して4thを
/// 入れる」修飾が土台の無いところに当たって壊れる
fn node(root: Pitch, quality: &[Modifier], on: Option<Pitch>) -> ChordNode {
    let mut modifiers = BTreeSet::from([Modifier::Major(5)]);
    modifiers.extend(quality.iter().cloned());
    ChordNode {
        key: Key::Absolute(root),
        modifiers,
        on: on.map(Key::Absolute),
        root_string: None,
    }
}

/// 押さえた音とコードのずれを点にする。小さいほど良い。
/// 押さえた音のうちコードで説明できない音が2つ以上なら候補にしない
fn score(played: &Played, chord: &ChordRef) -> Option<u32> {
    let extra_mask = played.pitch_classes & !chord.pitch_classes;
    // `find()` の `(f.pitch_classes & !chord.pitch_classes).count_ones() <= 1`
    // と同じ足切りを裏返したもの。この関数全体が「`find()` の採点を逆向きに
    // 走らせたもの」だと思って読める
    if extra_mask.count_ones() >= 2 {
        return None;
    }
    let extra: u32 = relative_semitones(extra_mask, chord.root)
        .map(extra_cost)
        .sum();
    let missing: u32 = relative_semitones(chord.pitch_classes & !played.pitch_classes, chord.root)
        .map(missing_cost)
        .sum();
    // ルートが鳴っていない呼び方は疑わしい
    let root_silent = if (played.pitch_classes >> chord.root) & 1 == 0 {
        ROOT_NOT_SOUNDING
    } else {
        0
    };
    // 最低音がルートでないのにスラッシュで書いていない
    let bass_mismatch = if chord.bass.is_none() && played.bass != chord.root {
        BASS_NOT_WRITTEN_AS_SLASH
    } else {
        0
    };
    // ルート以外がベースに来るパワーコード。パワーコードは2音しか無いので
    // たいていの押さえ方に 0 点で当たってしまい、まともな名前を押しのける。
    // {C, D#, A#} を「D# パワーコード on C」と読む人はいない（5thを省いた
    // Cm7 と読む）ので、自分のルートに乗っていないパワーコードだけ重くする。
    // `chord_ref` は Power の修飾から `wanted` を立てるので、それで見分ける
    let power_on_foreign_bass =
        if chord.wanted == Some(ShapeKind::Power) && chord.bass.is_some_and(|b| b != chord.root) {
            POWER_ON_FOREIGN_BASS
        } else {
            0
        };
    Some(extra + missing + root_silent + bass_mismatch + power_on_foreign_bass)
}

/// ノードを記法の文字列にする。
///
/// `Modifier::Power` を持つノードをそのまま書くと `D5` になるが、読み直すと
/// `Modifier::Dominant(5)` = ただのメジャートライアドになってしまう
/// （`Modifier::Power` は `pow()` からしか作れない）。`Power` を外した中身を
/// `pow()` で包んで、書いた通りに読める形にする。`@N` は `ChordNode` の
/// `Display` が末尾に付けるので、自然と括弧の内側に入る。
///
/// `Modifier::Octave` も `Aoct` と出て同じく読み戻せないが、`QUALITIES` に
/// 入れていないのでここには来ない。オクターブ形は `movable_candidate` が
/// `oct(...)` で書く
fn render(node: &ChordNode) -> String {
    if !node.modifiers.contains(&Modifier::Power) {
        return node.to_string();
    }
    let mut inner = node.clone();
    inner.modifiers.remove(&Modifier::Power);
    format!("pow({})", inner)
}

/// 記法を決める。`@N` 無しで元の形に戻るならそのまま、戻らなければ `@N` を
/// 足してもう一度試す。それでも戻らなければ `@N` 無しの名前を返し、
/// 「再現できない」と伝える。返り値は (記法, 元の形を再現するか)
fn notation_for(
    node: &ChordNode,
    r: &ChordRef,
    played: &Played,
    frets: &[Option<u8>; 6],
) -> (String, bool) {
    if find(r).map(|f| f.frets) == Some(*frets) {
        return (render(node), true);
    }
    let mut with_string = node.clone();
    // 記法は 1 = 1弦、Played.bass_string は 0 = 6弦
    with_string.root_string = Some(6 - played.bass_string);
    if let Ok(r2) = chord_ref(&with_string) {
        if find(&r2).map(|f| f.frets) == Some(*frets) {
            return (render(&with_string), true);
        }
    }
    (render(node), false)
}

/// 可動形（パワーコード / オクターブ形）の候補。
///
/// `C5` と書くと `Modifier::Dominant(5)` に読まれてただのメジャートライアドに
/// なってしまう（`Modifier::Power` は `pow()` からしか作れない）ので、記法は
/// 関数で包む。どの弦で取るかが形そのものなので `@N` は常に付ける
fn movable_candidate(f: &'static Fingering) -> Option<Candidate> {
    let (func, suffix) = match f.kind {
        ShapeKind::Power => ("pow", "5"),
        ShapeKind::Octave => ("oct", "oct"),
        ShapeKind::Normal => return None,
    };
    let root = Pitch::try_from(f.bass).expect("ピッチクラスは 0..12");
    // Fingering.lowest_string は 0 = 6弦、記法の @N は 1 = 1弦
    let string = 6 - f.lowest_string;
    Some(Candidate {
        name: format!("{}{}", root, suffix),
        notation: format!("{}({}@{})", func, root, string),
        source: Source::Data(f),
        reproduces_shape: true,
        score: 0,
    })
}

/// 押さえ方からコード名候補を引く。frets は 6弦→1弦、None = ミュート。
///
/// 返る候補は確度の高い順（[`Candidate::score`] の小さい順）で、最大
/// [`MAX_CANDIDATES`] 件。同じ [`Candidate::name`] は1件に絞る。1本も鳴って
/// いなければ空。押さえ方データに同じ形があればその候補に [`Source::Data`] が
/// 付くが、必ずしも先頭とは限らない（並ぶのはあくまで採点順）
pub fn identify(frets: [Option<u8>; 6]) -> Vec<Candidate> {
    let Some(played) = played(&frets) else {
        return Vec::new();
    };

    // (score, QUALITIES での並び順, ノード, 検索用の情報)
    let mut scored: Vec<(u32, usize, ChordNode, ChordRef)> = Vec::new();
    for root in 0..12u8 {
        let root_pitch = Pitch::try_from(root).expect("ピッチクラスは 0..12");
        // `on` 無しと、最低音がルートと違うときの `on` 付きを両方試す。
        // `C/E` と `C` はどちらも正しい呼び方なので、どちらが上に来るかは
        // 採点に任せる。品質には依らないので root ごとに1回だけ決める
        let on_bass = (played.bass != root)
            .then(|| Pitch::try_from(played.bass).expect("ピッチクラスは 0..12"));
        for (qi, quality) in QUALITIES.iter().enumerate() {
            for on in std::iter::once(None).chain(on_bass.map(Some)) {
                let node = node(root_pitch, quality, on);
                let Ok(r) = chord_ref(&node) else { continue };
                let Some(s) = score(&played, &r) else {
                    continue;
                };
                scored.push((s, qi, node, r));
            }
        }
    }
    // `(score, QUALITIES の並び順)` が同点のときは、`on` 無し（`C`）を `on` 付き
    // （`C/E`）より先に出す。これは先に push した方が残る安定ソートに頼っている
    // ので、`sort_unstable_by_key` に替えてはいけない
    scored.sort_by_key(|(s, qi, _, _)| (*s, *qi));

    // 同じ形が押さえ方データにあるか。名前はここから取らない（原典表記で
    // rechord のパーサに通らないため）。使うのは「載っているか」と kind だけ
    let data = FINGERINGS.iter().find(|f| f.frets == frets);

    let mut out: Vec<Candidate> = Vec::new();
    // 可動形は段2が出せない（Power は記法が違い、Octave は1音なので何にでも
    // 一致する）ので、データに当たったときだけここで先頭に足す
    if let Some(c) = data.and_then(movable_candidate) {
        out.push(c);
    }
    // 並び順は (score, QUALITIES の並び順) で既に確定しているので、上から
    // 必要な件数だけ記法を決めれば足りる
    for (s, _, node, r) in scored {
        if out.len() >= MAX_CANDIDATES {
            break;
        }
        if out.iter().any(|c| c.name == r.label) {
            continue;
        }
        let (notation, reproduces) = notation_for(&node, &r, &played, &frets);
        // `ShapeKind::Normal` の条件は消さないこと。可動形に当たった形は
        // `movable_candidate` が既に原典として1件出している。可動形は2音しか
        // 無いので理論候補もよく同じ形を引き当ててしまい（`57xxxx` の
        // `Esus4@6` など）、ここを緩めると Esus4 のカードに「原典での呼び名:
        // A5」が付く。原典名を貸してよいのは原典そのものの押さえ方だけ
        let source = match data {
            Some(f) if reproduces && f.kind == ShapeKind::Normal => Source::Data(f),
            _ => Source::Theory,
        };
        out.push(Candidate {
            name: r.label.clone(),
            notation,
            source,
            reproduces_shape: reproduces,
            score: s,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fingering::chord_refs;
    use std::collections::HashMap;

    /// "x3201x" のような文字列を frets に直す。x = ミュート、それ以外は1桁の16進
    fn frets(s: &str) -> [Option<u8>; 6] {
        assert_eq!(s.chars().count(), 6, "6弦ぶん書くこと: {}", s);
        let mut out = [None; 6];
        for (i, c) in s.chars().enumerate() {
            out[i] = match c {
                'x' => None,
                c => Some(c.to_digit(16).expect("フレットは x か16進1桁") as u8),
            };
        }
        out
    }

    #[test]
    fn identify_returns_nothing_for_all_muted() {
        assert!(identify(frets("xxxxxx")).is_empty());
    }

    #[test]
    fn identify_does_not_overflow_on_a_large_fret() {
        // identify は pub なので、型が許す値はどれも渡されうる。
        // MIDI 番号を u8 で足していた頃はここで桁あふれしてパニックした
        assert!(!identify([None, None, None, None, None, Some(200)]).is_empty());
    }

    #[test]
    fn identify_names_a_high_position_shape() {
        // 8aa988 = 8フレットの C（E フォームのバレー）。原典は簡易コードしか
        // 載せていないのでデータに無く、理論だけで名前が出る。
        // ハイポジションを見るのはこのテストだけ
        let c = identify(frets("8aa988"));
        assert_eq!(
            c[0].name,
            "C",
            "候補の並び: {:?}",
            c.iter().map(|x| &x.name).collect::<Vec<_>>()
        );
        assert_eq!(c[0].source, Source::Theory);
    }

    #[test]
    fn identify_names_an_open_e() {
        let c = identify(frets("022100"));
        assert_eq!(c[0].name, "E");
    }

    #[test]
    fn identify_names_an_open_a_minor() {
        // x02210 = Am
        let c = identify(frets("x02210"));
        assert_eq!(c[0].name, "Am");
    }

    #[test]
    fn identify_names_a_seventh() {
        // 020100 = E7
        let c = identify(frets("020100"));
        assert_eq!(c[0].name, "E7");
    }

    #[test]
    fn identify_caps_the_number_of_candidates() {
        // 可動形は movable_candidate が先に1枠使うので、打ち切りの効き方が
        // 通常の形と違う。両方見る
        for shape in ["x3201x", "57xxxx"] {
            let c = identify(frets(shape));
            assert!(
                c.len() <= MAX_CANDIDATES,
                "{} で {} 件返った",
                shape,
                c.len()
            );
        }
    }

    #[test]
    fn identify_offers_a_slash_chord_when_the_bass_is_not_the_root() {
        // x32010 は 5弦3fr(C) が最低音なので C のまま。1弦を開放にした
        // "332010" は 6弦3fr = G が最低音になる C/G
        let c = identify(frets("332010"));
        assert_eq!(
            c[0].name,
            "C/G",
            "候補の並び: {:?}",
            c.iter()
                .map(|x| format!("{}({})", x.name, x.score))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn identify_does_not_use_a_slash_name_when_the_bass_is_the_root() {
        // xx0232 は D。4弦開放(D) が最低音なのでスラッシュにはならない
        let d = identify(frets("xx0232"));
        assert_eq!(d[0].name, "D");
    }

    #[test]
    fn identify_has_no_duplicate_names() {
        let c = identify(frets("x3201x"));
        let mut names: Vec<_> = c.iter().map(|x| x.name.clone()).collect();
        names.sort();
        let before = names.len();
        names.dedup();
        assert_eq!(names.len(), before);
    }

    #[test]
    fn identify_marks_a_shape_that_exists_in_the_data() {
        let c = identify(frets("x3201x"));
        assert_eq!(c[0].name, "C");
        assert_eq!(c[0].notation, "C");
        assert!(matches!(c[0].source, Source::Data(_)));
        assert!(c[0].reproduces_shape);
    }

    #[test]
    fn identify_marks_a_shape_that_is_not_in_the_data() {
        // x3555x は C だが、この形は押さえ方データに無い
        let c = identify(frets("x3555x"));
        assert_eq!(c[0].name, "C");
        assert_eq!(c[0].source, Source::Theory);
        assert!(!c[0].reproduces_shape);
    }

    /// ピッチクラス集合を "C, E, G" のような音名の並びにする。
    /// u16 のビット集合を10進で突きつけられても何も分からないので
    fn notes(mask: u16) -> String {
        (0..12u8)
            .filter(|p| (mask >> *p) & 1 == 1)
            .map(|p| {
                Pitch::try_from(p)
                    .expect("ピッチクラスは 0..12")
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// 押さえ方を "x3201x" の形に戻す。失敗したとき指板で再現できるように
    fn tab(frets: [Option<u8>; 6]) -> String {
        frets
            .iter()
            .map(|v| v.map_or("x".to_string(), |n| format!("{:x}", n)))
            .collect()
    }

    /// 記法を読み直して押さえ方検索用の情報にする。`node_parser` は private
    /// モジュールにあり Span の組み立ても面倒なので、公開 API の
    /// RechordImporter を通す。これなら `pow(A@6)` のような関数も読める
    fn reparse(notation: &str) -> Vec<ChordRef> {
        use crate::import::{Importer, RechordImporter};
        // `pow(...)` / `oct(...)` は小節をまるごと包む書き方なので、後ろに `|` は
        // 付けられない。素のコード名だけ小節にする
        let input = if notation.starts_with("pow(") || notation.starts_with("oct(") {
            format!("{}\n", notation)
        } else {
            format!("{} |\n", notation)
        };
        let ast = RechordImporter
            .import(&input)
            .unwrap_or_else(|e| panic!("{} がパースできない: {}", notation, e))
            .resolve();
        chord_refs(&ast).unwrap()
    }

    #[test]
    fn identify_names_a_power_shape() {
        // 57xxxx = 6弦5fr + 5弦7fr の A パワーコード
        let c = identify(frets("57xxxx"));
        assert_eq!(c[0].name, "A5");
        assert_eq!(c[0].notation, "pow(A@6)");
        assert!(matches!(c[0].source, Source::Data(_)));
        assert!(c[0].reproduces_shape);
    }

    /// 可動形に当たったとき、原典のバッジが付くのは `movable_candidate` が出した
    /// 1件だけ。同じ形を `find()` で引き当てただけの理論候補に原典名を貸さない
    /// （`score()` の `ShapeKind::Normal` ガードが効いていること）
    #[test]
    fn identify_gives_the_data_badge_only_to_the_movable_candidate() {
        let c = identify(frets("57xxxx"));
        let data: Vec<_> = c
            .iter()
            .filter(|x| matches!(x.source, Source::Data(_)))
            .collect();
        assert_eq!(
            data.len(),
            1,
            "原典のバッジが付くのは1件だけのはず: {:?}",
            c.iter()
                .map(|x| (x.name.clone(), matches!(x.source, Source::Data(_))))
                .collect::<Vec<_>>()
        );
        assert_eq!(data[0].notation, "pow(A@6)");
    }

    #[test]
    fn identify_names_an_octave_shape() {
        // 5x7xxx = 6弦5fr + 4弦7fr の A オクターブ形
        let c = identify(frets("5x7xxx"));
        assert_eq!(c[0].name, "Aoct");
        assert_eq!(c[0].notation, "oct(A@6)");
        assert!(matches!(c[0].source, Source::Data(_)));
    }

    #[test]
    fn identify_names_a_five_string_power_shape() {
        // x02xxx = 5弦開放 + 4弦2fr の A パワーコード
        let c = identify(frets("x02xxx"));
        assert_eq!(c[0].notation, "pow(A@5)");
    }

    #[test]
    fn identify_prefers_a_real_name_over_a_power_chord_on_a_foreign_bass() {
        // x3x34x が鳴らすのは {C, D#, A#}。D#5/C（D# パワーコード on C）は
        // ちょうどこの3音に当たるので素点は 0 になるが、こう読む人はいない。
        // 5th を省いた Cm7 が先頭に来ること
        let c = identify(frets("x3x34x"));
        assert_eq!(
            c[0].name,
            "Cm7",
            "候補の並び: {:?}",
            c.iter()
                .map(|x| format!("{}({})", x.name, x.score))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn identify_keeps_a_power_chord_on_its_own_root() {
        // 自分のルートに乗っているパワーコードは素直な読み方なので罰しない。
        // どちらも原典が簡易コードとして載せている2音の形
        for (shape, want) in [("x3x01x", "C5"), ("3x003x", "G5")] {
            let c = identify(frets(shape));
            assert_eq!(
                c[0].name,
                want,
                "{} の候補の並び: {:?}",
                shape,
                c.iter()
                    .map(|x| format!("{}({})", x.name, x.score))
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn identify_wraps_a_power_candidate_that_is_not_in_the_data() {
        // 3x003x は原典の G。kind は Normal なので可動形としては拾われないが、
        // 鳴っている音からは G5 とも読める。この G5 を裸で書くと読み直したとき
        // Dominant(5) = G メジャートライアドになってしまうので pow() で包む
        let c = identify(frets("3x003x"));
        let power = c.iter().find(|x| x.name == "G5").unwrap_or_else(|| {
            panic!(
                "G5 が候補に無い: {:?}",
                c.iter().map(|x| &x.name).collect::<Vec<_>>()
            )
        });
        assert_eq!(power.notation, "pow(G)");
    }

    /// 候補の名前から、その名前が意味する構成音を引く。identify が使うのと
    /// 同じ品質テーブルから組み立てるので、突き合わせる相手は「パーサが記法を
    /// どう読んだか」という独立した側になる
    fn pitch_classes_by_name() -> HashMap<String, u16> {
        // oct は QUALITIES に無い（1音なので何にでも一致して候補を埋め尽くす）
        // が、可動形の候補としては出てくるのでこの表には入れる
        let octave: &[Modifier] = &[Modifier::Octave];
        let mut out = HashMap::new();
        for root in 0..12u8 {
            let Ok(root_pitch) = Pitch::try_from(root) else {
                continue;
            };
            for quality in QUALITIES.iter().chain(std::iter::once(&octave)) {
                let ons = std::iter::once(None).chain((0..12u8).map(|b| Pitch::try_from(b).ok()));
                for on in ons {
                    let n = node(root_pitch, quality, on);
                    if let Ok(r) = chord_ref(&n) {
                        // or_insert で黙って先勝ちにすると、同じ名前に違う
                        // 構成音が割り当たっていても気付けない
                        if let Some(prev) = out.insert(r.label.clone(), r.pitch_classes) {
                            assert_eq!(
                                prev, r.pitch_classes,
                                "同じ名前 {} に違う構成音が割り当たっている",
                                r.label
                            );
                        }
                    }
                }
            }
        }
        out
    }

    /// 記法は必ず、それが名乗っているコードを指していること。
    /// `D5` のような裸の記法は読み直すと別のコード（メジャートライアド）に
    /// なるので、`pow()` で包まれていなければここで落ちる
    #[test]
    fn every_candidate_notation_denotes_its_own_chord() {
        let expected = pitch_classes_by_name();
        for f in FINGERINGS.iter() {
            for c in identify(f.frets) {
                let refs = reparse(&c.notation);
                assert_eq!(refs.len(), 1, "{} が1コードにならない", c.notation);
                let want = expected.get(&c.name).unwrap_or_else(|| {
                    panic!(
                        "候補名 {} が品質テーブルから引けない ({})",
                        c.name, c.notation
                    )
                });
                assert_eq!(
                    refs[0].pitch_classes,
                    *want,
                    "{} ({}) の候補 {} の記法 {} が別のコードに読まれる: \
                     読み直すと {{{}}}、{} が指すのは {{{}}}",
                    f.name,
                    tab(f.frets),
                    c.name,
                    c.notation,
                    notes(refs[0].pitch_classes),
                    c.name,
                    notes(*want)
                );
            }
        }
    }

    /// 決まった種から押さえ方を作る線形合同法。`rand` を足さずに済ませるためと、
    /// 落ちたときに同じ形をもう一度出せるようにするため
    struct Lcg(u64);

    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            self.0 >> 33
        }

        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }

        /// 適当な押さえ方。1/4 でミュート、それ以外は 0〜12フレット。
        /// 1本しか鳴らない形は名前の付けようがないので引き直す
        fn shape(&mut self) -> [Option<u8>; 6] {
            loop {
                let mut out = [None; 6];
                for slot in out.iter_mut() {
                    *slot = if self.below(4) == 0 {
                        None
                    } else {
                        Some(self.below(13) as u8)
                    };
                }
                if out.iter().filter(|v| v.is_some()).count() >= 2 {
                    return out;
                }
            }
        }
    }

    /// 指板は自由に押さえられるので、原典の115形だけ見ていても本番の入力を
    /// 覆えない。むしろ載っている形のほうが通り続けやすく、自由に押さえた形
    /// こそがこの機能の入力なので、そちらにも同じ不変条件をかけておく。
    ///
    /// ここでしか見ていない条件が1つある: 記法を読み直した `label` が
    /// `name` と一致すること（記法と名前が食い違っていないこと）。
    ///
    /// 100形で候補は約500件。`reparse` はパーサを丸ごと通すので件数がそのまま
    /// 時間になる。これ以上増やしてもテストの重さに見合わない
    #[test]
    fn arbitrary_shapes_keep_the_invariants() {
        let expected = pitch_classes_by_name();
        let mut rng = Lcg(0x5eed_1234_abcd_0001);
        for _ in 0..100 {
            let frets = rng.shape();
            let got = identify(frets);
            assert!(
                got.len() <= MAX_CANDIDATES,
                "{} で {} 件返った",
                tab(frets),
                got.len()
            );
            let mut seen: Vec<&str> = Vec::new();
            for c in &got {
                assert!(
                    !seen.contains(&c.name.as_str()),
                    "{} で名前が重複した: {}",
                    tab(frets),
                    c.name
                );
                seen.push(c.name.as_str());

                let refs = reparse(&c.notation);
                assert_eq!(
                    refs.len(),
                    1,
                    "{} の記法 {} が1コードにならない",
                    tab(frets),
                    c.notation
                );
                assert_eq!(
                    refs[0].label,
                    c.name,
                    "{} の記法 {} を読み直すと名前が {} に変わる",
                    tab(frets),
                    c.notation,
                    refs[0].label
                );
                let want = expected.get(&c.name).unwrap_or_else(|| {
                    panic!(
                        "候補名 {} が品質テーブルから引けない ({}, {})",
                        c.name,
                        c.notation,
                        tab(frets)
                    )
                });
                assert_eq!(
                    refs[0].pitch_classes,
                    *want,
                    "{} の候補 {} の記法 {} が別のコードに読まれる: \
                     読み直すと {{{}}}、{} が指すのは {{{}}}",
                    tab(frets),
                    c.name,
                    c.notation,
                    notes(refs[0].pitch_classes),
                    c.name,
                    notes(*want)
                );
            }
        }
    }

    /// データの全要素について、逆引きが1件は返し、Data と印が付いた候補の記法を
    /// 読み直して find() に流すと元の形が戻る
    #[test]
    fn every_shape_in_the_data_round_trips() {
        let mut with_data = 0;
        for f in FINGERINGS.iter() {
            let got = identify(f.frets);
            assert!(!got.is_empty(), "{} ({}) の候補が空", f.name, tab(f.frets));
            if got.iter().any(|c| matches!(c.source, Source::Data(_))) {
                with_data += 1;
            }
            for c in got.iter().filter(|c| matches!(c.source, Source::Data(_))) {
                let refs = reparse(&c.notation);
                assert_eq!(refs.len(), 1, "{} が1コードにならない", c.notation);
                let back =
                    find(&refs[0]).unwrap_or_else(|| panic!("{} の押さえ方が引けない", c.notation));
                assert_eq!(
                    back.frets, f.frets,
                    "{} -> {} で形が変わった",
                    f.name, c.notation
                );
            }
        }
        // 上のループは Data の候補が無い形では何も検査しない。空振りで通らない
        // よう、実際に検査できた形の数を押さえる。届かないのは3形だけ:
        // x3x01x（原典の C だが鳴るのは {C, G} なので C5 と読まれる）、
        // 3x003x（同じく G）、022000（原典の Em7、G/E と読まれる）。
        // いずれも find() がその名前に別の形を返すので記法が往復しない
        assert!(
            with_data >= 112,
            "Data の候補が出た形が {} 件しかない（112件以上のはず）",
            with_data
        );
    }
}
