use super::scale::Degree;
use std::fmt::Display;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Modifier {
    Major(u8),
    Minor(u8),
    /// dominant chord: bare number notation (e.g. `7`, `9`) → major triad + flat 7th
    Dominant(u8),
    MinorMajaor7,
    Sus2,
    Sus4,
    Flat5th,
    Aug,
    Aug7,
    Dim,
    Dim7,
    Omit(u8),
    Add(u8),
    Tension(Degree),
    /// パワーコード。ルートと5thだけを鳴らす。他の修飾を上書きするため
    /// enum の末尾に置いて BTreeSet の順で最後に適用させる
    Power,
    /// オクターブ奏法。ルートだけを鳴らす。Power と同じ理由で末尾に置き、
    /// BTreeSet の順で最後に適用させて他の修飾を上書きする
    Octave,
}

impl Display for Modifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Modifier::Major(5) => write!(f, ""),
            Modifier::Major(d) => write!(f, "maj{}", d),
            // 素のマイナートライアドは "Am5" ではなく "Am"。Major(5) と対称にする
            Modifier::Minor(5) => write!(f, "m"),
            Modifier::Minor(d) => write!(f, "m{}", d),
            Modifier::Dominant(d) => write!(f, "{}", d),
            Modifier::MinorMajaor7 => write!(f, "mM7"),
            Modifier::Sus2 => write!(f, "sus2"),
            Modifier::Sus4 => write!(f, "sus4"),
            Modifier::Flat5th => write!(f, "b5"),
            Modifier::Aug => write!(f, "aug"),
            Modifier::Aug7 => write!(f, "aug7"),
            Modifier::Dim => write!(f, "dim"),
            Modifier::Dim7 => write!(f, "dim7"),
            Modifier::Omit(d) => write!(f, "omit{}", d),
            Modifier::Add(d) => write!(f, "add{}", d),
            // Degree(number, accidental): notation writes the accidental first (e.g. "b9")
            Modifier::Tension(Degree(number, accidental)) => write!(f, "{}{}", accidental, number),
            Modifier::Power => write!(f, "5"),
            Modifier::Octave => write!(f, "oct"),
        }
    }
}
