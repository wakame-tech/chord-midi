midi file:
    cargo run -p chord_midi_cli -- -i {{file}} --bpm 180 -o {{file_stem(file)}}.midi

rechord file:
    cargo run -p chord_midi_cli -- -i {{file}} --bpm 180 -o {{file_stem(file)}}.txt

fingering file:
    cargo run -p chord_midi_cli -- -i {{file}} --fingering

gen-fingerings:
    python3 scripts/gen_fingerings.py
    rustfmt --edition 2021 chord_midi/src/fingering/data.rs

gen-presets:
    python3 scripts/gen_presets.py
    rustfmt --edition 2021 chord_midi/src/presets.rs
