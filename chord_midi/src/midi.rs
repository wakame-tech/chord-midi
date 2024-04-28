use crate::import::ast::Ast;
use crate::import::ast::Node;
use crate::model::chord::Chord;
use crate::model::key::Key;
use anyhow::anyhow;
use anyhow::Result;
use midi_file::core::{Channel, Clocks, DurationName, GeneralMidi, NoteNumber, Velocity};
use midi_file::file::{QuartersPerMinute, Track};
use midi_file::MidiFile;
use std::io::Write;

const UNIT: u32 = 1024 / 4;

fn into_note_numbers(chord: &Chord) -> Result<Vec<NoteNumber>> {
    let Key::Absolute(p) = chord.key else {
        return Err(anyhow!("Relative key is not supported"));
    };
    let on = chord.on.clone().and_then(|on| {
        if let Key::Absolute(on) = on {
            Some(on)
        } else {
            None
        }
    });
    let mut semitones = chord
        .semitones
        .iter()
        // C4 is 60
        .map(|s| 12 * chord.octave + (p as u8) + *s)
        .collect::<Vec<_>>();
    for _ in 0..chord.inversion {
        let n = semitones.remove(0);
        semitones.push(n + 12);
    }
    if let Some(on) = on {
        semitones.push(12 * (chord.octave - 1) + (on as u8));
    }
    Ok(semitones
        .into_iter()
        .map(|s| NoteNumber::new(12 + s))
        .collect())
}

#[derive(Debug, Clone)]
struct Note {
    pub chord: Option<Vec<NoteNumber>>,
    pub duration: u32,
}

impl Note {
    fn new(chord: Option<Vec<NoteNumber>>, duration: u32) -> Self {
        Note { chord, duration }
    }
}

#[derive(Debug)]
struct Score {
    notes: Vec<Note>,
    sustain: u32,
    rest: u32,
    pre: Option<Chord>,
}

const MEASURE_LENGTH: u32 = 16;

impl Score {
    fn new() -> Self {
        Score {
            notes: vec![],
            sustain: 0,
            rest: 0,
            pre: None,
        }
    }

    fn inspect(&self) {
        log::debug!(
            "pre={} sus={}/{}, rest={}/{}",
            self.pre
                .as_ref()
                .map(|c| format!("{:?}", c))
                .unwrap_or("None".to_string()),
            self.sustain,
            MEASURE_LENGTH,
            self.rest,
            MEASURE_LENGTH
        );
    }

    fn interpret_node(&mut self, node: Node, dur: u32) -> Result<()> {
        self.inspect();
        if !matches!(node, Node::Sustain) && self.sustain != 0 {
            let notes = if let Some(pre) = &self.pre {
                Some(into_note_numbers(pre)?)
            } else {
                None
            };
            self.notes.push(Note::new(notes, self.sustain));
            self.sustain = 0;
        }
        if !matches!(node, Node::Rest) && self.rest != 0 {
            self.notes.push(Note::new(None, self.rest));
            self.rest = 0;
        }
        match node {
            Node::Chord(node) => {
                log::debug!("chord: {}: {:?}", node, node.modifiers);
                let chord = node.to_chord()?;
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
                if self.sustain != 0 {
                    let notes = if let Some(pre) = &self.pre {
                        Some(into_note_numbers(pre)?)
                    } else {
                        None
                    };
                    self.notes.push(Note::new(notes, self.sustain));
                    self.sustain = 0;
                }
                Ok(())
            }
            Ast::Measure(measure, _) => {
                let dur = Self::measure_unit_size(measure.len()).unwrap();
                for node in measure {
                    self.interpret_node(node, dur)?;
                }
                log::debug!("---");
                Ok(())
            }
        }
    }
}

fn write_notes(track: &mut Track, ch: Channel, notes: &[NoteNumber], dur: u32, skip: &mut u32) {
    for (i, n) in notes.iter().enumerate() {
        track
            .push_note_on(if i == 0 { *skip } else { 0 }, ch, *n, Velocity::default())
            .unwrap();
        *skip = 0;
    }
    for (i, n) in notes.iter().enumerate() {
        track
            .push_note_off(if i == 0 { dur } else { 0 }, ch, *n, Velocity::default())
            .unwrap();
    }
}

pub fn dump(f: &mut impl Write, ast: Ast, bpm: u8) -> Result<()> {
    let mut score = Score::new();
    score.interpret(ast)?;
    dump_notes(f, &score.notes, bpm)
}

fn dump_notes(f: &mut impl Write, notes: &[Note], bpm: u8) -> Result<()> {
    let mut mfile = MidiFile::new();
    let mut track = Track::default();
    let ch = Channel::new(0);

    track.set_general_midi(ch, GeneralMidi::SynthVoice).unwrap();
    track.push_time_signature(0, 6, DurationName::Sixteenth, Clocks::DottedQuarter)?;
    track.push_tempo(0, QuartersPerMinute::new(bpm))?;

    let mut skip = 0;
    for note in notes {
        let dur = note.duration * UNIT;
        let Some(chord) = &note.chord else {
            skip = dur;
            continue;
        };
        write_notes(&mut track, ch, chord, dur, &mut skip);
    }

    mfile.push_track(track)?;
    mfile.write(f)?;
    Ok(())
}
