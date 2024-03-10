#[derive(Debug)]
pub enum Scale {
    Major,
    Minor,
}

impl Scale {
    pub fn degrees(&self) -> Vec<u8> {
        match self {
            Scale::Major => vec![0, 2, 4, 5, 7, 9, 11],
            Scale::Minor => vec![0, 2, 3, 5, 7, 8, 10],
        }
    }

    pub fn semitone(&self, degree: u8) -> u8 {
        let mut semitone = 0;
        for i in 0..degree as usize - 1 {
            semitone += self.degrees()[i % 8];
        }
        semitone
    }

    pub fn semitones(&self, degrees: &[u8]) -> Vec<u8> {
        degrees.iter().map(|d| self.semitone(*d)).collect()
    }
}
