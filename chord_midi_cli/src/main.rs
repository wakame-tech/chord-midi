use anyhow::Result;
use chord_midi::import::Importer;
use chord_midi::model::pitch::Pitch;
use chord_midi::{import::RechordImporter, midi::dump};
use clap::Parser as _;
use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Write},
    path::PathBuf,
};

#[derive(Debug, clap::Parser)]
struct Cli {
    #[arg()]
    input: PathBuf,
    #[arg(short, long, global = true)]
    output: Option<PathBuf>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, clap::Parser)]
enum Commands {
    Convert(Convert),
    Midi(Midi),
}

#[derive(Debug, clap::Parser)]
struct Convert {
    #[arg(long)]
    as_pitch: Option<Pitch>,
    #[arg(long)]
    as_degree: Option<Pitch>,
}

#[derive(Debug, clap::Parser)]
struct Midi {
    #[arg(long, default_value_t = 180)]
    bpm: u8,
}

fn main() -> Result<()> {
    simplelog::SimpleLogger::init(log::LevelFilter::Debug, Default::default())?;
    let args = Cli::try_parse()?;

    let mut f = File::open(&args.input)?;
    let mut code = String::new();
    f.read_to_string(&mut code)?;
    // CR+LF to LF
    code = code.replace("\r\n", "\n");
    let ast = RechordImporter.import(code.as_str())?;

    let mut out = args.output.map(|p| {
        OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(p)
            .unwrap()
    });

    match args.command {
        Commands::Convert(args) => {
            let mut out = out
                .map(|f| Box::new(f) as Box<dyn Write>)
                .unwrap_or(Box::new(io::stdout()) as Box<dyn Write>);

            let ast = if let Some(p) = args.as_pitch {
                ast.into_pitch(p)
            } else if let Some(p) = args.as_degree {
                ast.into_degree(p)
            } else {
                ast
            };

            writeln!(out, "{}", ast)?;
        }
        Commands::Midi(args) => {
            if let Some(f) = out.as_mut() {
                dump(f, ast, args.bpm)?;
            }
        }
    }
    Ok(())
}
