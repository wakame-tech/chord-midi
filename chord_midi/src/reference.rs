//! 記法の一覧表。web の記法パネルと CLI の `--help-syntax` が同じものを出す。
//! `presets.rs` と同じ静的テーブル。生成元の JSON は持たないので手で足す。

/// 関数の適用単位。表の「単位」列と、web のパレットの2群が使う
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// `pow(C | E D)` のように小節列に掛かる
    Measures,
    /// `arp(C)` のように1つのコードに掛かる
    Chord,
}

impl Scope {
    /// 表に出す日本語の見出し
    pub fn label(&self) -> &'static str {
        match self {
            Scope::Measures => "小節列",
            Scope::Chord => "コード",
        }
    }
}

/// 記号1件。名前を持たない書き方
pub struct Symbol {
    pub symbol: &'static str,
    pub summary: &'static str,
    pub example: &'static str,
}

/// 関数1件
pub struct Function {
    pub name: &'static str,
    /// 引数の並びを日本語で。`pow(小節列)` の括弧の中に入る文字列
    pub args: &'static str,
    pub scope: Scope,
    pub summary: &'static str,
    pub example: &'static str,
}

pub const SYMBOLS: &[Symbol] = &[
    Symbol {
        symbol: "|",
        summary: "小節の区切り",
        example: "C | Am |",
    },
    Symbol {
        symbol: "=",
        summary: "前の音を伸ばす",
        example: "C | = |",
    },
    Symbol {
        symbol: "%",
        summary: "前の小節を繰り返す",
        example: "C | % |",
    },
    Symbol {
        symbol: "_",
        summary: "休符",
        example: "C | _ |",
    },
    Symbol {
        symbol: "N.C.",
        summary: "コードなし (休符と同じ)",
        example: "C | N.C. |",
    },
    Symbol {
        symbol: "/",
        summary: "分数コード。右がベース",
        example: "C/E |",
    },
    Symbol {
        symbol: "@N",
        summary: "最低音を取る弦 (1〜6)",
        example: "pow(A@6)",
    },
    Symbol {
        symbol: "$",
        summary: "定義と参照",
        example: "$A = C | Am |",
    },
    Symbol {
        symbol: "xN",
        summary: "直前の1項目を N 回 (2〜32)",
        example: "C x4 |",
    },
    Symbol {
        symbol: "( )",
        summary: "繰り返す範囲をまとめる",
        example: "(C | Am) x2",
    },
    Symbol {
        symbol: "key:",
        summary: "以降のキー",
        example: "key: C",
    },
    Symbol {
        symbol: "scale:",
        summary: "以降のスケール",
        example: "scale: dorian",
    },
    Symbol {
        symbol: "#",
        summary: "行コメント",
        example: "# memo",
    },
];

/// `pat` の第1引数に書けるリズムの文字
pub const BEATS: &[Symbol] = &[
    Symbol {
        symbol: "o",
        summary: "打弦",
        example: "pat(\"oooo\", C)",
    },
    Symbol {
        symbol: ".",
        summary: "休符",
        example: "pat(\"o.o.\", C)",
    },
    Symbol {
        symbol: "x",
        summary: "ブラッシング",
        example: "pat(\"oxox\", C)",
    },
];

pub const FUNCTIONS: &[Function] = &[
    Function {
        name: "pow",
        args: "小節列",
        scope: Scope::Measures,
        summary: "パワーコード化 (ルートと5th)",
        example: "pow(C | E D)",
    },
    Function {
        name: "oct",
        args: "小節列",
        scope: Scope::Measures,
        summary: "オクターブ奏法",
        example: "oct(A | G#)",
    },
    Function {
        name: "pat",
        args: "リズム, 小節列",
        scope: Scope::Measures,
        summary: "リズムパターンを掛ける",
        example: "pat(\"o.o.oxox\", C | E D)",
    },
    Function {
        name: "arp",
        args: "コード",
        scope: Scope::Chord,
        summary: "アルペジオ",
        example: "arp(C)",
    },
    Function {
        name: "blk",
        args: "コード",
        scope: Scope::Chord,
        summary: "ブラックアダーコード",
        example: "blk(F)",
    },
    Function {
        name: "s25",
        args: "コード",
        scope: Scope::Chord,
        summary: "ツーファイブ分割",
        example: "s25(G7)",
    },
    Function {
        name: "sub",
        args: "コード",
        scope: Scope::Chord,
        summary: "裏コード化",
        example: "sub(G7)",
    },
    Function {
        name: "dom",
        args: "コード",
        scope: Scope::Chord,
        summary: "ドミナント化",
        example: "dom(Dm7)",
    },
];

