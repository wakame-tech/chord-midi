midi file:
    cargo run -p chord_midi_cli -- {{file}} midi --bpm 180 --output {{file_stem(file)}}.midi

as_degree file:
    RUST_BACKTRACE=1 cargo run -p chord_midi_cli -- {{file}} convert --as-degree C --output out.txt

as_pitch file:
    cargo run -p chord_midi_cli -- {{file}} convert --as-pitch C --output out.txt
