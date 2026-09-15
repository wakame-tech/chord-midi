//! 押さえ方からコード名を引く。[`super::find`] の逆。

use super::{Fingering, OPEN_STRINGS, OPEN_STRING_PITCHES};

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

/// 押さえ方からコード名候補を引く。frets は 6弦→1弦、None = ミュート
pub fn identify(frets: [Option<u8>; 6]) -> Vec<Candidate> {
    let Some(_played) = played(&frets) else {
        return Vec::new();
    };
    // 中身は次のタスクで入れる。今は「鳴っていれば1件は返す」だけ
    vec![Candidate {
        name: String::new(),
        notation: String::new(),
        source: Source::Theory,
        unreachable: true,
        score: u32::MAX,
    }]
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
    fn identify_returns_something_for_a_real_chord() {
        // C コード。名前が何になるかは次のタスクで詰める
        assert!(!identify(frets("x3201x")).is_empty());
    }
}
