use anyhow::Result;
use midi::write_to_midi;
use score::Score;
use std::{
    env,
    fs::{File, OpenOptions},
    io::Read,
};

mod chord;
mod midi;
mod parser;
mod score;

fn main() -> Result<()> {
    let args = env::args().collect::<Vec<_>>();
    let mut f = File::open(&args[1])?;
    let mut code = String::new();
    f.read_to_string(&mut code)?;
    let score = Score::parse(code.as_str())?;
    let mut f = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open("out.midi")
        .unwrap();

    write_to_midi(&mut f, &score)?;
    Ok(())
}
