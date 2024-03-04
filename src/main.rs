use anyhow::Result;
use clap::Parser;
use de::ast::parse;
use model::{degree::Pitch, score::into_notes};
use ser::{
    midi::{self},
    score,
};
use std::{
    fs::{File, OpenOptions},
    io::Read,
    path::PathBuf,
};

mod de;
mod model;
mod ser;

#[derive(Debug, clap::Parser)]
struct Cli {
    input: PathBuf,
    #[arg(short, long)]
    output: Option<PathBuf>,
    #[arg(long, default_value_t = 180)]
    bpm: u8,
    #[arg(long)]
    as_degree: Option<Pitch>,
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

    if let Some(key) = args.as_degree {
        ast.as_degree(key);
        if let Some(f) = out.as_mut() {
            score::dump(f, &ast)?;
        } else {
            score::dump(&mut std::io::stdout(), &ast)?;
        }
    } else if let Some(f) = out.as_mut() {
        let notes = into_notes(ast, Some(Pitch::C))?;
        midi::dump(f, &notes, args.bpm)?;
    }
    Ok(())
}
