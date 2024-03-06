use chord_midi::{model::degree::Pitch, parse, score_dump};
use std::io::BufWriter;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn as_degree(code: &str) -> String {
    let mut out = BufWriter::new(Vec::new());
    let mut ast = parse(code).unwrap();
    ast.as_degree(Pitch::C);
    score_dump(&mut out, &ast).unwrap();
    String::from_utf8(out.into_inner().unwrap()).unwrap()
}
