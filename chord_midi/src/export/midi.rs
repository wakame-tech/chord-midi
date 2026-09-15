use super::Exporter;
use super::MidiExporter;
use crate::model::ast::{Beat, ChordNode, Expr, Func, Node, Pattern};
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

/// 開放弦の MIDI ノート番号。6弦→1弦 (E2 A2 D3 G3 B3 E4)
const OPEN_STRING_NOTES: [u8; 6] = [40, 45, 50, 55, 59, 64];

/// `ChordNode` を鳴らす音（MIDI ノート番号）に直す。低い方から並ぶ。
///
/// 押さえ方が見つかればその実音を鳴らす。プレビューが出している図と
/// 鳴っている音を一致させるため。見つからなければ従来の声部配置に落とす
fn voicing(node: &ChordNode) -> Result<Vec<NoteNumber>> {
    if let Some(f) = crate::fingering::find(&crate::fingering::chord_ref(node)?) {
        return Ok(f
            .frets
            .iter()
            .enumerate()
            .filter_map(|(i, fret)| fret.map(|n| NoteNumber::new(OPEN_STRING_NOTES[i] + n)))
            .collect());
    }
    into_note_numbers(&node.to_chord()?)
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
    /// 直前に鳴らした声部。`=` と `%` が伸ばす対象
    pre: Option<Vec<NoteNumber>>,
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
                .map(|notes| format!("{:?}", notes))
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
            self.notes.push(Note::new(self.pre.clone(), self.sustain));
            self.sustain = 0;
        }
        if !matches!(node, Node::Rest) && self.rest != 0 {
            self.notes.push(Note::new(None, self.rest));
            self.rest = 0;
        }
        match node {
            Node::Chord(node) => {
                log::debug!("chord: {}: {:?}", node, node.modifiers);
                self.pre = Some(voicing(&node)?);
                self.sustain = dur;
            }
            Node::Arpeggio(node) => {
                let all = voicing(&node)?;
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
                let notes = voicing(&node)?;
                self.pre = Some(notes.clone());
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

    fn interpret(&mut self, ast: Expr) -> Result<()> {
        match ast {
            Expr::Comment(_) => Ok(()),
            // resolve() 済みなら包みは消えている。念のため中身へ潜る
            Expr::Keyed(_, inner) | Expr::Scaled(_, inner) => self.interpret(*inner),
            Expr::Seq(score) => {
                for node in score.into_iter() {
                    self.interpret(node)?
                }
                if self.sustain != 0 {
                    self.notes.push(Note::new(self.pre.clone(), self.sustain));
                    self.sustain = 0;
                }
                Ok(())
            }
            Expr::Apply(Func::Pat(arg), inner, _) => {
                let pattern = arg.as_pattern()?.clone();
                self.interpret_with_pattern(*inner, &pattern)
            }
            Expr::Apply(_, inner, _) => self.interpret(*inner),
            // 展開の跡。どの定義から来たかは音に関係しないので中身へ潜るだけ
            Expr::Section(_, inner) => self.interpret(*inner),
            Expr::Measure(measure, _) => {
                let dur = Self::measure_unit_size(measure.len()).unwrap();
                for node in measure {
                    self.interpret_node(node, dur)?;
                }
                log::debug!("---");
                Ok(())
            }
            // `Func::Pat` の引数としてしか現れない。上の Apply(Func::Pat, ..)
            // の腕で消費されるので、ここに来るのは AST が壊れているとき
            Expr::Pattern(_) => Err(anyhow::anyhow!(
                "Expr::Pattern は pat() の引数以外の場所に現れてはいけない"
            )),
            // 定義と参照は expand が消す。ここに来るのは expand を通していないとき
            Expr::Define(name, _) => Err(anyhow::anyhow!(
                "定義 ${} が展開されていない。expand を通すこと",
                name
            )),
            Expr::Ref(name, _) => Err(anyhow::anyhow!(
                "参照 ${} が展開されていない。expand を通すこと",
                name
            )),
            // リピートも expand が消す
            Expr::Repeat(_, n, _) => Err(anyhow::anyhow!(
                "リピート x{} が展開されていない。expand を通すこと",
                n
            )),
        }
    }

    /// `pat(...)` の中身を、パターンに従って読み替えながら解釈する
    fn interpret_with_pattern(&mut self, ast: Expr, pattern: &Pattern) -> Result<()> {
        match ast {
            Expr::Measure(nodes, _) => self.interpret_patterned_measure(nodes, pattern),
            // `Apply` の中身は常に `Seq` なので、その中の各小節へパターンを配り直す
            Expr::Seq(items) => {
                for item in items {
                    self.interpret_with_pattern(item, pattern)?;
                }
                Ok(())
            }
            // 内側の pat が優先される
            Expr::Apply(Func::Pat(arg), items, _) => {
                let inner_pattern = arg.as_pattern()?.clone();
                self.interpret_with_pattern(*items, &inner_pattern)
            }
            Expr::Apply(_, items, _) => self.interpret_with_pattern(*items, pattern),
            // 展開の跡。ここで `interpret` に落とすと `pat()` のリズムが
            // 定義由来の小節にだけ掛からなくなるので、パターンを持ったまま潜る
            Expr::Section(_, inner) => self.interpret_with_pattern(*inner, pattern),
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
    fn export(&self, f: &mut impl Write, ast: Expr) -> anyhow::Result<()> {
        let ast = ast.resolve()?;
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
        let ast = RechordImporter
            .import("IVM7 | V7 | IIIm7 | VIm7 |\n")
            .unwrap();
        let mut score = Score::new();
        score.interpret(ast.resolve().unwrap()).unwrap();
        assert!(!score.notes.is_empty());
    }

    #[test]
    fn test_degree_progression_midi() {
        let ast = RechordImporter
            .import("key: C\nI | IV | V | I |\n")
            .unwrap();
        let mut score = Score::new();
        score.interpret(ast.resolve().unwrap()).unwrap();
        assert!(!score.notes.is_empty());
    }

    #[test]
    fn test_block_chord_is_simultaneous() {
        let notes = notes_of("C");
        assert_eq!(notes.len(), 1);
        // C の押さえ方 x-3-2-0-1-x は4本鳴る
        assert_eq!(notes[0].chord.as_ref().unwrap().len(), 4);
    }

    #[test]
    fn test_arpeggio_is_sequential() {
        let notes = notes_of("arp(C)");
        assert_eq!(notes.len(), 4); // 押さえ方の4音が順次
        assert!(notes.iter().all(|n| n.chord.as_ref().unwrap().len() == 1));
    }

    #[test]
    fn test_arpeggio_durations_sum_to_slot() {
        // single-node measure: dur = MEASURE_LENGTH = 16, C の押さえ方は4音
        let notes = notes_of("arp(C)");
        let durs: Vec<u32> = notes.iter().map(|n| n.duration).collect();
        assert_eq!(durs, vec![4, 4, 4, 4]);
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
        score.interpret(ast.resolve().unwrap()).unwrap();
        let chord_notes: Vec<_> = score.notes.iter().filter(|n| n.chord.is_some()).collect();
        assert_eq!(chord_notes.len(), 2);
    }

    fn resolved_notes_of(code: &str) -> Vec<Note> {
        let ast = RechordImporter.import(&format!("{}\n", code)).unwrap();
        let mut score = Score::new();
        score.interpret(ast.resolve().unwrap()).unwrap();
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

    /// 最初に鳴る和音の MIDI ノート番号
    fn pitches_of(code: &str) -> Vec<u8> {
        resolved_notes_of(code)
            .iter()
            .find(|n| n.chord.is_some())
            .expect("no sounding note")
            .chord
            .as_ref()
            .unwrap()
            .iter()
            .map(|n| n.get())
            .collect()
    }

    #[test]
    fn test_chord_sounds_the_fingering() {
        // C の押さえ方は x-3-2-0-1-x。
        // 5弦3fr=C3(48) 4弦2fr=E3(52) 3弦開放=G3(55) 2弦1fr=C4(60)
        assert_eq!(pitches_of("C |"), vec![48, 52, 55, 60]);
    }

    #[test]
    fn test_power_chord_sounds_the_fingering() {
        // 6弦5fr=A2(45) + 5弦7fr=E3(52)
        assert_eq!(pitches_of("pow(A@6)"), vec![45, 52]);
        // 5弦5fr=D3(50) + 4弦7fr=A3(57)
        assert_eq!(pitches_of("pow(D@5)"), vec![50, 57]);
        // 5弦7fr=E3(52) + 4弦9fr=B3(59)
        assert_eq!(pitches_of("pow(E@5)"), vec![52, 59]);
    }

    #[test]
    fn test_falls_back_when_no_fingering_matches() {
        // ルート1音だけの和音に合う押さえ方は無い (どの押さえ方も2音以上鳴る)
        // ので、従来の声部配置に落ちる
        let p = pitches_of("C#omit3omit5 |");
        assert_eq!(p.len(), 1, "{:?}", p);
    }

    #[test]
    fn test_voicing_is_ordered_low_to_high() {
        for code in ["C |", "Am7 |", "F |", "pow(A@6)"] {
            let p = pitches_of(code);
            assert!(p.windows(2).all(|w| w[0] < w[1]), "{}: {:?}", code, p);
        }
    }

    #[test]
    fn test_octave_shape_sounds_the_fingering() {
        // 6弦5fr=A2(45) + 4弦7fr=A3(57)
        assert_eq!(pitches_of("oct(A@6)"), vec![45, 57]);
        // 6弦4fr=G#2(44) + 4弦6fr=G#3(56)
        assert_eq!(pitches_of("oct(G#@6)"), vec![44, 56]);
        // 6弦9fr=C#3(49) + 4弦11fr=C#4(61)
        assert_eq!(pitches_of("oct(C#@6)"), vec![49, 61]);
    }
}
