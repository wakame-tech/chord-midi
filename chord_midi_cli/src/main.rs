use anyhow::Result;
use chord_midi::{midi::dump, parser::parse, syntax::Pitch};
use clap::Parser;
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
    Degree(Degree),
    Midi(Midi),
}

#[derive(Debug, clap::Parser)]
struct Degree {
    #[arg(long)]
    key: Pitch,
}

#[derive(Debug, clap::Parser)]
struct Midi {
    #[arg(long)]
    key: Option<Pitch>,
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
    let mut ast = parse(code.as_str())?;

    let mut out = args.output.map(|p| {
        OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(p)
            .unwrap()
    });

    match args.command {
        Commands::Degree(args) => {
            let mut out = out
                .map(|f| Box::new(f) as Box<dyn Write>)
                .unwrap_or(Box::new(io::stdout()) as Box<dyn Write>);
            ast.as_degree(args.key);
            writeln!(out, "{}", ast)?;
        }
        Commands::Midi(args) => {
            if let Some(f) = out.as_mut() {
                dump(f, ast, args.key, args.bpm)?;
            }
        }
    }
    Ok(())
}
