//! textarea のカーソル位置を起点にした編集。設計書 3.3 の
//! `insert_at_cursor` はこれ1つで、記法パネルの全部品がここを通る。
//!
//! 文字列の計算は `chord_midi::textedit` にある (workspace のテストに
//! 載せるため)。ここは web-sys から選択範囲を読んで渡すだけの層にする。

use chord_midi::textedit;
use web_sys::HtmlTextAreaElement;

/// GUI 部品が投げる編集要求。部品はテキストを持たないので、
/// 「何をしたいか」だけを App に渡す
#[derive(Clone, PartialEq, Debug)]
pub enum Edit {
    /// カーソル位置に入れる。選択範囲があれば置き換える
    Insert(String),
    /// 選択範囲を前後で包む。選択が無ければ挿入してカーソルを内側に置く
    Wrap(String, String),
    /// リズムパターン。カーソルが `pat("...")` の引用符の中なら
    /// その文字列だけを差し替え、そうでなければ `pat("<p>", )` を入れる
    Rhythm(String),
}

/// 編集を適用した新しいテキストと、その中でのカーソル位置 (UTF-16) を返す。
///
/// DOM は書き換えない。テキストの持ち主は Yew の state なので、
/// 返した値を呼び出し側が `set` する
pub fn insert_at_cursor(ta: &HtmlTextAreaElement, text: &str, edit: &Edit) -> (String, u32) {
    // selectionStart / selectionEnd は UTF-16 コード単位。日本語の
    // 定義名があるとバイト位置とズレるので、必ずここで変換する
    let start = ta.selection_start().ok().flatten().unwrap_or(0);
    let end = ta.selection_end().ok().flatten().unwrap_or(start);
    let start = textedit::utf16_to_byte(text, start);
    let end = textedit::utf16_to_byte(text, end);
    let edited = match edit {
        Edit::Insert(s) => textedit::replace_range(text, start, end, s),
        Edit::Wrap(open, close) => textedit::wrap_range(text, start, end, open, close),
        Edit::Rhythm(p) => match textedit::pat_pattern_span(text, start) {
            Some((a, b)) => textedit::replace_range(text, a, b, p),
            None => textedit::wrap_range(text, start, end, &format!("pat(\"{}\", ", p), ")"),
        },
    };
    let caret = textedit::byte_to_utf16(&edited.text, edited.cursor);
    (edited.text, caret)
}
