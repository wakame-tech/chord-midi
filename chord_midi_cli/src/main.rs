use anyhow::Result;
use chord_midi::export::{Exporter, RechordExporter};
use chord_midi::import::{Importer, SexpImporter};
use chord_midi::{export::MidiExporter, import::RechordImporter};
use clap::Parser as _;
use std::{
    fs::{File, OpenOptions},
    io::Read,
    path::PathBuf,
};

#[derive(Debug, clap::Parser)]
struct Cli {
    #[arg(short, long)]
    input: PathBuf,
    #[arg(short, long)]
    output: PathBuf,
    #[arg(long, default_value_t = 180)]
    bpm: u8,
}

fn extension(path: &PathBuf) -> String {
    path.extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}

fn main() -> Result<()> {
    simplelog::SimpleLogger::init(log::LevelFilter::Debug, Default::default())?;
    let args = Cli::try_parse()?;

    let mut f = File::open(&args.input)?;
    let mut code = String::new();
    f.read_to_string(&mut code)?;
    // CR+LF to LF
    code = code.replace("\r\n", "\n");

    let importer = match extension(&args.input).as_str() {
        "sexp" => Box::new(SexpImporter) as Box<dyn Importer>,
        _ => Box::new(RechordImporter) as Box<dyn Importer>,
    };
    let ast = importer.import(code.as_str())?;

    let mut out = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&args.output)
        .unwrap();

    match extension(&args.output).as_str() {
        "midi" => {
            MidiExporter { bpm: args.bpm }.export(&mut out, ast)?;
            println!("Exported to {}", args.output.display());
        }
        _ => {
            RechordExporter.export(&mut out, ast)?;
            println!("Exported to {}", args.output.display());
        }
    };
    Ok(())
}
