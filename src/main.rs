use anyhow::Result;
use clap::Parser;
use midi::write_to_midi;
use score::Score;
use std::{
    fs::{File, OpenOptions},
    io::Read,
    path::PathBuf,
};

mod chord;
mod midi;
mod parser;
mod score;

#[derive(Debug, clap::Parser)]
struct Cli {
    #[arg(short, long)]
    input: PathBuf,
    #[arg(short, long)]
    output: PathBuf,
}

fn main() -> Result<()> {
    let args = Cli::try_parse()?;
    let mut f = File::open(&args.input)?;
    let mut code = String::new();
    f.read_to_string(&mut code)?;
    let score = Score::parse(code.as_str())?;
    let mut f = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&args.output)
        .unwrap();

    write_to_midi(&mut f, &score)?;
    Ok(())
}
