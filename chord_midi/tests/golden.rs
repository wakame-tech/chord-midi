//! 既存の進行が同じ MIDI を吐き続けることの回帰テスト。
//!
//! スナップショットを作り直すときだけ `UPDATE_GOLDEN=1 cargo test -p chord_midi --test golden`。
//! 差分が出たのに意図した変更でないなら、それは後方互換の破壊。

use chord_midi::export::{Exporter, MidiExporter};
use chord_midi::import::{Importer, RechordImporter};
use chord_midi::presets::PRESETS;
use std::io::BufWriter;
use std::path::PathBuf;

/// README に載っている記法を一通り踏む
const SAMPLES: &[(&str, &str)] = &[
    ("plain", "key: C\nC | Am | F | G |\n"),
    ("degrees", "key: C\nI | VIm | IV | V |\n"),
    ("scale", "key: C\nscale: dorian\nI | III | IV | V |\n"),
    ("modifiers", "CM7 | Dm7 | G7(9) | C6 |\n"),
    ("slash", "C/E | F/G | Am/G | G |\n"),
    ("rest_sustain", "C | = | N.C. | % |\n"),
    ("arp", "arp(C) | arp(Am) |\n"),
    ("transforms", "blk(F) | s25(G7) | sub(G7) | dom(Dm7) |\n"),
    ("pow", "pow(C | E D)\n"),
    ("oct", "oct(A | G#)\n"),
    ("pat", "pat(\"o.o.oxox\", C | E D)\n"),
    ("pat_nested", "pat(\"oxox\", pow(C))\n"),
    ("root_string", "pow(A@6 | D@5 | E@5)\n"),
    ("comment", "# memo\nkey: C\nC | F |\n"),
    ("multiline", "C | Am |\nF | G |\n"),
];

fn midi_bytes(code: &str) -> Vec<u8> {
    let ast = RechordImporter.import(code).expect("parse");
    let mut writer = BufWriter::new(Vec::new());
    MidiExporter { bpm: 120 }
        .export(&mut writer, ast.resolve())
        .expect("export");
    writer.into_inner().expect("flush")
}

fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(format!("{}.mid", name))
}

fn check(name: &str, code: &str) {
    let actual = midi_bytes(code);
    let path = golden_path(name);
    if std::env::var("UPDATE_GOLDEN").is_ok() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &actual).unwrap();
        return;
    }
    let expected = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("golden 未生成 {}: {}。UPDATE_GOLDEN=1 で作る", name, e));
    assert_eq!(
        actual, expected,
        "{} の MIDI が変わった。意図した変更でなければ後方互換の破壊",
        name
    );
}

#[test]
fn samples_are_stable() {
    for (name, code) in SAMPLES {
        check(name, code);
    }
}

#[test]
fn presets_are_stable() {
    for preset in PRESETS.iter() {
        // プリセット名には空白や記号が入り得るのでファイル名としてサニタイズする
        let safe: String = preset
            .name
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect();
        check(
            &format!("preset_{}", safe),
            &format!("{}\n", preset.progression),
        );
    }
}
