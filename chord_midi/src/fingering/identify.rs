//! 押さえ方からコード名を引く。[`super::find`] の逆。

use super::{
    chord_ref, extra_cost, missing_cost, relative_semitones, ChordRef, Fingering, OPEN_STRINGS,
    OPEN_STRING_PITCHES,
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
    /// `notation` を書いても押さえた形には戻らない（今のデータで再現できない形）
    pub unreachable: bool,
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
    let mut lowest: Option<(u8, u8, u8)> = None; // (midi, ピッチクラス, 弦)
    for (s, fret) in frets.iter().enumerate() {
        let Some(fret) = fret else { continue };
        let midi = OPEN_STRING_PITCHES[s] + fret;
        let pc = (OPEN_STRINGS[s] + fret) % 12;
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
    &[Modifier::Power],                       // C5 (記法は pow(C)。Task 6 で包む)
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
        3
    } else {
        0
    };
    // 最低音がルートでないのにスラッシュで書いていない
    let bass_mismatch = if chord.bass.is_none() && played.bass != chord.root {
        2
    } else {
        0
    };
    Some(extra + missing + root_silent + bass_mismatch)
}

/// 押さえ方からコード名候補を引く。frets は 6弦→1弦、None = ミュート
pub fn identify(frets: [Option<u8>; 6]) -> Vec<Candidate> {
    let Some(played) = played(&frets) else {
        return Vec::new();
    };

    // (score, QUALITIES での並び順, ノード, 検索用の情報)
    let mut scored: Vec<(u32, usize, ChordNode, ChordRef)> = Vec::new();
    for root in 0..12u8 {
        let Ok(root_pitch) = Pitch::try_from(root) else {
            continue;
        };
        for (qi, quality) in QUALITIES.iter().enumerate() {
            // `on` 無しと、最低音がルートと違うときの `on` 付きを両方試す。
            // `C/E` と `C` はどちらも正しい呼び方なので、どちらが上に来るかは
            // 採点に任せる
            let mut ons: Vec<Option<Pitch>> = vec![None];
            if played.bass != root {
                if let Ok(bass) = Pitch::try_from(played.bass) {
                    ons.push(Some(bass));
                }
            }
            for on in ons {
                let node = node(root_pitch, quality, on);
                let Ok(r) = chord_ref(&node) else { continue };
                let Some(s) = score(&played, &r) else { continue };
                scored.push((s, qi, node, r));
            }
        }
    }
    scored.sort_by_key(|(s, qi, _, _)| (*s, *qi));

    let mut out: Vec<Candidate> = Vec::new();
    for (s, _, node, r) in scored {
        if out.len() >= MAX_CANDIDATES {
            break;
        }
        if out.iter().any(|c| c.name == r.label) {
            continue;
        }
        out.push(Candidate {
            name: r.label.clone(),
            notation: node.to_string(),
            source: Source::Theory,
            unreachable: false,
            score: s,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// "x3201x" のような文字列を frets に直す。x = ミュート、それ以外は1桁の16進
    fn frets(s: &str) -> [Option<u8>; 6] {
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
    fn identify_names_an_open_c() {
        let c = identify(frets("x3201x"));
        assert_eq!(c[0].name, "C");
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
        assert!(identify(frets("x3201x")).len() <= 5);
    }

    #[test]
    fn identify_offers_a_slash_chord_when_the_bass_is_not_the_root() {
        // x32010 は 5弦3fr(C) が最低音なので C のまま。1弦を開放にした
        // "332010" は 6弦3fr = G が最低音になる C/G
        let c = identify(frets("332010"));
        assert!(
            c.iter().any(|x| x.name == "C/G"),
            "C/G が候補に無い: {:?}",
            c.iter().map(|x| &x.name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn identify_prefers_the_slash_name_when_the_bass_is_not_the_root() {
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
}
