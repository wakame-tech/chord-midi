use super::Exporter;
use super::MidiExporter;
use crate::model::ast::{Ast, Beat, Func, Node, Pattern};
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

/// ブラッシング（ミュートした空振り）のベロシティ
const BRUSH_VELOCITY: Velocity = Velocity::new(40);

#[derive(Debug, Clone)]
struct Note {
    pub chord: Option<Vec<NoteNumber>>,
    pub duration: u32,
    pub velocity: Velocity,
}

impl Note {
    fn new(chord: Option<Vec<NoteNumber>>, duration: u32) -> Self {
        Note {
            chord,
            duration,
            velocity: Velocity::default(),
        }
    }

    fn brushed(chord: Option<Vec<NoteNumber>>, duration: u32) -> Self {
        Note {
            chord,
            duration,
            velocity: BRUSH_VELOCITY,
        }
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
            Node::Arpeggio(node) => {
                let chord = node.to_chord()?;
                let all = into_note_numbers(&chord)?;
                // dur が音数より少ない密な小節では鳴らせる音数を dur に制限し、
                // 総尺が dur を超えて後続ノードがずれるのを防ぐ（総尺は常に dur）
                let k = (all.len() as u32).min(dur).max(1);
                let each = dur / k;
                let rem = dur % k;
                for (i, n) in all.into_iter().take(k as usize).enumerate() {
                    let d = each + if i as u32 == k - 1 { rem } else { 0 };
                    self.notes.push(Note::new(Some(vec![n]), d));
                }
            }
            Node::Transform(t, c) => {
                // keys are already Absolute after resolve(); Absolute shift ignores tonic/scale
                let nodes = t.expand(
                    c,
                    crate::model::pitch::Pitch::C,
                    crate::model::scale::Scale::Major,
                );
                // cap the expanded-node count to the slot so a dense measure can't overrun
                // (mirrors the arpeggio guard); each sub-chord gets an equal share of dur
                let k = (nodes.len() as u32).min(dur).max(1);
                let each = dur / k;
                let rem = dur % k;
                for (i, n) in nodes.into_iter().take(k as usize).enumerate() {
                    let d = each + if i as u32 == k - 1 { rem } else { 0 };
                    self.interpret_node(n, d)?;
                }
            }
            Node::Brush(node) => {
                let chord = node.to_chord()?;
                let notes = into_note_numbers(&chord)?;
                self.pre = Some(chord);
                self.notes.push(Note::brushed(Some(notes), dur));
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
            Ast::Directive(_) => Ok(()),
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
            Ast::Apply(Func::Pat(pattern), items, _) => {
                for item in items {
                    self.interpret_with_pattern(*item, &pattern)?;
                }
                Ok(())
            }
            Ast::Apply(_, items, _) => {
                // pow は resolve() で消えているが、素の AST を渡された場合に備える
                for item in items {
                    self.interpret(*item)?;
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

    /// `pat(...)` の中身を、パターンに従って読み替えながら解釈する
    fn interpret_with_pattern(&mut self, ast: Ast, pattern: &Pattern) -> Result<()> {
        match ast {
            Ast::Measure(nodes, _) => self.interpret_patterned_measure(nodes, pattern),
            // 内側の pat が優先される
            Ast::Apply(Func::Pat(inner), items, _) => {
                for item in items {
                    self.interpret_with_pattern(*item, &inner)?;
                }
                Ok(())
            }
            Ast::Apply(_, items, _) => {
                for item in items {
                    self.interpret_with_pattern(*item, pattern)?;
                }
                Ok(())
            }
            other => self.interpret(other),
        }
    }

    /// 1小節をパターンのスロット数に割り直す
    fn interpret_patterned_measure(&mut self, nodes: Vec<Node>, pattern: &Pattern) -> Result<()> {
        let slots = pattern.0.len() as u32;
        // パターン長は 1/2/4/8/16 に限られるので必ず割り切れる
        let slot_dur = MEASURE_LENGTH / slots;
        // 元の小節で1ノードが占める尺
        let node_dur = Self::measure_unit_size(nodes.len())?;
        for (i, beat) in pattern.0.iter().enumerate() {
            let at = i as u32 * slot_dur;
            let index = ((at / node_dur) as usize).min(nodes.len() - 1);
            let node = nodes[index].clone();
            let node = match (beat, node) {
                // 元が休符のところは打弦もブラッシングもしない
                (_, Node::Rest) => Node::Rest,
                (Beat::Rest, _) => Node::Rest,
                (Beat::Hit, Node::Brush(c)) => Node::Chord(c),
                (Beat::Hit, other) => other,
                (Beat::Brush, Node::Chord(c)) => Node::Brush(c),
                (Beat::Brush, Node::Arpeggio(c)) => Node::Brush(c),
                (Beat::Brush, Node::Transform(_, c)) => Node::Brush(c),
                (Beat::Brush, other) => other,
            };
            self.interpret_node(node, slot_dur)?;
        }
        Ok(())
    }
}

fn write_notes(
    track: &mut Track,
    ch: Channel,
    notes: &[NoteNumber],
    dur: u32,
    velocity: Velocity,
    skip: &mut u32,
) {
    for (i, n) in notes.iter().enumerate() {
        track
            .push_note_on(if i == 0 { *skip } else { 0 }, ch, *n, velocity)
            .unwrap();
        *skip = 0;
    }
    for (i, n) in notes.iter().enumerate() {
        track
            .push_note_off(if i == 0 { dur } else { 0 }, ch, *n, Velocity::default())
            .unwrap();
    }
}

impl Exporter for MidiExporter {
    fn export(&self, f: &mut impl Write, ast: Ast) -> anyhow::Result<()> {
        let ast = ast.resolve();
        let mut score = Score::new();
        score.interpret(ast)?;
        dump_notes(f, &score.notes, self.bpm)
    }
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
        write_notes(&mut track, ch, chord, dur, note.velocity, &mut skip);
    }

    mfile.push_track(track)?;
    mfile.write(f)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Note, Score};
    use crate::import::{Importer, RechordImporter};

    fn notes_of(code: &str) -> Vec<Note> {
        let ast = RechordImporter.import(&format!("{}\n", code)).unwrap();
        let mut score = Score::new();
        score.interpret(ast).unwrap();
        score.notes
    }

    #[test]
    fn test_transform_dense_measure_no_overrun() {
        // 9 nodes -> dur 1 each; each s25 (2 chords) is capped to 1 chord/slot,
        // so total duration stays 9 (un-capped it would overrun to 18)
        let notes =
            notes_of("s25(G7) s25(G7) s25(G7) s25(G7) s25(G7) s25(G7) s25(G7) s25(G7) s25(G7)");
        assert_eq!(notes.iter().map(|n| n.duration).sum::<u32>(), 9);
    }

    #[test]
    fn test_keyless_degree_progression_plays_in_c() {
        // no key: directive -> resolve defaults to C, produces notes (must not panic)
        let ast = RechordImporter.import("IVM7 | V7 | IIIm7 | VIm7 |\n").unwrap();
        let mut score = Score::new();
        score.interpret(ast.resolve()).unwrap();
        assert!(!score.notes.is_empty());
    }

    #[test]
    fn test_degree_progression_midi() {
        let ast = RechordImporter.import("key: C\nI | IV | V | I |\n").unwrap();
        let mut score = Score::new();
        score.interpret(ast.resolve()).unwrap();
        assert!(!score.notes.is_empty());
    }

    #[test]
    fn test_block_chord_is_simultaneous() {
        let notes = notes_of("C");
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].chord.as_ref().unwrap().len(), 3); // C E G
    }

