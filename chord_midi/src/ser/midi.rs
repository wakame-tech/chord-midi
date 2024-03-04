use crate::de::ast::Ast;
use crate::model::degree::Pitch;
use crate::model::score::{into_notes, Note};
use anyhow::Result;
use midi_file::core::{Channel, Clocks, DurationName, GeneralMidi, NoteNumber, Velocity};
use midi_file::file::{QuartersPerMinute, Track};
use midi_file::MidiFile;
use std::io::Write;

const UNIT: u32 = 1024 / 4;

fn write_notes(track: &mut Track, ch: Channel, semitones: &[NoteNumber], dur: u32, skip: &mut u32) {
    for (i, n) in semitones.iter().enumerate() {
        track
            .push_note_on(if i == 0 { *skip } else { 0 }, ch, *n, Velocity::default())
            .unwrap();
        *skip = 0;
    }
    for (i, n) in semitones.iter().enumerate() {
        track
            .push_note_off(if i == 0 { dur } else { 0 }, ch, *n, Velocity::default())
            .unwrap();
    }
}

pub fn dump(f: &mut impl Write, ast: Ast, key: Option<Pitch>, bpm: u8) -> Result<()> {
    let notes = into_notes(ast, key)?;
    dump_notes(f, &notes, bpm)
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
        let semitones = chord
            .semitones()?
            .into_iter()
            .map(NoteNumber::new)
            .collect::<Vec<_>>();
        write_notes(&mut track, ch, &semitones, dur, &mut skip);
    }

    mfile.push_track(track)?;
    mfile.write(f)?;
    Ok(())
}
