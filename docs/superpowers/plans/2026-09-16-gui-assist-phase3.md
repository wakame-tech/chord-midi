# Phase 3: GUI 補助 (記法パネル) 実装計画

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 記法を覚えていなくても書けるようにする。エディタタブに記法パネル (リズム入力・関数ボタン・記法表・定義チップ) を足し、CLI にも同じ記法表を `--help-syntax` で出す。記法の機能追加はゼロ。

**Architecture:** GUI は独自モデルを持たない。部品は「テキストをどう変えたいか」を表す `Edit` を `Callback` で投げるだけで、テキストの持ち主は今までどおり `App` の `use_state(String)`。判断のある文字列処理 (カーソル位置への挿入・選択範囲の包み込み・`pat("...")` の検出・定義名の抽出・UTF-16 とバイト位置の変換) は `chord_midi/src/textedit.rs` に純粋関数として置き、workspace のテストに載せる。`chord_midi_web/src/cursor.rs` は web-sys から選択範囲を読んでそれを呼ぶだけの薄い層にする。記法の一覧は `chord_midi/src/reference.rs` の静的テーブル1本で、web は HTML の表、CLI は等幅テキストとして出す。

**Tech Stack:** Rust 2021, yew 0.21 (web), web-sys 0.3 (textarea の選択範囲), clap 4 (CLI), regex + once_cell (定義名の抽出)

**設計書:** `docs/superpowers/specs/2026-09-15-notation-ux-design.md` の「3. GUI 補助」
**前提:** Phase 1 (`2026-09-15-expr-ast-phase1.md`) と Phase 2 (`2026-09-15-defines-refs-phase2.md`) がマージ済み。次があること。

- `Expr::Define` / `Ref` / `Repeat` / `Pattern` / `Section`、`Func::Pat(Box<Expr>)`、`Expr: Clone`
- `MeasureRef.section: Option<String>` (Task 10 が使う)
- `Display for Pattern` は引用符なしの `oxox` を出し、引用符を付けるのは `Expr::Pattern` の側 (Task 6 の `RhythmGrid` がこれに乗る)

**Phase 3 は `Expr` にも `Func` にも触らない。** `Func::Pat` が `Pattern` から `Box<Expr>` に変わったことは Phase 3 の部品には影響しない。`RhythmGrid` は AST を見ずに文字列を走査するし (決定4)、`Pattern` 構造体そのものは Phase 2 でも変わっていないため。

---

## Phase 1 / Phase 2 から引き継ぐ約束

1. **ゴールデン MIDI (`chord_midi/tests/golden/*.mid`) を1バイトも変えない。** Phase 3 は記法にも変換にも触らないので、ゴールデンに差分が出たらそれ自体がバグ
2. **既存テストのアサーションの意味を変えない。** 期待値を変えたくなったら手を止めて報告する
3. **個別タスクで `cargo fmt` を走らせない。** 整形は最後に1本のコミットで当てる。CI に `fmt --check` が入っているので、Task 11 までに必ず当てること
4. **macOS の BSD sed は `\b` を解釈せず、エラーなしで0件置換になる。** 一括置換は `perl -pi -e`
5. `chord_midi_web` は workspace から exclude。チェックは `(cd chord_midi_web && cargo check --target wasm32-unknown-unknown)`、ビルドは `(cd chord_midi_web && trunk build)`

---

## 設計上の決定 (設計書が決めていなかったもの)

### 決定1: 文字列の計算は `chord_midi` に置き、web は DOM だけを見る

`chord_midi_web` は workspace から exclude されていて `cargo test` が回らない。CI も `cargo check --target wasm32-unknown-unknown` しか走らせない。**web に判断を置くとテストできない**ので、カーソル操作の中身 (`pat("...")` の探索、包み込み、定義名の抽出) は `chord_midi/src/textedit.rs` に純粋関数として置く。DOM に触るのは `chord_midi_web/src/cursor.rs` の `insert_at_cursor` だけで、そこは分岐を3つ持つだけの層になる。

### 決定2: カーソル位置は「state に積んで effect で戻す」

textarea は `value={(*input).clone()}` の制御コンポーネントなので、DOM を直接書き換えると state とズレる。逆に `input.set()` だけすると、Yew が `value` を丸ごと書き戻したときにカーソルが末尾へ飛ぶ。

手順を固定する。

1. ボタンの `onclick` が `editor_ref.cast::<HtmlTextAreaElement>()` で textarea を取り、`selection_start()` / `selection_end()` を読む
2. 新しいテキストと新しいカーソル位置 (UTF-16) を計算する
3. `input.set(新テキスト)` と `caret.set(Some(位置))` を**同じコールバックの中で**呼ぶ
4. `use_effect_with(*caret, ...)` が描画の後に走り、`ta.focus()` → `ta.set_selection_range(pos, pos)` → `caret.set(None)` する

3 で両方の state を先に書いてから描画が起きるので、effect が走る時点では textarea の `value` は必ず新しい方になっている。`caret` を `None` に戻すのは、次に同じ位置へ挿入したときも dep が変わって effect が走るようにするため。

`NodeRef` は必要。`fretboard.rs` はテキスト末尾に足すだけで DOM を見ないので `NodeRef` を持っていないが、カーソル位置は DOM にしかないので App が `editor_ref` を持つ。

### 決定3: `selection_start` は UTF-16、Rust のスライスはバイト

`$Aメロ` のような日本語の定義名があるので、ここを混ぜると必ず panic する (`byte index is not a char boundary`)。`textedit::utf16_to_byte` / `byte_to_utf16` を置き、境界の丸めも `textedit` 側でやる。web 側では生の数値を扱わない。

### 決定4: `pat("...")` の検出は AST を使わず、カーソルのある1行だけを文字列で走査する

- **AST を使わない理由**: 打鍵の途中はパースが落ちている。落ちている間だけリズムボタンの挙動が変わるのは最悪の UX
- **1行に閉じる理由**: パターン文字列に改行は入らない (`pattern_parser` は `o` `.` `x` しか受け付けない)。行に閉じておけば、引用符の閉じ忘れが行をまたいで暴走しない

境界条件の決め方:

| 状況 | 扱い |
|---|---|
| `pat("oxox", C)` の引用符の中にカーソル | その引用符の内側を置換 |
| `pat("oxox", C)` の `C` にカーソル | 置換しない。新しい `pat("...", )` を挿入する |
| `pat("ab", C)` (中身がパターン文字でない) | 触らない。別物を壊さないため |
| `pat("oxox, C)` (引用符が閉じていない) | その行はそこで打ち切り。置換しない |
| `pat("oxox", pat("oo", C))` (ネスト) | 引用符の区間は重ならないので、カーソルを含む方が当たる |
| 同じ行に `pat` が2つ | 同上 |
| `pat($eight, C)` (Phase 2 の参照をリズム欄に渡した形) | 引用符が無いので区間は無い。置換せず、新しい `pat("...", )` を挿入する。参照を黙って実体に戻さないため |
| 行頭が `#` (コメント行) | 触らない |

「引用符の中」を厳密に取る (小節の位置にカーソルがあるときは置換しない) のは、**そこにカーソルがあるユーザは「ここに新しい pat を入れたい」ことも同じくらいある**ため。厳密にしておけば、置換と挿入のどちらもクリック1回で確実に出せる。

### 決定5: `reference.rs` は記号と関数で型を分ける

1つの型に畳むと記号側に空の `args` が並び、CLI の列幅計算が「空文字を無視する」分岐を持つことになる。`Symbol { symbol, summary, example }` と `Function { name, args, scope, summary, example }` の2つに分け、テーブルも `SYMBOLS` / `BEATS` / `FUNCTIONS` の3本にする。`presets.rs` と同じく `&'static [T]` の定数 (ただし生成元の JSON は持たないので手で足す)。

`Function.scope` (`Scope::Measures` / `Scope::Chord`) は表の「単位」列と `FuncPalette` の2群の並びの両方が使う。設計書が `pow oct pat / arp blk s25 sub dom` と斜線で分けているのはこの区別。

**GUI の挿入テンプレートはテーブルに入れない。** `pow(` `)` のような文字列は web 側で `name` から組む。CLI にとって無用なフィールドを説明用テーブルに載せると、表の意味が濁る。

### 決定6: CLI は `--help-syntax` の bool フラグ。サブコマンドにしない

