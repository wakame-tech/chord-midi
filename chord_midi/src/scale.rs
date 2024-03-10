#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Scale {
    Major,
    Minor,
}

impl Scale {
    pub fn degrees(&self) -> Vec<u8> {
        match self {
            Scale::Major => vec![2, 2, 1, 2, 2, 2, 1],
            Scale::Minor => vec![2, 1, 2, 2, 1, 2, 2],
        }
    }

    pub fn semitone(&self, degree: u8) -> u8 {
        let s = self.degrees();
        let mut semitone = 0;
        for i in 0..degree as usize - 1 {
            semitone += s[i % 7];
        }
        semitone
    }

    pub fn semitones(&self, degrees: &[u8]) -> Vec<u8> {
        degrees.iter().map(|d| self.semitone(*d)).collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::scale::Scale;

    #[test]
    fn test_semitone() {
        assert_eq!(Scale::Major.semitone(1), 0);
        assert_eq!(Scale::Major.semitone(2), 2);
        assert_eq!(Scale::Major.semitone(3), 4);
        assert_eq!(Scale::Major.semitone(4), 5);
    }
}