    #[test]
    fn test_arpeggio_is_sequential() {
        let notes = notes_of("arp(C)");
        assert_eq!(notes.len(), 3); // 3音が順次
        assert!(notes.iter().all(|n| n.chord.as_ref().unwrap().len() == 1));
    }

    #[test]
    fn test_arpeggio_durations_sum_to_slot() {
        // single-node measure: dur = MEASURE_LENGTH = 16, triad k=3
        let notes = notes_of("arp(C)");
        let durs: Vec<u32> = notes.iter().map(|n| n.duration).collect();
        assert_eq!(durs, vec![5, 5, 6]); // each=5, remainder folded into last
        assert_eq!(durs.iter().sum::<u32>(), 16);
    }

    #[test]
    fn test_arpeggio_dense_measure_no_overrun() {
        // 5 nodes -> dur = 2 each; triad capped to k=2, so 2 notes/arp, 2 units/arp
        let notes = notes_of("arp(C) arp(C) arp(C) arp(C) arp(C)");
        assert!(notes.iter().all(|n| n.chord.as_ref().unwrap().len() == 1));
        // total duration must equal 5 nodes * dur(2) = 10 (old buggy code gave 25)
        assert_eq!(notes.iter().map(|n| n.duration).sum::<u32>(), 10);
    }

