use crate::model::{
    chord::{match_pitches, Chord},
    key::Key,
    modifier::Modifier,
    pitch::Pitch,
};
use anyhow::Result;
use std::collections::BTreeSet;

#[derive(Debug, PartialEq)]
pub enum Ast {
    Comment(String),
    // nodes, br?
    Measure(Vec<Node>, bool),
    Score(Vec<Box<Ast>>),
}

#[derive(Debug, PartialEq)]
pub enum Node {
    Chord(ChordNode),
    Rest,
    Sustain,
    Repeat,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChordNode {
    pub key: Key,
    pub modifiers: BTreeSet<Modifier>,
    pub on: Option<Key>,
}

impl ChordNode {
    pub fn absolute(pitch: Pitch) -> Self {
        ChordNode {
            key: Key::Absolute(pitch),
            modifiers: BTreeSet::new(),
            on: None,
        }
    }

    pub fn relative(semitone: u8) -> Self {
        ChordNode {
            key: Key::Relative(semitone),
            modifiers: BTreeSet::new(),
            on: None,
        }
    }

    pub fn to_chord(&self) -> Result<Chord> {
        let mut chord = Chord::new(5, 0, self.key.clone());
        chord.on = self.on.clone();
        for modifier in &self.modifiers {
            chord.modify(modifier)?;
        }
        let (octave, inversion) = match_pitches(12 * chord.octave, &chord)?;
        chord.octave = octave;
        chord.inversion = inversion;
        Ok(chord)
    }
}
