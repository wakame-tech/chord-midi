use super::chord::Chord;
use crate::{
    de::ast::{Ast, Node},
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
    key: Option<Pitch>,
    notes: Vec<Note>,
    sustain: u32,
    rest: u32,
    pre: Option<Chord>,
}

impl Score {
    fn new(key: Option<Pitch>) -> Self {
        Score {
            key,
            notes: vec![],
            sustain: 0,
            rest: 0,
            pre: None,
        }
    }

    fn interpret_node(&mut self, node: Node, dur: u32) -> Result<()> {
        log::trace!("{:?} sus={} rest={}", node, self.sustain, self.rest);
        if node != Node::Sustain && self.sustain != 0 {
            log::trace!("push {:?} sus={}", self.pre, self.sustain);
            self.notes.push(Note::new(self.pre.clone(), self.sustain));
            self.sustain = 0;
        }
        if node != Node::Rest && self.rest != 0 {
            log::trace!("push None sus={}", self.rest);
            self.notes.push(Note::new(None, self.rest));
            self.rest = 0;
        }
        match node {
            Node::Chord(node) => {
                let chord = node.into_chord(5)?;
                log::debug!("-> {}", chord);
                self.pre = Some(chord.clone());
                self.sustain = dur;
            }
            Node::Degree(node) => {
                let Some(key) = self.key else {
                    return Err(anyhow::anyhow!("key is not set"));
                };
                let chord = node.into_chord(key, 5)?;
                log::debug!("-> {}", chord);
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
        Ok(())
    }

    fn measure_unit_size(n: usize) -> Result<u32> {
        const MEASURE_LENGTH: u32 = 16;
        let len = match n {
            1 => 1,
            2 => 2,
            3..=4 => 4,
            5..=8 => 8,
            9..=16 => 16,
            _ => {
                return Err(anyhow::anyhow!("too many nodes: {}", n));
            }
        };
        Ok(MEASURE_LENGTH / len)
    }

    fn interpret(&mut self, ast: Ast) -> Result<()> {
        match ast {
            Ast::Comment(_) => Ok(()),
            Ast::Score(score) => {
                for node in score.into_iter() {
                    self.interpret(*node)?
                }
                Ok(())
            }
            Ast::Measure(measure, _) => {
                let dur = Self::measure_unit_size(measure.len()).unwrap();
                for node in measure {
                    self.interpret_node(node, dur)?;
                }
                if self.sustain != 0 {
                    self.notes.push(Note::new(self.pre.clone(), self.sustain));
                }
                Ok(())
            }
        }
    }
}

pub fn into_notes(ast: Ast, key: Option<Pitch>) -> Result<Vec<Note>> {
    let mut score = Score::new(key);
    score.interpret(ast)?;
    Ok(score.notes)
}
