use super::scale::Degree;
use std::fmt::Display;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Modifier {
    Major(u8),
    Minor(u8),
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
}

impl Display for Modifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Modifier::Major(5) => write!(f, ""),
            Modifier::Major(d) => write!(f, "{}", d),
            Modifier::Minor(d) => write!(f, "m{}", d),
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
            Modifier::Tension(Degree(a, d)) => write!(f, "{}{}", a, d),
        }
    }
}
