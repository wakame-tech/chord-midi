use anyhow::{anyhow, Result};
use chord_midi::edit::{set_scale, shift_key};
use chord_midi::export::{Exporter, RechordExporter};
use chord_midi::fingering;
use chord_midi::import::{Importer, SexpImporter};
use chord_midi::model::ast::Expr;
use chord_midi::model::scale::Scale;
use chord_midi::presets::PRESETS;
use chord_midi::{export::MidiExporter, import::RechordImporter};
use clap::Parser as _;
use std::str::FromStr;
use std::{
    fs::{File, OpenOptions},
    io::Read,
    path::{Path, PathBuf},
};

mod diagram;

#[derive(Debug, Clone, Copy, PartialEq, clap::ValueEnum)]
enum AstForm {
    /// パース直後の構文木を JSON で
    Syntax,
    /// key/scale を解決し pow/oct を潰した後の木を JSON で
    Expanded,
    /// rechord 記法に書き戻したテキスト
    Text,
}

#[derive(Debug, clap::Parser)]
#[command(version, about = "rechord 記法のコード進行を MIDI などに変換する")]
struct Cli {
    /// 入力ファイル。`--preset` / `--list-presets` のときは省略できる
    #[arg(short, long)]
    input: Option<PathBuf>,
    /// 出力ファイル。拡張子が midi なら MIDI、それ以外は rechord 記法
    #[arg(short, long)]
    output: Option<PathBuf>,
    #[arg(long, default_value_t = 180)]
    bpm: u8,
    /// 入力ファイルの代わりに使う進行プリセット名
    #[arg(long, conflicts_with = "input")]
    preset: Option<String>,
    /// 使えるプリセットを一覧して終わる
    #[arg(long)]
    list_presets: bool,
    /// キーを半音単位で移調する。負の値も取る
    #[arg(long, allow_hyphen_values = true, value_name = "SEMITONES")]
    transpose: Option<i8>,
    /// スケールを張り替える (major, minor, dorian, ...)
    #[arg(long, value_name = "SCALE")]
    scale: Option<String>,
    /// AST を表示する。syntax = 構文木 (既定), expanded = 解決後, text = rechord 記法
    #[arg(
        long,
        value_name = "FORM",
        num_args = 0..=1,
        default_missing_value = "syntax"
    )]
    print_ast: Option<AstForm>,
    /// ギターの押さえ方を標準出力に書く
    #[arg(long)]
    fingering: bool,
    /// 変換の途中経過をログに出す
    #[arg(short, long)]
    verbose: bool,
}

fn extension(path: &Path) -> String {
    path.extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}

fn list_presets() {
    for preset in PRESETS {
        println!("{}\t{}", preset.name, preset.degrees);
    }
}

fn preset_progression(name: &str) -> Result<String> {
    PRESETS
        .iter()
        .find(|p| p.name == name)
        .map(|p| format!("{}\n", p.progression))
        .ok_or_else(|| anyhow!("unknown preset: {} (`--list-presets` で一覧が出る)", name))
}

fn read_input(path: &Path) -> Result<String> {
    let mut f = File::open(path)?;
    let mut code = String::new();
    f.read_to_string(&mut code)?;
    // CR+LF to LF
    Ok(code.replace("\r\n", "\n"))
}

/// `--scale` → `--transpose` の順に入力テキストを書き換える。
/// どちらも AST ではなくテキストを直す。web 版の編集ボタンと同じ扱い
fn edit_source(code: String, args: &Cli) -> Result<String> {
    let code = match &args.scale {
        Some(name) => set_scale(code.as_str(), Scale::from_str(name)?),
        None => code,
    };
    let code = match args.transpose {
        Some(delta) => shift_key(code.as_str(), delta),
        None => code,
    };
    Ok(code)
}

/// 押さえ方を小節ごとに書く。`|` で小節を区切り、入力の改行もそのまま出す
fn print_fingerings(ast: Expr) -> Result<()> {
    let resolved = ast.resolve()?;
    for measure in fingering::measure_refs(&resolved)? {
        for chord in &measure.chords {
            print!("{}", diagram::render(&chord.label, fingering::find(chord)));
            println!();
        }
    }
    Ok(())
}

fn export(args: &Cli, path: &Path, ast: Expr) -> Result<()> {
    let mut out = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)?;
    match extension(path).as_str() {
        "midi" => MidiExporter { bpm: args.bpm }.export(&mut out, ast)?,
        _ => RechordExporter.export(&mut out, ast)?,
    };
    println!("Exported to {}", path.display());
    Ok(())
}

fn main() -> Result<()> {
    let args = Cli::parse();
    // 変換結果を標準出力に流すので、ログは既定では黙らせる
    let level = if args.verbose {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Warn
    };
    simplelog::SimpleLogger::init(level, Default::default())?;

    if args.list_presets {
        list_presets();
        return Ok(());
    }

    let (code, sexp) = match (&args.preset, &args.input) {
        (Some(name), _) => (preset_progression(name)?, false),
        (None, Some(path)) => (read_input(path)?, extension(path) == "sexp"),
        (None, None) => {
            return Err(anyhow!("--input か --preset のどちらかが要る"));
        }
    };
    let code = edit_source(code, &args)?;

    // Expr は Clone できず resolve() / export() が consume するので、
    // 必要になるたびに読み直す。パースは十分安い
    let parse = || -> Result<Expr> {
        let importer = if sexp {
            Box::new(SexpImporter) as Box<dyn Importer>
        } else {
            Box::new(RechordImporter) as Box<dyn Importer>
        };
        importer.import(code.as_str())
    };

    if let Some(form) = args.print_ast {
        let ast = parse()?;
        match form {
            AstForm::Syntax => println!("{}", serde_json::to_string_pretty(&ast)?),
            AstForm::Expanded => {
                println!("{}", serde_json::to_string_pretty(&ast.resolve()?)?)
            }
            AstForm::Text => print!("{}", ast),
        }
    }
    if args.fingering {
        print_fingerings(parse()?)?;
    }
    match &args.output {
        Some(path) => export(&args, path, parse()?)?,
        // 出力先も表示指定も無いときは、書き換えた入力をそのまま返す
        None if args.print_ast.is_none() && !args.fingering => {
            print!("{}", code);
            if !code.ends_with('\n') {
                println!();
            }
        }
        None => {}
    }
    Ok(())
}
