use super::chord::Chord;
use crate::{
    de::score::{Measure, Node, AST},
    model::degree::Pitch,
};
use anyhow::Result;

#[derive(Debug, Clone)]
pub struct Note {
    pub chord: Option<Chord>,
    pub duration: u32,
}

impl Note {
    fn new(chord: Option<Chord>, duration: u32) -> Self {
        Note { chord, duration }
    }
}

#[derive(Debug)]
pub struct Score {
    notes: Vec<Note>,
    sustain: u32,
    rest: u32,
    pre: Option<Chord>,
}

impl Score {
    fn new() -> Self {
        Score {
            notes: vec![],
            sustain: 0,
            rest: 0,
            pre: None,
        }
    }

    fn interpret_node(&mut self, node: &Node, dur: u32) {
        log::trace!("{:?} sus={} rest={}", node, self.sustain, self.rest);
        if node != &Node::Sustain && self.sustain != 0 {
            log::trace!("push {:?} sus={}", self.pre, self.sustain);
            self.notes.push(Note::new(self.pre.clone(), self.sustain));
            self.sustain = 0;
        }
        if node != &Node::Rest && self.rest != 0 {
            log::trace!("push None sus={}", self.rest);
            self.notes.push(Note::new(None, self.rest));
            self.rest = 0;
        }
        match node {
            Node::Chord(node) => {
                let chord = Chord::new(3, node.root, Chord::degrees(&node.modifiers));
                self.pre = Some(chord.clone());
                self.sustain = dur;
            }
            Node::Degree(node) => {
                let chord = Chord::new(3, Pitch::C, Chord::degrees(&node.modifiers));
                self.pre = Some(chord.clone());
                self.sustain = dur;
            }
            Node::Repeat => {
                self.sustain = dur;
            }
            Node::Sustain => {
                self.sustain += dur;
            }
            Node::Rest => {
                self.rest += dur;
            }
        }
    }

    fn measure_unit_size(measure: &Measure) -> Result<u32> {
        const MEASURE_LENGTH: u32 = 16;
        let len = match measure.0.len() {
            1 => 1,
            2 => 2,
            3..=4 => 4,
            5..=8 => 8,
            9..=16 => 16,
            _ => {
                return Err(anyhow::anyhow!("too many nodes: {:?}", measure));
            }
        };
        Ok(MEASURE_LENGTH / len)
    }

    fn interpret(&mut self, ast: &AST) {
        for measure in &ast.0 {
            let dur = Self::measure_unit_size(&measure).unwrap();
            for node in &measure.0 {
                self.interpret_node(&node, dur);
            }
        }
        if self.sustain != 0 {
            self.notes.push(Note::new(self.pre.clone(), self.sustain));
        }
    }
}

pub fn into_notes(ast: &AST) -> Result<Vec<Note>> {
    let mut score = Score::new();
    score.interpret(ast);
    Ok(score.notes)
}
