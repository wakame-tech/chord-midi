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