    #[test]
    fn test_s25_produces_two_chords() {
        let notes = notes_of("s25(G7)");
        let chord_notes: Vec<_> = notes.iter().filter(|n| n.chord.is_some()).collect();
        assert_eq!(chord_notes.len(), 2);
    }

    #[test]
    fn test_blk_produces_one_four_note_chord() {
        let notes = notes_of("blk(F)");
        let chord_notes: Vec<_> = notes.iter().filter(|n| n.chord.is_some()).collect();
        assert_eq!(chord_notes.len(), 1);
        // Baug/F = F(bass) + B D# G = 4 notes
        assert_eq!(chord_notes[0].chord.as_ref().unwrap().len(), 4);
    }

    #[test]
    fn test_transform_with_key_directive() {
        // degree + key + transform: key C, s25(V7) -> Dm7 G7 (2 chords)
        let ast = RechordImporter.import("key: C\ns25(V7) |\n").unwrap();
        let mut score = Score::new();
        score.interpret(ast.resolve()).unwrap();
        let chord_notes: Vec<_> = score.notes.iter().filter(|n| n.chord.is_some()).collect();
        assert_eq!(chord_notes.len(), 2);
    }

    fn resolved_notes_of(code: &str) -> Vec<Note> {
        let ast = RechordImporter.import(&format!("{}\n", code)).unwrap();
        let mut score = Score::new();
        score.interpret(ast.resolve()).unwrap();
        score.notes
    }

    #[test]
    fn test_pat_splits_measure_into_slots() {
        // "o.o." は4分割。打弦は2回で、それぞれ1スロットぶんの尺になる。
        // 休符は連続すると1つの Note にまとまるので、鳴っている音だけを見る
        let notes = resolved_notes_of("pat(\"o.o.\", C)");
        let sounding: Vec<_> = notes.iter().filter(|n| n.chord.is_some()).collect();
        assert_eq!(sounding.len(), 2);
        assert!(sounding.iter().all(|n| n.duration == 4), "{:?}", sounding);
    }

    #[test]
    fn test_pat_hit_count_matches_pattern() {
        let notes = resolved_notes_of("pat(\"oooo\", C)");
        assert_eq!(notes.iter().filter(|n| n.chord.is_some()).count(), 4);
        let notes = resolved_notes_of("pat(\"o...\", C)");
        assert_eq!(notes.iter().filter(|n| n.chord.is_some()).count(), 1);
    }

    #[test]
    fn test_pat_total_duration_matches_plain_measure() {
        // 全スロットで打弦するパターンなら、総尺はパターン無しと一致する
        let plain = resolved_notes_of("C | F |")
            .iter()
            .map(|n| n.duration)
            .sum::<u32>();
        let patterned = resolved_notes_of("pat(\"oooo\", C | F)")
            .iter()
            .map(|n| n.duration)
            .sum::<u32>();
        assert_eq!(plain, patterned);
    }

    #[test]
    fn test_pat_brush_is_quieter() {
        let notes = resolved_notes_of("pat(\"ox\", C)");
        let sounding: Vec<_> = notes.iter().filter(|n| n.chord.is_some()).collect();
        assert_eq!(sounding.len(), 2);
        assert!(sounding[0].velocity > sounding[1].velocity);
    }

    #[test]
    fn test_pat_follows_the_chord_at_each_slot() {
        // E D の小節に4スロット。前半が E、後半が D
        let notes = resolved_notes_of("pat(\"oooo\", E D)");
        assert_eq!(notes.len(), 4);
        let first = notes[0].chord.clone().unwrap();
        let second = notes[1].chord.clone().unwrap();
        let third = notes[2].chord.clone().unwrap();
        let fourth = notes[3].chord.clone().unwrap();
        assert_eq!(first, second, "前半2スロットは同じコード");
        assert_eq!(third, fourth, "後半2スロットは同じコード");
        assert_ne!(first, third, "前半と後半は違うコード");
    }

    #[test]
    fn test_pat_keeps_rest_as_rest() {
        // 元が休符の位置は o でも鳴らさない。鳴るのは後半2スロットだけ
        let notes = resolved_notes_of("pat(\"oooo\", _ C)");
        let sounding: Vec<_> = notes.iter().filter(|n| n.chord.is_some()).collect();
        assert_eq!(sounding.len(), 2);
        assert!(sounding.iter().all(|n| n.duration == 4));
    }
}
