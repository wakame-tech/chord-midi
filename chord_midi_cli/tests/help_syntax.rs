//! `--help-syntax` の配線テスト。表の中身は
//! `chord_midi::reference` のテストが見るので、ここは
//! 「入力ファイル無しで表が出て、正常終了する」ことだけ確かめる

use std::process::Command;

fn run() -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_chord_midi_cli"))
        .arg("--help-syntax")
        .output()
        .expect("run cli")
}

#[test]
fn help_syntax_prints_the_table() {
    let out = run();
    assert!(out.status.success(), "exit: {:?}", out.status);
    let text = String::from_utf8(out.stdout).expect("utf8");
    for needle in ["pow", "pat", "arp", "xN", "N.C.", "記号", "関数"] {
        assert!(
            text.contains(needle),
            "`{}` が出ていない:\n{}",
            needle,
            text
        );
    }
}

#[test]
fn help_syntax_does_not_need_an_input_file() {
    // `--list-presets` と同じ扱い。`--input` が無くてもエラーにしない
    let out = run();
    assert!(
        out.stderr.is_empty(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}
