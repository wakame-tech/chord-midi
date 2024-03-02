use anyhow::Result;
use clap::Parser;
use de::ast::AST;
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
    #[arg(short, long)]
    input: PathBuf,
    #[arg(short, long)]
    output: Option<PathBuf>,
    #[arg(long, default_value_t = 180)]
    bpm: u8,
    #[arg(long)]
    key: Option<Pitch>,
    #[arg(long)]
    as_degree: bool,
}

fn main() -> Result<()> {
    simplelog::SimpleLogger::init(log::LevelFilter::Debug, Default::default())?;

    let args = Cli::try_parse()?;
    let mut f = File::open(&args.input)?;
    let mut code = String::new();
    f.read_to_string(&mut code)?;

    let mut ast = AST::parse(code.as_str())?;
    if args.as_degree {
        if let Some(key) = args.key {
            ast.as_degree(key);
        }
        if let Some(output) = &args.output {
            let mut f = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(output)
                .unwrap();
            score::dump(&mut f, &ast)?;
        }
    } else {
        let notes = into_notes(&ast, args.key)?;
        if let Some(output) = &args.output {
            let mut f = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(output)
                .unwrap();
            midi::dump(&mut f, &notes, args.bpm)?;
        }
    }
    Ok(())
}
