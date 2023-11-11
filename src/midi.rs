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

pub fn write_to_midi(f: &mut impl Write, score: &Score) -> Result<()> {
    let mut mfile = MidiFile::new();
    let mut track = Track::default();

    let ch = Channel::new(0);
    track.set_general_midi(ch, GeneralMidi::SynthVoice).unwrap();
    track.push_time_signature(0, 6, DurationName::Eighth, Clocks::DottedQuarter)?;
    track.push_tempo(0, QuartersPerMinute::new(score.bpm))?;

    const QUARTER: u32 = 1024;

    let mut skip = 0;
    for measure in score.chords.iter() {
        let dur = match measure.len() {
            1 => QUARTER * 4,
            2 => QUARTER * 2,
            4 => QUARTER,
            8 => QUARTER / 2,
            _ => {
                return Err(anyhow::anyhow!("invalid measure length: {:?}", measure));
            }
        };
        for chord in measure {
            let Some(chord) = chord else {
                skip += dur;
                continue;
            };
            let notes = chord
                .notes()
                .iter()
                .map(|n| note_to_pitch(n))
                .collect::<Vec<_>>();
            for n in notes.iter() {
                track
                    .push_note_on(skip, ch, n.clone(), Velocity::default())
                    .unwrap();
                skip = 0;
            }
            for (i, n) in notes.iter().enumerate() {
                track
                    .push_note_off(
                        if i == 0 { dur } else { 0 },
                        ch,
                        n.clone(),
                        Velocity::default(),
                    )
                    .unwrap();
            }
        }
    }

    mfile.push_track(track)?;
    mfile.write(f)?;
    Ok(())
}