既存の `--list-presets` が「情報を出して終わる bool フラグ」の型を作っている。サブコマンドにすると既存の呼び出しが全部 `convert` の下に潜ることになり、README の CLI 節が丸ごと壊れる。clap の `--help` とも別にする: `--help` は引数の説明で、記法は入力ファイルの中身の話なので別物。`after_help` に詰めると `--help` のたびに表が出て邪魔になる。

### 決定7: パネルの配置は CSS Grid。ブレークポイントは既存の 640px を使い回す

`style.css` のブレークポイントは `@media (max-width: 640px)` の1本だけ。新しい境界を増やさず、そこで1段組に落とす。

- 641px 以上: `grid-template-columns: minmax(0, 1fr) minmax(240px, 280px)`。パネルは `position: sticky` でエディタを見ながら追従する
- 640px 以下: 1列。パネルはエディタの下に回り、`position: static` に戻す

パネル全体を `<details class="side-panel" open=true>` にすることで、設計書の「狭い画面では下、`<details>` で畳む」を JS なしで満たす。幅で `open` を切り替えることは CSS ではできないので、**どの幅でも開いた状態で始まり、どの幅でも畳める**とする。

### 決定8: 進行プリセットは触らない。左右に分けて役割を見せる

設計書は「プリセットを `$` 定義として貼る形に変える案はあるが、今回はやらない」と書いている。挙動は1行も変えない。紛らわしさは配置とラベルで解く。

| | 進行プリセット | DefineChips |
|---|---|---|
| 置き場所 | 左 (エディタ側) の従来の位置 | 右のパネルの一番上 |
| ラベル | 「進行プリセット (末尾に追記)」 | 「定義 (カーソル位置に挿入)」 |
| 見た目 | 2段の `.preset` ボタン (名前 + ディグリー) | 等幅1段の `.chip` (`$A`) |
| 挿入先 | テキストの末尾 (`append_line`) | カーソル位置 |

定義が0件のときもブロックは消さず、「行頭に `$名前 = ...` と書くと、ここに出る」と出す。列の高さが跳ねないのと、記法をそこで教えられるのが理由。

### 決定9: 運指タブの「挿入」は末尾追記のまま残す

`fretboard.rs` の `on_insert` は `append_line` でテキスト末尾に足す。設計書 3.3 の「全部品から呼ぶ」は記法パネルの3部品を指す。運指タブは別タブでエディタが見えておらず、**見えていないカーソルに入れる方が驚く**。README にも「エディタの入力末尾に足される」と書いてあるので、挙動を変えない。

---

## ファイル構成

| ファイル | 責務 | 変更 |
|---|---|---|
| `chord_midi/src/textedit.rs` | **新規**。カーソル位置を起点にした純粋なテキスト編集と、定義名の抽出 | 新規 |
| `chord_midi/src/reference.rs` | **新規**。記号・リズム文字・関数の静的テーブルと、CLI 用の等幅テキスト | 新規 |
| `chord_midi/src/lib.rs` | 上2つを公開する | 小 |
| `chord_midi_cli/src/main.rs` | `--help-syntax` | 小 |
| `chord_midi_cli/tests/help_syntax.rs` | **新規**。`--help-syntax` の配線テスト | 新規 |
| `chord_midi_web/src/cursor.rs` | **新規**。`Edit` と `insert_at_cursor` (DOM 層) | 新規 |
| `chord_midi_web/src/palette.rs` | **新規**。`FuncPalette` | 新規 |
| `chord_midi_web/src/rhythm.rs` | **新規**。`RhythmGrid` | 新規 |
| `chord_midi_web/src/defines.rs` | **新規**。`DefineChips` | 新規 |
| `chord_midi_web/src/syntax_ref.rs` | **新規**。`SyntaxReference` | 新規 |
| `chord_midi_web/src/main.rs` | `editor_ref` / `caret` / `on_edit`、2段組の器、4部品のマウント、セクションラベル | 中 |
| `chord_midi_web/style.css` | 2段組、パネル、4部品、セクション帯 | 中 |
| `README.md` | `--help-syntax` と記法パネルの説明 | 小 |

---

## 依存関係と並列化

```
Task 1 (textedit.rs) ─┬─────────→ Task 4 (web 配線) ─┬→ Task 6 (rhythm.rs)
                      │                              ├→ Task 7 (defines.rs)
Task 2 (reference.rs) ┼→ Task 3 (CLI)                │
                      │                              ├→ Task 5 (palette.rs)
                      └──────────────────────────────┴→ Task 8 (syntax_ref.rs)
                                                         ↓
                                          Task 9 (パネルにマウント)
                                                         ↓
                                          Task 10 (セクションラベル)
                                                         ↓
                                          Task 11 (README と整形)
```

**並列グループ A: Task 1 と Task 2。** どちらも新規ファイル1本 + `chord_midi/src/lib.rs` に `pub mod` を1行。衝突するのは lib.rs の1行だけで、解決は**両方の行を残す**だけ (アルファベット順に `pub mod reference;` → `pub mod textedit;`)。

**並列グループ B: Task 3 と Task 4。** 触るクレートが別 (`chord_midi_cli` と `chord_midi_web`)。完全に独立。

**並列グループ C: Task 5 / 6 / 7 / 8。** それぞれ `chord_midi_web/src/` に新規ファイルを1本作り、`main.rs` に `mod <名前>;` を1行足すだけ。Rust は `mod` 宣言の無いファイルをコンパイルしないので、この1行だけはどうしても要る。4つを並列で走らせると `main.rs` の `mod` 宣言の塊が4方向で衝突するが、**解決は4行すべてを残すだけ**。パネルへのマウントは Task 9 がまとめてやるので、それ以外に `main.rs` を触らせない。

**`style.css` は Task 4 が全部書く。** 4部品の CSS も Task 4 が先に書き切る (markup はこの計画が指定しているので、部品より先に CSS を書ける)。こうしないとグループ C の4タスクが `style.css` の末尾と `@media` ブロックの2か所で衝突する。

Task 9 以降は直列。

---

## Task 1: `chord_midi/src/textedit.rs` — カーソル位置の編集 (並列可)

**Files:**
- Create: `chord_midi/src/textedit.rs`
- Modify: `chord_midi/src/lib.rs`

- [ ] **Step 1: 失敗するテストを書く**

`chord_midi/src/textedit.rs` を作り、**テストだけ**を書く (関数はまだ無い):

