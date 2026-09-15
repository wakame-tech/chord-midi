//! 既存の進行が同じ MIDI を吐き続けることの回帰テスト。
//!
//! スナップショットを作り直すときだけ `UPDATE_GOLDEN=1 cargo test -p chord_midi --test golden`。
//! 差分が出たのに意図した変更でないなら、それは後方互換の破壊。

use chord_midi::export::{Exporter, MidiExporter};
use chord_midi::import::{Importer, RechordImporter};
use chord_midi::presets::PRESETS;
use std::collections::HashSet;
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
    // 入れ子の向きを逆にした形。pow/oct が外、pat が内。
    // 修飾が pat の包む Seq を越えて内側のコードまで届くかを見る
    ("pow_pat", "pow(pat(\"oxox\", C))\n"),
    ("oct_pat", "oct(pat(\"oxox\", A))\n"),
    // 参照の中身にも関数の修飾が届くこと。mark が Section を越えられないと
    // パワーコードが三和音のまま鳴る
    ("pow_ref", "$a = C\npow($a)\n"),
    ("oct_ref", "$b = A\noct($b)\n"),
    // pat の中の変換。ブラッシングのスロットで変換が捨てられていたのと、
    // s25 が全スロットで2コードとも鳴っていたのを固定する
    ("pat_blk", "pat(\"o.ox\", blk(F))\n"),
    ("pat_s25", "pat(\"oooo\", s25(G7))\n"),
    ("pat_dom_brush", "pat(\"xxxx\", dom(Dm7))\n"),
    // Phase 2 の記法
    ("define_ref", "$riff = pow(C | E D)\n$riff | C |\n"),
    (
        "define_pattern",
        "$eight = \"o.o.o.o.\"\npat($eight, C | Am)\n",
    ),
    ("repeat_measure", "C x4 |\n"),
    ("repeat_group", "(C | Am | F | G) x2\n"),
    ("nested_define", "$a = C\n$b = $a | $a\n$b x2\n"),
    ("define_under_key", "key: C\n$a = I | IV |\n$a x2\n"),
    ("root_string", "pow(A@6 | D@5 | E@5)\n"),
    ("comment", "# memo\nkey: C\nC | F |\n"),
    ("multiline", "C | Am |\nF | G |\n"),
];

/// `code` を1回パースして MIDI バイト列にする。
///
/// `pre_resolve` が真なら呼び出し側で `ast.resolve()` してから `export()` に渡す
/// (`export()` は内部でもう一度 `resolve()` を呼ぶので二重呼び出しになる)。
/// 偽なら `resolve()` を呼ばずに `export()` へ渡す (`export()` 内部の1回だけ)。
///
/// どの段階で失敗しても、どのサンプルで・どの入力で落ちたかが分かるように
/// `name` と `code` をエラーメッセージに含める。
fn midi_bytes_with(name: &str, code: &str, pre_resolve: bool) -> Result<Vec<u8>, String> {
    let ast = RechordImporter
        .import(code)
        .map_err(|e| format!("[{name}] parse に失敗した (code={code:?}): {e}"))?;
    let mut writer = BufWriter::new(Vec::new());
    let export_result = if pre_resolve {
        let resolved = ast
            .resolve()
            .map_err(|e| format!("[{name}] resolve に失敗した (code={code:?}): {e}"))?;
        MidiExporter { bpm: 120 }.export(&mut writer, resolved)
    } else {
        MidiExporter { bpm: 120 }.export(&mut writer, ast)
    };
    export_result.map_err(|e| format!("[{name}] export に失敗した (code={code:?}): {e}"))?;
    writer
        .into_inner()
        .map_err(|e| format!("[{name}] flush に失敗した (code={code:?}): {e}"))
}

/// これまで通りの経路 (呼び出し側で resolve() してから export() に渡す = 二重呼び出し)。
fn midi_bytes(name: &str, code: &str) -> Result<Vec<u8>, String> {
    midi_bytes_with(name, code, true)
}

fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(format!("{}.mid", name))
}

/// `UPDATE_GOLDEN` の値を厳密に見る。`UPDATE_GOLDEN=1` のときだけ更新モード。
/// `UPDATE_GOLDEN=0` や `UPDATE_GOLDEN=""` など、値が違う・空の場合は更新モードに入らない。
fn is_update_golden() -> bool {
    std::env::var("UPDATE_GOLDEN").as_deref() == Ok("1")
}

