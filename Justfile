midi file:
    cargo run -p chord_midi_cli -- -i {{file}} --bpm 180 -o {{file_stem(file)}}.midi

rechord file:
    cargo run -p chord_midi_cli -- -i {{file}} --bpm 180 -o {{file_stem(file)}}.txt