```rust
//! textarea のカーソル位置を起点にテキストを書き換える純粋関数。
//!
//! DOM に触らないので workspace のテストに載る。`chord_midi_web` は
//! workspace から外れていて `cargo test` が回らないため、判断のある処理は
//! ここに置き、web 側は web-sys を呼ぶだけの薄い層にする。

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replace_range_puts_cursor_after_the_replacement() {
        let e = replace_range("C | Am |", 4, 6, "F");
        assert_eq!(e.text, "C | F |");
        assert_eq!(e.cursor, 5);
    }

    #[test]
    fn test_wrap_range_wraps_a_selection() {
        // `C | E D` を選んで pow で包む
        let text = "C | E D\n";
        let e = wrap_range(text, 0, 7, "pow(", ")");
        assert_eq!(e.text, "pow(C | E D)\n");
        // 選択があるときは包んだ後ろにカーソルを置く
        assert_eq!(e.cursor, 12);
    }

    #[test]
    fn test_wrap_range_without_selection_puts_cursor_inside() {
        let e = wrap_range("", 0, 0, "pow(", ")");
        assert_eq!(e.text, "pow()");
        // 括弧の内側
        assert_eq!(e.cursor, 4);
    }

    #[test]
    fn test_wrap_range_accepts_reversed_selection() {
        // 右から左へドラッグすると start > end で渡ってくる
        let e = wrap_range("C | E D\n", 7, 0, "pow(", ")");
        assert_eq!(e.text, "pow(C | E D)\n");
    }

    #[test]
    fn test_pat_span_inside_the_quotes() {
        let text = "pat(\"oxox\", C)\n";
        // `oxox` は 5..9
        assert_eq!(pat_pattern_span(text, 6), Some((5, 9)));
        // 引用符に張り付いていても中とみなす
        assert_eq!(pat_pattern_span(text, 5), Some((5, 9)));
        assert_eq!(pat_pattern_span(text, 9), Some((5, 9)));
    }

    #[test]
    fn test_pat_span_is_none_outside_the_quotes() {
        let text = "pat(\"oxox\", C)\n";
        // `C` の上
        assert_eq!(pat_pattern_span(text, 12), None);
        // `pat(` の上
        assert_eq!(pat_pattern_span(text, 1), None);
    }

    #[test]
    fn test_pat_span_picks_the_containing_one_when_nested() {
        //             0123456789...
        let text = "pat(\"oxox\", pat(\"oo\", C))\n";
        let outer = pat_pattern_span(text, 6).unwrap();
        let inner = pat_pattern_span(text, 18).unwrap();
        assert_eq!(&text[outer.0..outer.1], "oxox");
        assert_eq!(&text[inner.0..inner.1], "oo");
    }

    #[test]
    fn test_pat_span_ignores_unterminated_quote() {
        assert_eq!(pat_pattern_span("pat(\"oxox, C)\n", 7), None);
    }

    #[test]
    fn test_pat_span_ignores_non_pattern_body() {
        // 中身が o . x 以外なら別物。壊さない
        assert_eq!(pat_pattern_span("pat(\"abc\", C)\n", 6), None);
    }

    #[test]
    fn test_pat_span_ignores_a_reference_argument() {
        // Phase 2 の `pat($eight, ...)`。引用符が無いので区間は作らない
        assert_eq!(pat_pattern_span("pat($eight, C)\n", 7), None);
    }

    #[test]
    fn test_pat_span_ignores_comment_line() {
        assert_eq!(pat_pattern_span("# pat(\"oxox\", C)\n", 8), None);
    }

    #[test]
    fn test_pat_span_stays_on_the_cursor_line() {
        let text = "pat(\"oxox\", C)\nC | Am |\n";
        // 2行目にカーソルがあるなら1行目の pat は当たらない
        assert_eq!(pat_pattern_span(text, 17), None);
    }

    #[test]
    fn test_define_names_picks_line_heads() {
        let names = define_names("$eight = \"o.o.o.o.\"\n$riff  = pow(C | E D)\n$riff | C |\n");
        assert_eq!(names, vec!["eight".to_string(), "riff".to_string()]);
    }

    #[test]
    fn test_define_names_allows_japanese() {
        assert_eq!(define_names("$Aメロ = C | Am |\n"), vec!["Aメロ".to_string()]);
    }

    #[test]
    fn test_define_names_ignores_references_and_sustain() {
        // 参照だけの行、`=` (伸ばす) の小節、`$` の付かない行は拾わない
        assert!(define_names("$A | $B |\nC | = |\nkey: C\n").is_empty());
    }

    #[test]
    fn test_define_names_dedupes_keeping_first() {
        assert_eq!(define_names("$a = C\n$a = D\n"), vec!["a".to_string()]);
    }

    #[test]
    fn test_utf16_and_byte_round_trip() {
        let text = "$Aメロ = C\n";
        // `$Aメロ` は UTF-16 で 4 単位 ($ A メ ロ)、バイトで 8 (1+1+3+3)
        let b = utf16_to_byte(text, 4);
        assert_eq!(b, 8);
        assert_eq!(byte_to_utf16(text, b), 4);
        // 範囲外は末尾に丸める
        assert_eq!(utf16_to_byte(text, 999), text.len());
    }
}
```

- [ ] **Step 2: 落ちることを確認**

```bash
cargo test -p chord_midi --lib textedit
```

Expected: FAIL (コンパイルエラー。`replace_range` などが無い。`lib.rs` に `pub mod textedit;` もまだ無いので、まずそこから)

- [ ] **Step 3: `lib.rs` に足す**

`chord_midi/src/lib.rs`:

```rust
pub mod edit;
pub mod export;
pub mod fingering;
pub mod import;
pub mod lint;
pub mod model;
pub mod presets;
pub mod textedit;
```

(`lint` は Phase 2 で入っている。並列で走っている Task 2 が `pub mod reference;` を足すので、マージのときは両方残す)

- [ ] **Step 4: 位置の計算を書く**

`chord_midi/src/textedit.rs` のテストの前に置く:

```rust
use once_cell::sync::Lazy;
use regex::Regex;

/// 編集の結果。位置はすべてバイトオフセット
#[derive(Debug, Clone, PartialEq)]
pub struct Edited {
    pub text: String,
    /// 編集後のテキストでのカーソル位置
    pub cursor: usize,
}

/// `start <= end` に直し、文字境界へ丸める
fn clamp(text: &str, start: usize, end: usize) -> (usize, usize) {
    let (start, end) = if start <= end {
        (start, end)
    } else {
        (end, start)
    };
    (floor_boundary(text, start), floor_boundary(text, end))
}

/// `i` を文字の先頭まで戻す。textarea 由来の位置をそのまま
/// スライスに使うと、日本語の途中で panic する
fn floor_boundary(text: &str, mut i: usize) -> usize {
    if i >= text.len() {
        return text.len();
    }
    while i > 0 && !text.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// `[start, end)` を `replacement` で置き換え、カーソルを置換後の末尾に置く
pub fn replace_range(text: &str, start: usize, end: usize, replacement: &str) -> Edited {
    let (start, end) = clamp(text, start, end);
    let mut out = String::with_capacity(text.len() + replacement.len());
    out.push_str(&text[..start]);
    out.push_str(replacement);
    out.push_str(&text[end..]);
    Edited {
        cursor: start + replacement.len(),
        text: out,
    }
}

/// `[start, end)` を `open` / `close` で包む。
/// 選択が空なら `open` + `close` を挿入し、カーソルを括弧の内側に置く
pub fn wrap_range(text: &str, start: usize, end: usize, open: &str, close: &str) -> Edited {
    let (start, end) = clamp(text, start, end);
    let selected = &text[start..end];
    let mut out = String::with_capacity(text.len() + open.len() + close.len());
    out.push_str(&text[..start]);
    out.push_str(open);
    out.push_str(selected);
    out.push_str(close);
    out.push_str(&text[end..]);
    // 選択があれば包んだ後ろへ、無ければ括弧の内側へ
    let cursor = if selected.is_empty() {
        start + open.len()
    } else {
        start + open.len() + selected.len() + close.len()
    };
    Edited { text: out, cursor }
}

/// UTF-16 コード単位のオフセット (textarea の `selectionStart`) をバイト位置に直す
pub fn utf16_to_byte(text: &str, offset: u32) -> usize {
    let mut units = 0u32;
    for (i, c) in text.char_indices() {
        if units >= offset {
            return i;
        }
        units += c.len_utf16() as u32;
    }
    text.len()
}

/// バイト位置を UTF-16 コード単位のオフセットに直す
pub fn byte_to_utf16(text: &str, offset: usize) -> u32 {
    let offset = floor_boundary(text, offset);
    text[..offset].chars().map(|c| c.len_utf16() as u32).sum()
}
```

- [ ] **Step 5: `pat("...")` の探索を書く**

同じファイルに足す:

```rust
/// カーソルのある行の開始バイト位置と、その行の中身 (改行を含まない)
fn line_at(text: &str, cursor: usize) -> (usize, &str) {
    let start = text[..cursor].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let end = text[cursor..]
        .find('\n')
        .map(|i| cursor + i)
        .unwrap_or(text.len());
    (start, &text[start..end])
}

/// カーソルが `pat("....")` のパターン文字列の中にあるとき、
/// 引用符の内側の `[start, end)` をバイトオフセットで返す。
///
/// AST を使わないので、打鍵の途中でパースが落ちていても動く。探索は
/// カーソルのある1行に閉じる。パターン文字列に改行は入らない
/// (`pattern_parser` が `o` `.` `x` しか受け付けない) ので、これで
/// 引用符の閉じ忘れが行をまたいで暴走することがない。
pub fn pat_pattern_span(text: &str, cursor: usize) -> Option<(usize, usize)> {
    let cursor = floor_boundary(text, cursor);
    let (line_start, line) = line_at(text, cursor);
    // コメント行は触らない
    if line.trim_start().starts_with('#') {
        return None;
    }
    let mut from = 0usize;
    while let Some(rel) = line[from..].find("pat(\"") {
        let open = from + rel + "pat(\"".len();
        // 引用符が閉じていない。この行のここから先に区間は作れない
        let close = open + line[open..].find('"')?;
        // 中身がパターン文字でないなら別物。壊さない
        if line[open..close].chars().all(|c| matches!(c, 'o' | '.' | 'x')) {
            let (a, b) = (line_start + open, line_start + close);
            // どちらの引用符に張り付いていても「中」とみなす
            if (a..=b).contains(&cursor) {
                return Some((a, b));
            }
        }
        from = close + 1;
    }
    None
}
```

- [ ] **Step 6: 定義名の抽出を書く**

同じファイルに足す:

```rust
/// 行頭の `$name =` を拾う。`import/chord.rs` と同じく module 直下の Lazy に置く
static DEFINE_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^[ \t]*\$([0-9A-Za-z_\p{Hiragana}\p{Katakana}\p{Han}ー]+)[ \t]*=").unwrap()
});

/// テキストから定義名を拾う。現れた順で、重複は最初の1つだけ残す。
///
/// AST を通さないのは、打鍵の途中でパースが落ちていてもチップを
/// 消さないため。`=` を要求するので、参照 (`$A | $B |`) は拾わない
pub fn define_names(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for caps in DEFINE_REGEX.captures_iter(text) {
        let name = caps[1].to_string();
        if !out.contains(&name) {
            out.push(name);
        }
    }
    out
}
```

`ー` (U+30FC 長音記号) は Unicode の script が Common なので、`\p{Katakana}` に入らない。`$Aメロー` のような名前のために明示で足す。

- [ ] **Step 7: 通ることを確認**

```bash
cargo test -p chord_midi --lib textedit
```

Expected: 17 tests すべて PASS

- [ ] **Step 8: 全テストとゴールデンを確認**

```bash
cargo test --workspace && /usr/bin/git status --short chord_midi/tests/golden/
```

Expected: PASS、golden に差分なし (出力は空)

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "feat: カーソル位置を起点にテキストを編集する純粋関数を足す"
```

---

## Task 2: `chord_midi/src/reference.rs` — 記法テーブル (並列可)

**Files:**
- Create: `chord_midi/src/reference.rs`
- Modify: `chord_midi/src/lib.rs`

- [ ] **Step 1: 失敗するテストを書く**

`chord_midi/src/reference.rs` を作り、**テストだけ**を書く:

```rust
//! 記法の一覧表。web の記法パネルと CLI の `--help-syntax` が同じものを出す。
//! `presets.rs` と同じ静的テーブル。生成元の JSON は持たないので手で足す。

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
```

- [ ] **Step 2: 落ちることを確認**

```bash
cargo test -p chord_midi --lib reference
```

Expected: FAIL (コンパイルエラー。`SYMBOLS` などが無い)

- [ ] **Step 3: `lib.rs` に足す**

```rust
pub mod reference;
```

を `pub mod presets;` の次に置く。

- [ ] **Step 4: 型とテーブルを書く**

`chord_midi/src/reference.rs` のテストの前に置く:

```rust
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
```

- [ ] **Step 5: 等幅テキストを書く**

同じファイルに足す:

```rust
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
```

`name_w` を `width(name) + width(args) + 2` にしているのは、1列目に `pow(小節列)` の形で出すため (括弧2文字ぶん)。

- [ ] **Step 6: 通ることを確認**

```bash
cargo test -p chord_midi --lib reference
```

Expected: 6 tests すべて PASS。`every_example_parses` が落ちたら**表の例が実際の記法とズレている**ので、例の方を直す

実物の見た目は Task 3 で `--help-syntax` から確認する。ここでは
`help_text_columns_are_aligned` が桁を見張っているので、目視は Task 3 に任せる。

- [ ] **Step 7: 全テストとゴールデンを確認**

```bash
cargo test --workspace && /usr/bin/git status --short chord_midi/tests/golden/
```

Expected: PASS、golden に差分なし

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "feat: 記法の一覧テーブルを chord_midi に置く"
```

---

## Task 3: CLI の `--help-syntax` (Task 2 の後。Task 4 と並列可)

**Files:**
- Modify: `chord_midi_cli/src/main.rs`
- Create: `chord_midi_cli/tests/help_syntax.rs`

- [ ] **Step 1: 失敗するテストを書く**

`chord_midi_cli/tests/help_syntax.rs`:

```rust
//! `--help-syntax` の配線テスト。表の中身は
//! `chord_midi::reference` のテストが見るので、ここは
//! 「入力ファイル無しで表が出て、正常終了する」ことだけ確かめる

use std::process::Command;

fn run() -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_chord_midi_cli"))
        .arg("--help-syntax")
        .output()
        .expect("run cli")
}

#[test]
fn help_syntax_prints_the_table() {
    let out = run();
    assert!(out.status.success(), "exit: {:?}", out.status);
    let text = String::from_utf8(out.stdout).expect("utf8");
    for needle in ["pow", "pat", "arp", "xN", "N.C.", "記号", "関数"] {
        assert!(text.contains(needle), "`{}` が出ていない:\n{}", needle, text);
    }
}

#[test]
fn help_syntax_does_not_need_an_input_file() {
    // `--list-presets` と同じ扱い。`--input` が無くてもエラーにしない
    let out = run();
    assert!(
        out.stderr.is_empty(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}
```

- [ ] **Step 2: 落ちることを確認**

```bash
cargo test -p chord_midi_cli --test help_syntax
```

Expected: FAIL (`--help-syntax` は未知の引数なので clap が exit code 2 で落ちる)

- [ ] **Step 3: 引数を足す**

`chord_midi_cli/src/main.rs` の `Cli` に、`list_presets` の隣へ:

```rust
    /// 記法の一覧を出して終わる
    #[arg(long)]
    help_syntax: bool,
```

`input` の doc コメントも直す:

```rust
    /// 入力ファイル。`--preset` / `--list-presets` / `--help-syntax` のときは省略できる
    #[arg(short, long)]
    input: Option<PathBuf>,
```

- [ ] **Step 4: `main` で早く返す**

`if args.list_presets {` の**前**に置く (記法の説明の方が「まず読むもの」なので上に出す):

```rust
    if args.help_syntax {
        print!("{}", chord_midi::reference::help_text());
        return Ok(());
    }
```

`use` は足さなくてよい。`chord_midi::presets::PRESETS` と違って1か所しか使わないのでフルパスで呼ぶ。

- [ ] **Step 5: 通ることを確認**

```bash
cargo test -p chord_midi_cli --test help_syntax
```

Expected: 2 tests PASS

- [ ] **Step 6: 実物を目で見る**

```bash
cargo run -q -p chord_midi_cli -- --help-syntax
```

Expected: 次のような等幅の表 (列が揃っていること)

```
記号
  |       小節の区切り                C | Am |
  =       前の音を伸ばす              C | = |
  ...
関数
  pow(小節列)        小節列  パワーコード化 (ルートと5th)  pow(C | E D)
  ...
```

列が揃っていなければ `reference.rs` の `width` / `pad` を疑う。**「だいたい揃っている」で済ませないこと。** ターミナルで実際に折り返さずに読めるか見る

- [ ] **Step 7: 既存の使い方が壊れていないことを確認**

```bash
cargo run -q -p chord_midi_cli -- --list-presets | head -3
printf 'key: C\nC | Am |\n' > /tmp/p3.txt && cargo run -q -p chord_midi_cli -- -i /tmp/p3.txt
```

Expected: どちらも今までどおり動く

- [ ] **Step 8: 全テストを確認**

```bash
cargo test --workspace
```

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "feat(cli): --help-syntax で記法の一覧を出す"
```

---

## Task 4: web の配線とパネルの器 (Task 1 の後。Task 3 と並列可)

この Task が `chord_midi_web/style.css` を**全部**書く。グループ C の4タスクが CSS で衝突しないようにするため、まだ存在しない部品のクラスもここで定義しておく (markup はこの計画が決めているので先に書ける)。

**Files:**
- Create: `chord_midi_web/src/cursor.rs`
- Modify: `chord_midi_web/src/main.rs`
- Modify: `chord_midi_web/style.css`

- [ ] **Step 1: `cursor.rs` を書く**

`chord_midi_web/src/cursor.rs`:

```rust
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
```

`selection_start` / `selection_end` / `set_selection_range` はどれも web-sys の `HtmlTextAreaElement` feature に入っており、yew 0.21 がその feature を有効にしている。`focus()` も yew が有効にする `HtmlElement` にある。**`chord_midi_web/Cargo.toml` は触らなくてよい。**

- [ ] **Step 2: `main.rs` に配線する**

`mod` 宣言に足す:

```rust
mod cursor;
mod diagram;
mod fretboard;
```

`use` に足す:

```rust
use cursor::Edit;
```

`App` の state に足す (`gutter_ref` の隣):

```rust
    let editor_ref = use_node_ref();
    // 挿入の後にカーソルを戻す位置 (UTF-16)。Yew が textarea の value を
    // 書き戻すとカーソルが末尾へ飛ぶので、描画の後に当て直す
    let caret = use_state(|| None::<u32>);
```

textarea に `ref` を付ける:

```rust
                <textarea
                    class="editor"
                    ref={editor_ref.clone()}
                    rows=10
                    ...
```

- [ ] **Step 3: `on_edit` とカーソル復元の effect を書く**

`on_insert` (運指タブ用) の隣に置く:

```rust
    // 記法パネルの全部品がここを通る。テキストの持ち主は input なので、
    // 部品は「何をしたいか」だけを投げてくる
    let on_edit = {
        let input = input.clone();
        let caret = caret.clone();
        let editor_ref = editor_ref.clone();
        Callback::from(move |edit: Edit| {
            let Some(ta) = editor_ref.cast::<HtmlTextAreaElement>() else {
                return;
            };
            let (text, pos) = cursor::insert_at_cursor(&ta, input.as_str(), &edit);
            // 2つの set は同じコールバックの中なので、次の描画には
            // どちらも反映される。effect はその描画の後に走る
            input.set(text);
            caret.set(Some(pos));
        })
    };

    // 描画が終わってからカーソルを戻す。None に戻しておくのは、次に
    // 同じ位置へ挿入したときも dep が変わって effect が走るようにするため
    {
        let editor_ref = editor_ref.clone();
        let caret = caret.clone();
        use_effect_with(*caret, move |pos| {
            if let (Some(pos), Some(ta)) = (*pos, editor_ref.cast::<HtmlTextAreaElement>()) {
                let _ = ta.focus();
                let _ = ta.set_selection_range(pos, pos);
                caret.set(None);
            }
        });
    }
```

`cursor::insert_at_cursor(&ta, &input, &edit)` の `&input` は `UseStateHandle<String>` の `Deref` で `&String` → `&str` に落ちる。

- [ ] **Step 4: エディタタブを2段組にする**

`let editor = html! {` の中身を包み直す。**既存の中身 (`editor-wrap` / `error` / `controls` / 進行プリセット / MIDI保存 / `details.ast`) はそのまま `editor-main` の中に入れる。** 進行プリセットの `block-label` だけ文言を変える:

```rust
    let editor = html! {
        <div class="editor-tab">
            <div class="editor-main">
                // …既存の中身をそのまま…
            </div>
            <details class="side-panel" open=true>
                <summary>{"記法パネル"}</summary>
                <div class="side-panel-body">
                    // Task 9 でここに4部品を差す
                </div>
            </details>
        </div>
    };
```

進行プリセットのラベル:

```rust
                <span class="block-label">{"進行プリセット (末尾に追記)"}</span>
```

- [ ] **Step 5: CSS を書く**

`chord_midi_web/style.css` の `@media (max-width: 640px)` ブロックの**直前**に足す:

```css
/* ---- エディタタブの2段組と記法パネル ---- */

.editor-tab {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(240px, 280px);
    gap: 16px;
    align-items: start;
}

/* textarea が grid の列をはみ出さないようにする */
.editor-main {
    min-width: 0;
}

.side-panel {
    padding: 8px 12px 12px;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    /* エディタを見ながら使う。フッタのプレイヤーのぶんを引いて収める */
    position: sticky;
    top: 12px;
    max-height: calc(100vh - 170px);
    overflow-y: auto;
}

.side-panel-body > .block:first-child {
    margin-top: 8px;
}

/* ---- 記法パネル: 関数ボタン ---- */

.palette {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
}

/* .btn の幅は文字数任せで、記法なので等幅にする */
.palette .btn {
    padding: 0 8px;
    font-family: var(--mono);
    font-size: 12px;
}

/* ---- 記法パネル: リズム入力 ---- */

.rhythm-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 6px;
}

.rhythm-preview {
    font-family: var(--mono);
    font-size: 12px;
    letter-spacing: 2px;
    color: var(--muted);
    overflow-x: auto;
}

.rhythm-grid {
    display: flex;
    flex-wrap: wrap;
    gap: 2px;
    margin-bottom: 8px;
}

/* マスは実ボタン。o / . / x の文字をそのまま出す */
.rhythm-cell {
    appearance: none;
    width: 24px;
    height: 28px;
    padding: 0;
    background: var(--panel);
    color: var(--ink);
    border: 1px solid var(--line);
    border-radius: 4px;
    font-family: var(--mono);
    font-size: 13px;
    line-height: 1;
    cursor: pointer;
}

.rhythm-cell:hover {
    background: var(--hover);
}

.rhythm-cell.is-rest {
    background: var(--gutter-bg);
    color: var(--muted);
}

.rhythm-help {
    margin: 6px 0 0;
    color: var(--muted);
    font-size: 11px;
    line-height: 1.5;
}

/* ---- 記法パネル: 定義チップ ---- */

.chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
}

