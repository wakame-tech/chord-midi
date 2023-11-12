use crate::Score;
use anyhow::Result;
use midi_file::core::{Channel, Clocks, DurationName, GeneralMidi, NoteNumber, Velocity};
use midi_file::file::{QuartersPerMinute, Track};
use midi_file::MidiFile;
use rust_music_theory::note::Note;
use std::io::Write;

fn note_to_pitch(note: &Note) -> NoteNumber {
    NoteNumber::new(24 + 12 * note.octave + note.pitch_class.into_u8())
}

const UNIT: u32 = 1024 / 4;

pub fn write_to_midi(f: &mut impl Write, score: &Score) -> Result<()> {
    let mut mfile = MidiFile::new();
    let mut track = Track::default();

    let ch = Channel::new(0);
    track.set_general_midi(ch, GeneralMidi::SynthVoice).unwrap();
    track.push_time_signature(0, 6, DurationName::Sixteenth, Clocks::DottedQuarter)?;
    track.push_tempo(0, QuartersPerMinute::new(score.bpm))?;

    let mut skip = 0;
    for (chord, dur) in score.chords.iter() {
        let dur = *dur * UNIT;
        let Some(chord) = chord else {
            skip = dur;
            continue;
        };
        let notes = chord.notes().iter().map(note_to_pitch).collect::<Vec<_>>();
        for (i, n) in notes.iter().enumerate() {
            track
                .push_note_on(if i == 0 { skip } else { 0 }, ch, *n, Velocity::default())
                .unwrap();
            skip = 0;
        }
        for (i, n) in notes.iter().enumerate() {
            track
                .push_note_off(if i == 0 { dur } else { 0 }, ch, *n, Velocity::default())
                .unwrap();
        }
    }

    mfile.push_track(track)?;
    mfile.write(f)?;
    Ok(())
}