/// CLI の `--help-syntax` が出す等幅テキスト。
/// web の表と同じテーブルから作るので、二重管理にならない
pub fn help_text() -> String {
    let mut out = String::new();
    out.push_str("記号\n");
    out.push_str(&symbol_table(SYMBOLS));
    out.push_str("\nリズムの文字 (pat の第1引数)\n");
    out.push_str(&symbol_table(BEATS));
    out.push_str("\n関数\n");
    let name_w = FUNCTIONS
        .iter()
        .map(|f| width(f.name) + width(f.args) + 2)
        .max()
        .unwrap_or(0);
    let scope_w = FUNCTIONS
        .iter()
        .map(|f| width(f.scope.label()))
        .max()
        .unwrap_or(0);
    let sum_w = FUNCTIONS
        .iter()
        .map(|f| width(f.summary))
        .max()
        .unwrap_or(0);
    for f in FUNCTIONS {
        let sig = format!("{}({})", f.name, f.args);
        out.push_str(&format!(
            "  {}  {}  {}  {}\n",
            pad(&sig, name_w),
            pad(f.scope.label(), scope_w),
            pad(f.summary, sum_w),
            f.example
        ));
    }
    out
}

fn symbol_table(rows: &[Symbol]) -> String {
    let sym_w = rows.iter().map(|s| width(s.symbol)).max().unwrap_or(0);
    let sum_w = rows.iter().map(|s| width(s.summary)).max().unwrap_or(0);
    rows.iter()
        .map(|s| {
            format!(
                "  {}  {}  {}\n",
                pad(s.symbol, sym_w),
                pad(s.summary, sum_w),
                s.example
            )
        })
        .collect()
}

/// 端末での見た目の桁数。日本語は2桁として数える
fn width(s: &str) -> usize {
    s.chars().map(|c| if is_wide(c) { 2 } else { 1 }).sum()
}

/// 全角として扱う範囲。ここに出るのは日本語の説明文だけなので、
/// CJK と長音記号・全角括弧を見れば足りる (unicode-width は入れない)
fn is_wide(c: char) -> bool {
    matches!(
        c as u32,
        0x1100..=0x115F
            | 0x2E80..=0xA4CF
            | 0xAC00..=0xD7A3
            | 0xF900..=0xFAFF
            | 0xFE30..=0xFE4F
            | 0xFF00..=0xFF60
            | 0xFFE0..=0xFFE6
    )
}

fn pad(s: &str, w: usize) -> String {
    let mut out = s.to_string();
    for _ in width(s)..w {
        out.push(' ');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::{Importer, RechordImporter};

    /// 表に載せた例が本当にパースできること。
    /// 記法が変わったときに表だけ取り残されるのを機械で止める
    #[test]
    fn every_example_parses() {
        let examples = SYMBOLS
            .iter()
            .chain(BEATS.iter())
            .map(|s| s.example)
            .chain(FUNCTIONS.iter().map(|f| f.example));
        for example in examples {
            let code = format!("{}\n", example);
            assert!(
                RechordImporter.import(&code).is_ok(),
                "表の例がパースできない: {}",
                example
            );
        }
    }

    #[test]
    fn help_text_lists_every_row() {
        let text = help_text();
        for s in SYMBOLS.iter().chain(BEATS.iter()) {
            assert!(text.contains(s.symbol), "{} が出ていない", s.symbol);
            assert!(text.contains(s.summary), "{} が出ていない", s.summary);
        }
        for f in FUNCTIONS {
            assert!(text.contains(f.name), "{} が出ていない", f.name);
            assert!(text.contains(f.summary), "{} が出ていない", f.summary);
        }
    }

    #[test]
    fn functions_are_grouped_by_scope() {
        // パレットの2群がどちらも空にならないこと
        assert!(FUNCTIONS.iter().any(|f| f.scope == Scope::Measures));
        assert!(FUNCTIONS.iter().any(|f| f.scope == Scope::Chord));
    }

    #[test]
    fn width_counts_japanese_as_two_columns() {
        assert_eq!(width("pow"), 3);
        assert_eq!(width("小節列"), 6);
        assert_eq!(pad("o", 3), "o  ");
    }

    #[test]
    fn help_text_columns_are_aligned() {
        // 「関数」の行は列が揃っていること。等幅で読ませる表なので、
        // ここが崩れると CLI の出力が読めない
        let text = help_text();
        let lines: Vec<&str> = text
            .lines()
            .skip_while(|l| *l != "関数")
            .skip(1)
            .filter(|l| !l.trim().is_empty())
            .collect();
        assert_eq!(lines.len(), FUNCTIONS.len());
        // 「例」が始まる桁がどの行でも同じであること
        let example_column = |line: &str| {
            let example = FUNCTIONS
                .iter()
                .find(|f| line.ends_with(f.example))
                .expect("行は例で終わる")
                .example;
            width(line) - width(example)
        };
        let first = example_column(lines[0]);
        for line in &lines {
            assert_eq!(example_column(line), first, "列が揃っていない: {}", line);
        }
    }

    /// README に載っている関数名が表から漏れていないこと。
    /// 「README と二重管理しない」を機械で見張る
    #[test]
    fn readme_mentions_every_function() {
        let readme = include_str!("../../README.md");
        for f in FUNCTIONS {
            assert!(readme.contains(f.name), "README に {} が無い", f.name);
        }
    }
}
