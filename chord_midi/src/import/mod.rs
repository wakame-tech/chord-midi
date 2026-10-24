use crate::model::ast::Expr;

mod chord;
mod parser_util;
mod rechord;
mod sexp;

pub trait Importer {
    fn import(&self, code: &str) -> anyhow::Result<Expr>;
}

#[derive(Debug)]
pub struct RechordImporter;

#[derive(Debug)]
pub struct SexpImporter;

/// 読めなかった位置。`anyhow::Error` に包んで返すので、位置が欲しい側
/// (web のガター) は `downcast_ref::<ParseError>()` で取り出す
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    /// 1 始まり
    pub line: usize,
    /// 1 始まり。バイトではなく文字で数える (日本語の定義名があってもズレない)
    pub column: usize,
    /// エラーの行そのもの (改行なし)
    pub line_text: String,
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}行目 {}列目: {}\n{}\n{}^",
            self.line,
            self.column,
            self.message,
            self.line_text,
            " ".repeat(self.column - 1)
        )
    }
}

impl std::error::Error for ParseError {}