/* 進行プリセットの2段ボタンと区別が付くよう、等幅1段にする */
.chip {
    background: var(--accent-soft);
    border-color: transparent;
    color: var(--accent);
    font-family: var(--mono);
    font-weight: 600;
}

.chips-empty {
    margin: 0;
    color: var(--muted);
    font-size: 11px;
    line-height: 1.5;
}

/* ---- 記法パネル: 記法表 ---- */

.syntax-ref {
    margin-top: 16px;
    padding-top: 8px;
    border-top: 1px solid var(--line);
}

.syntax-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 11px;
}

.syntax-table td {
    padding: 2px 4px 2px 0;
    vertical-align: top;
    border-bottom: 1px solid var(--line);
}

.syntax-table code {
    font-family: var(--mono);
    font-size: 11px;
    white-space: nowrap;
}

.syntax-example {
    color: var(--muted);
}
```

さらに、既存の `.ast > summary` 一式のセレクタに新しい `<details>` を足す (**新しいルールを足すのではなく、既存のセレクタに並べる**。三角形の描き方を2か所に持ちたくない):

```css
.ast > summary,
.side-panel > summary,
.syntax-ref > summary { /* …既存の中身そのまま… */ }

.ast > summary::-webkit-details-marker,
.side-panel > summary::-webkit-details-marker,
.syntax-ref > summary::-webkit-details-marker { display: none; }