/// 1件のサンプル/プリセットを検証する。パース・エクスポート・比較のどこで
/// 失敗しても panic せず `Err` を返す。呼び出し側がループを最後まで回し切って
/// 全件の失敗をまとめて報告できるようにするため。
fn check(name: &str, code: &str) -> Result<(), String> {
    let actual = midi_bytes(name, code)?;
    let path = golden_path(name);
    if is_update_golden() {
        // 更新モードは既存のゴールデンを黙って上書きするので、目立つ警告を出す。
        eprintln!(
            "⚠️  UPDATE_GOLDEN=1: [{name}] を更新モードで書き換える ({})",
            path.display()
        );
        std::fs::create_dir_all(path.parent().unwrap())
            .map_err(|e| format!("[{name}] golden 用ディレクトリの作成に失敗: {e}"))?;
        std::fs::write(&path, &actual)
            .map_err(|e| format!("[{name}] golden ファイルの書き込みに失敗: {e}"))?;
        return Ok(());
    }
    let expected = std::fs::read(&path)
        .map_err(|e| format!("[{name}] golden 未生成 ({e})。UPDATE_GOLDEN=1 で作る"))?;
    if actual != expected {
        return Err(format!(
            "[{name}] の MIDI が変わった (expected {} bytes, actual {} bytes)。意図した変更でなければ後方互換の破壊",
            expected.len(),
            actual.len()
        ));
    }
    Ok(())
}

/// プリセット名をファイル名に使える形にサニタイズする。
fn sanitize_preset_name(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect()
}

