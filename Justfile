midi file:
    cargo run -p chord_midi_cli -- -i {{file}} --bpm 180 -o {{file_stem(file)}}.midi

rechord file:
    cargo run -p chord_midi_cli -- -i {{file}} --bpm 180 -o {{file_stem(file)}}.txt

gen-fingerings:
    python3 scripts/gen_fingerings.py
    rustfmt --edition 2021 chord_midi/src/fingering/data.rs

gen-presets:
    python3 scripts/gen_presets.py
    rustfmt --edition 2021 chord_midi_web/src/presets.rs