.ast > summary::before,
.side-panel > summary::before,
.syntax-ref > summary::before { /* …既存の中身そのまま… */ }

.ast[open] > summary::before,
.side-panel[open] > summary::before,
.syntax-ref[open] > summary::before { transform: rotate(90deg); }

.ast > summary:hover,
.side-panel > summary:hover,
.syntax-ref > summary:hover { color: var(--ink); }
```

最後に、既存の `@media (max-width: 640px)` ブロックの**中**に足す:

```css
    /* 狭い画面ではパネルをエディタの下に回す */
    .editor-tab {
        grid-template-columns: minmax(0, 1fr);
    }

    .side-panel {
        position: static;
        max-height: none;
    }
```

- [ ] **Step 6: ビルドが通ることを確認**

```bash
(cd chord_midi_web && cargo check --target wasm32-unknown-unknown)
```

Expected: PASS。`cursor` が未使用で warning が出るのは Task 9 まで想定内

- [ ] **Step 7: 目で確認する**

```bash
(cd chord_midi_web && trunk serve)
```

ブラウザで開き、次を確かめる:

1. エディタタブが2段組になり、右に「記法パネル」の空の `<details>` が出る
2. ウィンドウを 640px 以下に狭めると、パネルがエディタの下に回る
3. `<summary>` の三角が `解釈結果` と同じ見た目で、クリックで畳める
4. **進行プリセットのボタンが今までどおり動く** (ラベルだけ変わっている)
5. プレイヤーと運指タブが壊れていない

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "feat(web): カーソル位置への挿入の配線と記法パネルの器"
```

---

## Task 5 / 6 / 7 / 8 は並列に実行してよい