#[test]
fn samples_are_stable() {
    let mut failures = Vec::new();
    for (name, code) in SAMPLES {
        if let Err(e) = check(name, code) {
            failures.push(e);
        }
    }
    assert!(
        failures.is_empty(),
        "{} 件のサンプルが golden と食い違った:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn presets_are_stable() {
    let mut failures = Vec::new();
    // プリセット名には空白や記号が入り得るのでファイル名としてサニタイズするが、
    // サニタイズ後に別のプリセットと同じ名前になった場合は検出する
    // (検出しないと UPDATE_GOLDEN=1 のとき後勝ちで黙って上書きされる)。
    let mut seen_safe_names: HashSet<String> = HashSet::new();
    for preset in PRESETS.iter() {
        let safe = sanitize_preset_name(preset.name);
        let golden_name = format!("preset_{}", safe);
        if !seen_safe_names.insert(golden_name.clone()) {
            failures.push(format!(
                "[{}] のサニタイズ後のファイル名 \"{}\" が他のプリセットと衝突した。golden ファイル名が一意になるようにプリセット名を変えること",
                preset.name, golden_name
            ));
            continue;
        }
        if let Err(e) = check(&golden_name, &format!("{}\n", preset.progression)) {
            failures.push(e);
        }
    }
    assert!(
        failures.is_empty(),
        "{} 件のプリセットが golden と食い違った:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// `pat` の中の変換が、手で展開した形と同じ MIDI を出すこと。
///
/// かつてはブラッシングのスロットで変換が捨てられ (打弦は Baug/F なのに
/// ブラッシングは F)、s25 は全スロットで2コードとも鳴っていた。
#[test]
fn transforms_in_pattern_match_hand_expansion() {
    let pairs: &[(&str, &str, &str)] = &[
        // s25 は小節を2つに割る。パターンを掛けても手で 2 コード書いたのと同じ
        ("s25", "pat(\"oooo\", s25(G7))\n", "pat(\"oooo\", Dm7 G7)\n"),
        ("s25_dense", "pat(\"oxoxoxox\", s25(G7))\n", "pat(\"oxoxoxox\", Dm7 G7)\n"),
    ];
    let mut failures = Vec::new();
    for (name, shorthand, expanded) in pairs {
        match (midi_bytes(name, shorthand), midi_bytes(name, expanded)) {
            (Ok(a), Ok(b)) if a == b => {}
            (Ok(a), Ok(b)) => failures.push(format!(
                "[{}] 変換版と手で書いた版の MIDI が違う ({} bytes vs {} bytes)",
                name,
                a.len(),
                b.len()
            )),
            (Err(e), _) | (_, Err(e)) => failures.push(format!("[{}] {}", name, e)),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// ブラッシングのスロットでも変換が効くこと (捨てられて素のコードにならない)
#[test]
fn transforms_survive_brush_beats() {
    let cases: &[(&str, &str, &str)] = &[
        ("blk", "pat(\"xxxx\", blk(F))\n", "pat(\"xxxx\", F)\n"),
        ("dom", "pat(\"xxxx\", dom(Dm7))\n", "pat(\"xxxx\", Dm7)\n"),
        ("sub", "pat(\"xxxx\", sub(G7))\n", "pat(\"xxxx\", G7)\n"),
    ];
    let mut failures = Vec::new();
    for (name, transformed, plain) in cases {
        match (midi_bytes(name, transformed), midi_bytes(name, plain)) {
            (Ok(a), Ok(b)) if a != b => {}
            (Ok(_), Ok(_)) => failures.push(format!(
                "[{}] ブラッシングのスロットで変換が捨てられている (素のコードと同じ音)",
                name
            )),
            (Err(e), _) | (_, Err(e)) => failures.push(format!("[{}] {}", name, e)),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// 定義を使った書き方と、手で展開した書き方が同じ MIDI を出すこと。
///
/// Phase 2 で足した定義・参照・リピート・グループは「書き方を短くするだけで、
/// 鳴る音は変えない」というのが前提。ここが崩れると、定義を使った瞬間に
/// 曲が変わることになる。
///
/// `key_scope` のペアだけは意味が違う。定義の本体は度数のまま環境に入り、
/// 度数が実音になるのは**使う位置**のキーで決まる。だから同じ `$a` が
/// `key: C` の下では C F に、`key: D` の下では D G になる
#[test]
fn defines_match_hand_expansion() {
    let pairs: &[(&str, &str, &str)] = &[
        (
            "riff",
            "$riff = pow(C | E D)\n$riff | C |\n",
            "pow(C | E D) C |\n",
        ),
        ("repeat", "C x4 |\n", "C | C | C | C |\n"),
        ("group", "(C | Am) x2\n", "C | Am | C | Am |\n"),
        (
            "nested",
            "$a = C\n$b = $a | $a\n$b x2\n",
            "C | C | C | C |\n",
        ),
        // pow / oct の修飾が参照の中身まで届くこと
        ("pow_through_ref", "$a = C\npow($a)\n", "pow(C)\n"),
        ("oct_through_ref", "$a = A\noct($a)\n", "oct(A)\n"),
        // pat の第1引数が参照でも、同じリズムになること
        (
            "pat_through_ref",
            "$p = \"oxox\"\npat($p, C)\n",
            "pat(\"oxox\", C)\n",
        ),
        // 度数表記のリフを、キーを変えて使い回せること。
        // 定義した位置 (key: C) ではなく、使う位置の key: が効く
        (
            "key_scope",
            "key: C\n$a = I | IV |\n$a\nkey: D\n$a\n",
            "key: C\nI | IV |\nkey: D\nI | IV |\n",
        ),
    ];
    let mut failures = Vec::new();
    for (name, with_define, expanded) in pairs {
        match (midi_bytes(name, with_define), midi_bytes(name, expanded)) {
            (Ok(a), Ok(b)) if a == b => {}
            (Ok(a), Ok(b)) => failures.push(format!(
                "[{}] 定義版と展開版の MIDI が違う ({} bytes vs {} bytes)",
                name,
                a.len(),
                b.len()
            )),
            (Err(e), _) | (_, Err(e)) => failures.push(format!("[{}] {}", name, e)),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// サンプル + プリセットの全ケースを `(診断用の名前, コード)` で列挙する。
fn all_cases() -> Vec<(String, String)> {
    let mut cases: Vec<(String, String)> = SAMPLES
        .iter()
        .map(|(name, code)| (name.to_string(), code.to_string()))
        .collect();
    for preset in PRESETS.iter() {
        cases.push((
            format!("preset:{}", preset.name),
            format!("{}\n", preset.progression),
        ));
    }
    cases
}

/// `resolve()` の冪等性そのものを検査する。
///
/// 現状は
/// - `chord_midi_cli` の MIDI 出力: `resolve()` を明示的に呼ばず `export()` 内部の1回だけ (単一呼び出し)
/// - このゴールデンテストや `chord_midi_web`: 呼び出し側で `resolve()` してから `export()` に渡す
///   → `export()` 内部でもう一度呼ばれるので二重呼び出し
///
/// という2つの経路が混在している。今は `Key::into_pitch` が `Key::Absolute` に対して
/// no-op なので冪等が成立しているが、Task 3〜12 はまさに `resolve()` の意味論を変える
/// 作業なので、この前提が壊れうる。壊れると「二重呼び出し」経路 (golden) は通るのに
/// 「単一呼び出し」経路 (CLI) だけ壊れる、golden では検知できない事故が起きる。
///
/// このテストは単一呼び出しと二重呼び出しの結果が完全一致することを保証し、
/// 冪等性が壊れた瞬間に golden テストの範囲内で気づけるようにする。
#[test]
fn resolve_is_idempotent() {
    let mut failures = Vec::new();
    for (name, code) in all_cases() {
        let single = midi_bytes_with(&name, &code, false);
        let double = midi_bytes_with(&name, &code, true);
        match (single, double) {
            (Ok(s), Ok(d)) => {
                if s != d {
                    failures.push(format!(
                        "[{name}] resolve() が冪等でない: 単一呼び出し ({} bytes) と二重呼び出し ({} bytes) の MIDI が一致しない",
                        s.len(),
                        d.len()
                    ));
                }
            }
            (Err(e), Ok(_)) => failures.push(format!("{e} (単一呼び出し側)")),
            (Ok(_), Err(e)) => failures.push(format!("{e} (二重呼び出し側)")),
            (Err(e1), Err(e2)) => {
                failures.push(format!("{e1} (単一呼び出し側)"));
                failures.push(format!("{e2} (二重呼び出し側)"));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} 件で resolve() の冪等性が崩れた:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
