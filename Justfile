midi file:
    cargo run -p chord_midi_cli -- -i {{file}} --bpm 180 -o {{file_stem(file)}}.midi

rechord file:
    cargo run -p chord_midi_cli -- -i {{file}} --bpm 180 -o {{file_stem(file)}}.txt

gen-fingerings:
    python3 scripts/gen_fingerings.py
    cargo fmt -p chord_midi