それぞれ `chord_midi_web/src/` に新規ファイルを1本作り、`main.rs` の `mod` 宣言に1行足すだけ。**`main.rs` のそれ以外と `style.css` には触らないこと。** 並列で走らせたときに衝突するのは `mod` 宣言の塊だけで、解決は4行すべてを残す。

---

## Task 5: `FuncPalette` — 関数ボタン (並列可)

**Files:**
- Create: `chord_midi_web/src/palette.rs`
- Modify: `chord_midi_web/src/main.rs` (`mod palette;` の1行だけ)

- [ ] **Step 1: `main.rs` に `mod palette;` を足す**

```rust
mod cursor;
mod diagram;
mod fretboard;
mod palette;
```

- [ ] **Step 2: コンパイルが落ちることを確認**

```bash
(cd chord_midi_web && cargo check --target wasm32-unknown-unknown)
```

Expected: FAIL (`file not found for module palette`)

- [ ] **Step 3: `palette.rs` を書く**

```rust
//! 関数ボタン。選択範囲があれば包み、無ければ雛形を入れる。
//!
//! テキストは持たない。`fretboard.rs` と同じく、やりたいことを
//! `Callback` で親に投げるだけ

use chord_midi::reference::{Scope, FUNCTIONS};
use yew::prelude::*;

use crate::cursor::Edit;

#[derive(Properties, PartialEq)]
pub struct FuncPaletteProps {
    pub on_edit: Callback<Edit>,
}

/// 関数1つぶんの編集要求。`Edit::Wrap` は選択の有無で振る舞いが変わる
/// (選択があれば包む / 無ければ挿入して括弧の内側にカーソル) ので、
/// 設計書の「選択があれば包む、無ければ雛形」はこれ1つで足りる
fn wrap_for(name: &str) -> Edit {
    if name == "pat" {
        // pat だけは第1引数が要る。8分の頭打ちを雛形にする
        Edit::Wrap("pat(\"o.o.o.o.\", ".to_string(), ")".to_string())
    } else {
        Edit::Wrap(format!("{}(", name), ")".to_string())
    }
}

fn buttons(scope: Scope, on_edit: &Callback<Edit>) -> Html {
    FUNCTIONS
        .iter()
        .filter(|f| f.scope == scope)
        .map(|f| {
            let on_edit = on_edit.clone();
            let edit = wrap_for(f.name);
            let onclick = Callback::from(move |_| on_edit.emit(edit.clone()));
            html! {
                <button class="btn" {onclick}
                    title={format!("{}  例: {}", f.summary, f.example)}>
                    {f.name}
                </button>
            }
        })
        .collect::<Html>()
}

#[function_component(FuncPalette)]
pub fn func_palette(props: &FuncPaletteProps) -> Html {
    // `( )` と `xN` は関数ではないが、「選択を包む」操作は同じなので並べる
    let on_group = {
        let on_edit = props.on_edit.clone();
        Callback::from(move |_| on_edit.emit(Edit::Wrap("(".to_string(), ")".to_string())))
    };
    let on_repeat = {
        let on_edit = props.on_edit.clone();
        Callback::from(move |_| on_edit.emit(Edit::Wrap("(".to_string(), ") x2".to_string())))
    };
    html! {
        <>
            <div class="block">
                <span class="block-label">{"小節列に掛ける"}</span>
                <div class="palette">{ buttons(Scope::Measures, &props.on_edit) }</div>
            </div>
            <div class="block">
                <span class="block-label">{"コードに掛ける"}</span>
                <div class="palette">{ buttons(Scope::Chord, &props.on_edit) }</div>
            </div>
            <div class="block">
                <span class="block-label">{"まとめ・繰り返し"}</span>
                <div class="palette">
                    <button class="btn" onclick={on_group}
                        title="選んだ範囲を ( ) でまとめる">{"( )"}</button>
                    <button class="btn" onclick={on_repeat}
                        title="選んだ範囲を ( ) x2 で包む">{"( ) x2"}</button>
                </div>
            </div>
        </>
    }
}
```

- [ ] **Step 4: 通ることを確認**

```bash
(cd chord_midi_web && cargo check --target wasm32-unknown-unknown)
```

Expected: PASS (`FuncPalette` が未使用の warning は Task 9 まで想定内)

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(web): 関数ボタンのパレット"
```

---

## Task 6: `RhythmGrid` — リズム入力 (並列可)

**Files:**
- Create: `chord_midi_web/src/rhythm.rs`
- Modify: `chord_midi_web/src/main.rs` (`mod rhythm;` の1行だけ)

- [ ] **Step 1: `main.rs` に `mod rhythm;` を足す**

- [ ] **Step 2: コンパイルが落ちることを確認**

```bash
(cd chord_midi_web && cargo check --target wasm32-unknown-unknown)
```

Expected: FAIL (`file not found for module rhythm`)

- [ ] **Step 3: `rhythm.rs` を書く**

```rust
//! リズム入力のグリッド。マスを `o` → `.` → `x` と回して、
//! できたパターンを `pat()` としてエディタに入れる。
//!
//! 文字列は `chord_midi` の `Pattern` の `Display` で作る。自分で
//! 組むとパーサの受け付ける形とズレる余地が残るため

use chord_midi::model::ast::{Beat, Pattern};
use web_sys::HtmlSelectElement;
use yew::prelude::*;

use crate::cursor::Edit;

#[derive(Properties, PartialEq)]
pub struct RhythmGridProps {
    pub on_edit: Callback<Edit>,
}

/// グリッドで扱う分割数。記法は 1 と 2 も許すが、マスが少なすぎて
/// GUI にする意味がないので手で書いてもらう
const DIVISIONS: [usize; 3] = [4, 8, 16];

/// `o` → `.` → `x` → `o` の順に回す
fn next_beat(b: Beat) -> Beat {
    match b {
        Beat::Hit => Beat::Rest,
        Beat::Rest => Beat::Brush,
        Beat::Brush => Beat::Hit,
    }
}

fn beat_char(b: Beat) -> &'static str {
    match b {
        Beat::Hit => "o",
        Beat::Rest => ".",
        Beat::Brush => "x",
    }
}

/// 頭打ち。README の `"o.o.o.o."` と同じ形
fn default_beats(n: usize) -> Vec<Beat> {
    (0..n)
        .map(|i| if i % 2 == 0 { Beat::Hit } else { Beat::Rest })
        .collect()
}

#[function_component(RhythmGrid)]
pub fn rhythm_grid(props: &RhythmGridProps) -> Html {
    let beats = use_state(|| default_beats(8));
    let pattern = Pattern((*beats).clone()).to_string();

    // 分割数を変えたらマスは作り直す。4分割の `o.o.` を8分割へ
    // 引き伸ばす決まった正解が無いため
    let on_division = {
        let beats = beats.clone();
        Callback::from(move |e: Event| {
            let el: HtmlSelectElement = e.target_unchecked_into();
            if let Ok(n) = el.value().parse::<usize>() {
                beats.set(default_beats(n));
            }
        })
    };

    let cells = (*beats)
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let b = *b;
            let onclick = {
                let beats = beats.clone();
                Callback::from(move |_| {
                    let mut next = (*beats).clone();
                    next[i] = next_beat(b);
                    beats.set(next);
                })
            };
            let class = classes!("rhythm-cell", matches!(b, Beat::Rest).then_some("is-rest"));
            // マスは見た目でしか区別できないので aria-label で言葉にする
            html! {
                <button type="button" {class} {onclick}
                    aria-label={format!("{}つ目 {}", i + 1, beat_char(b))}>
                    {beat_char(b)}
                </button>
            }
        })
        .collect::<Html>();

    let on_insert = {
        let on_edit = props.on_edit.clone();
        let pattern = pattern.clone();
        Callback::from(move |_| on_edit.emit(Edit::Rhythm(pattern.clone())))
    };

    html! {
        <div class="block">
            <span class="block-label">{"リズム"}</span>
            <div class="rhythm-row">
                <select onchange={on_division}>
                    { for DIVISIONS.iter().map(|n| html! {
                        <option value={n.to_string()} selected={*n == beats.len()}>
                            {format!("{}分割", n)}
                        </option>
                    }) }
                </select>
                <code class="rhythm-preview">{pattern}</code>
            </div>
            <div class="rhythm-grid">{cells}</div>
            <button class="btn btn-primary" onclick={on_insert}>{"pat() を入れる"}</button>
            <p class="rhythm-help">
                {"カーソルが pat(\"…\") の引用符の中にあるときは、パターンだけを差し替える。"}
            </p>
        </div>
    }
}
```

`DIVISIONS` の 4 / 8 / 16 はどれも `pattern_parser` が許す長さ (1/2/4/8/16) なので、このグリッドから出た文字列は必ずパースできる。

- [ ] **Step 4: 通ることを確認**

```bash
(cd chord_midi_web && cargo check --target wasm32-unknown-unknown)
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(web): リズム入力のグリッド"
```

---

## Task 7: `DefineChips` — 定義の一覧 (並列可)

**Files:**
- Create: `chord_midi_web/src/defines.rs`
- Modify: `chord_midi_web/src/main.rs` (`mod defines;` の1行だけ)

- [ ] **Step 1: `main.rs` に `mod defines;` を足す**

- [ ] **Step 2: コンパイルが落ちることを確認**

```bash
(cd chord_midi_web && cargo check --target wasm32-unknown-unknown)
```

Expected: FAIL (`file not found for module defines`)

- [ ] **Step 3: `defines.rs` を書く**

```rust
//! 今のテキストに書かれている定義をボタンにする。
//! クリックでカーソル位置に `$name` を入れる。
//!
//! AST ではなく生テキストを見る (`textedit::define_names`)。打鍵の
//! 途中でパースが落ちていてもチップが消えないようにするため

use chord_midi::textedit::define_names;
use yew::prelude::*;

use crate::cursor::Edit;

#[derive(Properties, PartialEq)]
pub struct DefineChipsProps {
    /// 今のエディタの中身
    pub text: String,
    pub on_edit: Callback<Edit>,
}

#[function_component(DefineChips)]
pub fn define_chips(props: &DefineChipsProps) -> Html {
    let names = define_names(&props.text);
    let body = if names.is_empty() {
        // 0件でもブロックは消さない。列の高さが跳ねないのと、
        // ここで記法を教えられるのが理由
        html! {
            <p class="chips-empty">{"行頭に $名前 = ... と書くと、ここに出る"}</p>
        }
    } else {
        let chips = names
            .iter()
            .map(|name| {
                let label = format!("${}", name);
                let onclick = {
                    let on_edit = props.on_edit.clone();
                    let label = label.clone();
                    Callback::from(move |_| on_edit.emit(Edit::Insert(label.clone())))
                };
                html! { <button class="btn chip" {onclick}>{label}</button> }
            })
            .collect::<Html>();
        html! { <div class="chips">{chips}</div> }
    };
    html! {
        <div class="block">
            <span class="block-label">{"定義 (カーソル位置に挿入)"}</span>
            {body}
        </div>
    }
}
```

- [ ] **Step 4: 通ることを確認**

```bash
(cd chord_midi_web && cargo check --target wasm32-unknown-unknown)
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(web): 定義の一覧チップ"
```

---

## Task 8: `SyntaxReference` — 記法表 (並列可)

**Files:**
- Create: `chord_midi_web/src/syntax_ref.rs`
- Modify: `chord_midi_web/src/main.rs` (`mod syntax_ref;` の1行だけ)

- [ ] **Step 1: `main.rs` に `mod syntax_ref;` を足す**

- [ ] **Step 2: コンパイルが落ちることを確認**

```bash
(cd chord_midi_web && cargo check --target wasm32-unknown-unknown)
```

Expected: FAIL (`file not found for module syntax_ref`)

- [ ] **Step 3: `syntax_ref.rs` を書く**

```rust
//! 記法の一覧表。CLI の `--help-syntax` と同じ
//! `chord_midi::reference` のテーブルから作る。
//!
//! 既定で開いておく (常設表示) が、行数が多いので畳めるようにする。
//! パネル自体が `overflow-y: auto` なので、開いたまま使える

use chord_midi::reference::{Function, Symbol, BEATS, FUNCTIONS, SYMBOLS};
use yew::prelude::*;

fn symbol_table(rows: &[Symbol]) -> Html {
    html! {
        <table class="syntax-table">
            { for rows.iter().map(|s| html! {
                <tr>
                    <td><code>{s.symbol}</code></td>
                    <td>{s.summary}</td>
                    <td><code class="syntax-example">{s.example}</code></td>
                </tr>
            }) }
        </table>
    }
}

fn function_table(rows: &[Function]) -> Html {
    html! {
        <table class="syntax-table">
            { for rows.iter().map(|f| html! {
                <tr>
                    <td><code>{format!("{}({})", f.name, f.args)}</code></td>
                    <td>{f.summary}</td>
                    <td><code class="syntax-example">{f.example}</code></td>
                </tr>
            }) }
        </table>
    }
}

#[function_component(SyntaxReference)]
pub fn syntax_reference() -> Html {
    html! {
        <details class="syntax-ref" open=true>
            <summary>{"記法の一覧"}</summary>
            <div class="block">
                <span class="block-label">{"記号"}</span>
                { symbol_table(SYMBOLS) }
            </div>
            <div class="block">
                <span class="block-label">{"リズムの文字"}</span>
                { symbol_table(BEATS) }
            </div>
            <div class="block">
                <span class="block-label">{"関数"}</span>
                { function_table(FUNCTIONS) }
            </div>
        </details>
    }
}
```

- [ ] **Step 4: 通ることを確認**

```bash
(cd chord_midi_web && cargo check --target wasm32-unknown-unknown)
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(web): 記法の一覧表"
```

---

## Task 9: パネルに4部品をマウントする

**Files:**
- Modify: `chord_midi_web/src/main.rs`

- [ ] **Step 1: `mod` 宣言が7本そろっていることを確認**

グループ C を並列で走らせた場合、ここが衝突している可能性がある。7本すべてあることを確かめる:

```bash
grep -n "^mod " chord_midi_web/src/main.rs
```

Expected:

```
mod cursor;
mod defines;
mod diagram;
mod fretboard;
mod palette;
mod rhythm;
mod syntax_ref;
```

足りなければ足す。

- [ ] **Step 2: `use` を足す**

```rust
use defines::DefineChips;
use palette::FuncPalette;
use rhythm::RhythmGrid;
use syntax_ref::SyntaxReference;
```

既存の `use diagram::ChordDiagram;` / `use fretboard::Fretboard;` の並びに合わせる。

- [ ] **Step 3: パネルに差す**

Task 4 で空にしておいた `side-panel-body` を埋める:

```rust
                <div class="side-panel-body">
                    <DefineChips text={(*input).clone()} on_edit={on_edit.clone()} />
                    <RhythmGrid on_edit={on_edit.clone()} />
                    <FuncPalette on_edit={on_edit.clone()} />
                    <SyntaxReference />
                </div>
```

並びの理由: 上から「今のファイル固有のもの (定義)」→「よく使う入力 (リズム・関数)」→「常設の一覧」。一覧は行数が多いので一番下。

- [ ] **Step 4: ビルドが通ることを確認**

```bash
(cd chord_midi_web && cargo check --target wasm32-unknown-unknown && trunk build)
```

Expected: PASS、未使用の warning が消えている

- [ ] **Step 5: 手で通しで確かめる**

```bash
(cd chord_midi_web && trunk serve)
```

エディタに次を貼って、1つずつ確かめる。

```
key: C

$eight = "o.o.o.o."
$riff  = pow(C | E D)

$riff | C |
```

| 操作 | 期待 |
|---|---|
| 定義チップに `$eight` `$riff` が出る | 出る。`$riff \| C \|` の参照は拾わない |
| `C` の後ろにカーソルを置き `$eight` をクリック | そこに `$eight` が入り、カーソルがその直後に残る (末尾に飛ばない) |
| `C \| E D` を選んで `pow` をクリック | `pow(C \| E D)` になる |
| 何も選ばずに `pow` をクリック | `pow()` が入り、カーソルが括弧の内側に来る |
| 何も選ばずに `pat` をクリック | `pat("o.o.o.o.", )` が入り、カーソルが `, ` の後ろに来る |
| `C \| E D` を選んで `( ) x2` をクリック | `(C \| E D) x2` になる |
| リズムのマスを押す | `o` → `.` → `x` と回り、右のプレビューが追う |
| 分割数を 16 にする | マスが16個になる |
| 何も選ばずに「pat() を入れる」 | カーソル位置に `pat("<今のパターン>", )` が入る |
| `pat("o.o.o.o.", ...)` の `o.o.o.o.` の中にカーソルを置いて「pat() を入れる」 | **パターン文字列だけが差し替わる。`pat` は増えない** |
| 定義名を `$Aメロ = C \| Am \|` にしてチップをクリック | 日本語でもカーソルがズレない (panic しない) |
| 記法の一覧を開く | 記号・リズムの文字・関数の3つの表が出る |
| ウィンドウを 640px 以下にする | パネルがエディタの下に回る |

**日本語の定義名の行は必ず試すこと。** UTF-16 とバイト位置の変換を間違えていると、ここだけで panic する (wasm では画面が固まる)。

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat(web): 記法パネルに4部品を載せる"
```

---

## Task 10: プレビューにセクションのラベルを出す

設計書 3.4。`$A` 由来の小節がどこまでかを図で分かるようにする。

**Files:**
- Modify: `chord_midi_web/src/main.rs`
- Modify: `chord_midi_web/style.css`

- [ ] **Step 1: `MeasureRef.section` が来ていることを確認**

```bash
printf '$A = C | Am |\n$A | F |\n' > /tmp/p3sec.txt
cargo run -q -p chord_midi_cli -- -i /tmp/p3sec.txt --print-ast expanded | grep -c Section
```

Expected: 1 以上。0 なら Phase 2 の Task 8 が入っていないので、そこから直す

- [ ] **Step 2: `cards` の組み立てを直す**

`chord_midi_web/src/main.rs` の `let cards = {` の中、`for measure in measures.iter() {` のループを直す。ループの前に:

```rust
        // 直前の小節のセクション名。名前は範囲の先頭の小節にだけ出す
        let mut prev_section: Option<String> = None;
```

`rows.push(...)` の直前に:

```rust
            // $定義 から展開された小節に帯を出す。名前は範囲の先頭だけに
            // 出し、続きは帯だけにする。枠は常に置いてカードの上端を揃える
            let is_head = measure.section != prev_section;
            let section = match (&measure.section, is_head) {
                (Some(name), true) => html! { <div class="section">{format!("${}", name)}</div> },
                _ => html! { <div class="section"></div> },
            };
            prev_section = measure.section.clone();
```

`rows.push` を:

```rust
            rows.push(html! {
                <div class={classes!("measure",
                    measure.section.is_some().then_some("in-section"))}>
                    { section }
                    { rhythm }
                    <div class="measure-cards">{cards}</div>
                </div>
            });
```

- [ ] **Step 3: CSS を足す**

`style.css` の `.rhythm` の定義の隣 (押さえ方ダイアグラムの節の中) に:

```css
/* $定義 から展開された小節。名前は範囲の先頭だけに出し、
   続きは下線で「どこまでか」を示す。枠は常に置いて上端を揃える */
.section {
    min-height: 15px;
    font-family: var(--mono);
    font-size: 11px;
    line-height: 15px;
    color: var(--accent);
    font-weight: 600;
    border-bottom: 2px solid transparent;
}

.measure.in-section .section {
    border-bottom-color: var(--accent-soft);
}
```

`border-bottom` を透明で常に置いているのは、セクションのある小節と無い小節で高さが変わらないようにするため (`.rhythm` の `min-height` と同じ考え方)。

- [ ] **Step 4: ビルドが通ることを確認**

```bash
(cd chord_midi_web && cargo check --target wasm32-unknown-unknown && trunk build)
```

- [ ] **Step 5: 目で確かめる**

```bash
(cd chord_midi_web && trunk serve)
```

エディタに貼る:

```
$A = C | Am |
$B = F | G |
$A x2 | $B |
```

Expected (プレビュータブ):

- 最初の小節の上に `$A`、続く3小節は名前なしで下線だけ (`$A` を2回展開したので4小節)
- 5小節目の上に `$B`、6小節目は下線だけ
- `$A x2 | $B |` を `C | Am |` だけに書き換えると、ラベルも下線も消える

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat(web): プレビューに定義のセクション名を出す"
```

---

## Task 11: README と整形

**Files:**
- Modify: `README.md`
- 全体 (`cargo fmt`)

- [ ] **Step 1: README の CLI 節に足す**

`# 表示` のブロックに1行:

```
    cargo run -p chord_midi_cli -- --help-syntax                  # 記法の一覧
```

`--help-syntax` は `-i` を要らない。`--list-presets` の説明の後ろに:

```markdown
記法の一覧は `chord_midi/src/reference.rs`（手で足す静的テーブル）を web と
CLI で共有している。進行プリセットと同じ流儀。
```

- [ ] **Step 2: README に記法パネルの節を足す**

`## 逆引き（運指タブ）` の**前**に:

```markdown
## 記法パネル（エディタタブ）

エディタの右（狭い画面では下）に、記法を思い出さずに書くためのパネルがある。
GUI は独自の状態を持たず、**テキストを編集するだけ**なので、パネルで作った
ものは手で書いたものと区別がつかない。

- **定義** — テキストに書かれている `$名前 = ...` をボタンにする。押すと
  カーソル位置に `$名前` が入る。行頭を正規表現で拾うだけなので、打鍵の
  途中でパースが落ちていてもボタンは消えない
- **リズム** — 4 / 8 / 16 分割のマスを `o` → `.` → `x` と回して作る。
  「pat() を入れる」でカーソル位置に `pat("…", )` が入る。カーソルが既存の
  `pat("…")` の引用符の中にあるときは、パターン文字列だけが差し替わる
- **関数** — `pow` `oct` `pat` / `arp` `blk` `s25` `sub` `dom` と
  `( )` `( ) x2`。選択範囲があれば包み、無ければ雛形を入れてカーソルを
  括弧の内側に置く
- **記法の一覧** — 記号と関数の表。CLI の `--help-syntax` と同じものが出る

「進行プリセット」はエディタ側に残っていて、こちらは**テキストの末尾に
追記する**。パネルの定義ボタンは**カーソル位置に挿入する**。
```

- [ ] **Step 3: README の例が動くことを確認**

```bash
cargo run -q -p chord_midi_cli -- --help-syntax | head -5
```

- [ ] **Step 4: README と表の同期テストが通ることを確認**

```bash
cargo test -p chord_midi --lib reference
```

Expected: `readme_mentions_every_function` を含めて PASS

- [ ] **Step 5: 整形する**

```bash
cargo fmt --all
(cd chord_midi_web && cargo fmt)
```

- [ ] **Step 6: 差分が整形だけであることを確認**

```bash
/usr/bin/git diff | grep -E "^[+-]" | grep -v "^[+-][+-]" | head -40
```

- [ ] **Step 7: 全部を通し直す**

```bash
cargo test --workspace
cargo fmt --all -- --check
(cd chord_midi_web && cargo fmt -- --check && cargo check --target wasm32-unknown-unknown && trunk build)
```

Expected: 全部成功

- [ ] **Step 8: ゴールデンが1バイトも変わっていないことを確認**

```bash
/usr/bin/git diff --stat main -- chord_midi/tests/golden/
```

Expected: **出力が空。** Phase 3 は記法にも変換にも触っていないので、ここに差分が出たらどこかで壊している

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "docs: 記法パネルと --help-syntax を README に書く"
```

---

## 完了条件

- `cargo test --workspace` が通る
- `cargo fmt --all -- --check` と `(cd chord_midi_web && cargo fmt -- --check)` が通る
- `(cd chord_midi_web && cargo check --target wasm32-unknown-unknown)` と `trunk build` が通る
- **ゴールデン MIDI が1バイトも変わっていない** (`git diff --stat main -- chord_midi/tests/golden/` が空)
- `cargo run -p chord_midi_cli -- --help-syntax` が列の揃った表を出し、`-i` を要求しない
- `reference::help_text()` と web の記法表が同じテーブルから出ている (`reference.rs` 以外に記法の一覧が無い)
- `every_example_parses` が通る (表の例が実際にパースできる)
- 記法パネルの4部品がすべてカーソル位置に効く。Task 9 Step 5 の表の全項目が期待どおり
- **日本語の定義名 (`$Aメロ`) でカーソルがズレない**
- 640px 以下でパネルがエディタの下に回る
- 進行プリセットと運指タブの「挿入」の挙動が Phase 2 から変わっていない
