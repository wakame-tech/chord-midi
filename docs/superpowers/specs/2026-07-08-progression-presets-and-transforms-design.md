# 進行プリセット・キー/スケール・コード変換 設計 (Issue #12)

日付: 2026-07-08
Issue: https://github.com/wakame-tech/chord-midi/issues/12

## 目的

`chord_midi` に3機能を追加する。すべて rechord テキスト記法に持たせ、web はテキストを編集するフロントとする。

1. **キー/スケール(旋法)設定** — 進行を任意キーの実音へ transpose し、旋法で度数→音の対応を変える。web はキー上下・スケール選択。
2. **進行プリセット** — ディグリー表記の定番進行を JSON で同梱し、web でボタン挿入。
3. **コード変換関数** — `arp(...)` 同様の関数記法で `blk`/`s25`/`sub`/`dom` を展開。

## 用語・変換定義 (ユーザー確定)

- `blk(X)` ブラックアダーコード = `(X+6)aug / X`。aug トライアド(ルート=Xのトライトーン上) を X ベース上に。例 `blk(F)` = `Baug/F` = 音 {F, B, D#, G}(ベースF基準の半音 {0,2,6,10})
- `s25(X)` ツーファイブ分割 = V を ii-V にリハモ。`(X.root+7 半音)m7` を前置し X は残す。例 `s25(G7)` = `Dm7 G7`
- `sub(X)` 裏コード化 = トライトーン代理。`(X.root+6 半音)dom7`。例 `sub(G7)` = `Db7`(X 置換)
- `dom(X)` ドミナント化 = X のルートで dominant 7th。例 `dom(Dm7)` = `D7`
- コンスタントストラクチャーは今回対象外(YAGNI)

## スコープ外 (YAGNI)

- コンスタントストラクチャー変換
- プリセットのユーザー編集/localStorage 保存(同梱固定セットのみ)
- 変換関数のネスト(`s25(sub(...))` 等)は初期対象外。関数の中身は単一 ChordNode のみ
- `into_degree` の旋法対応(逆変換=解析方向。従来どおり chromatic/Major 基準の `Degree::from_semitone` を使う)

---

## A. コア・データモデル変更

### A-1. `Key::Relative` を Degree 保持に

`chord_midi/src/model/key.rs`

現状 `Key::Relative(u8)` は「半音」を保持。パーサが `Scale::Major.semitone(度数)` で解決済みの半音を入れているため、旋法を変えられない。

**変更**: `Key::Relative(Degree)` にする(`Degree(pub u8, pub Accidental)` は `scale.rs` に既存、これを再利用)。パーサは旋法非依存に度数+臨時記号だけ記録し、旋法解決は `into_pitch` で行う。

```rust
pub enum Key {
    Absolute(Pitch),
    Relative(Degree), // 度数(1-7..)+臨時記号。旋法解決は into_pitch で
}
```

`Display for Key`: `Relative(deg)` は `write!(f, "{}", deg)`(`Degree` の Display は既存: 例 `bIII`)。

`into_pitch` / `into_degree` に `scale: Scale` を追加:

```rust
impl Key {
    pub fn into_pitch(self, tonic: Pitch, scale: Scale) -> Key {
        match self {
            Key::Absolute(_) => self,
            Key::Relative(Degree(n, acc)) => {
                let semitone = scale.semitone(n) as i8 + i8::from(acc);
                let p = ((semitone + tonic as i8).rem_euclid(12)) as u8;
                Key::Absolute(Pitch::try_from(p).unwrap())
            }
        }
    }

    pub fn into_degree(self, tonic: Pitch, _scale: Scale) -> Key {
        match self {
            Key::Absolute(p) => {
                let semitone = (p as i8 - tonic as i8).rem_euclid(12) as u8;
                Key::Relative(Degree::from_semitone(semitone)) // chromatic 逆変換(Major基準)
            }
            Key::Relative(_) => self,
        }
    }
}
```

- `Degree::with_pitch` は不要になる場合は残置(削除しない)。
- `Scale` は `Copy` でない(`derive(Clone)`)ので引数は値渡し `scale: Scale`(呼び出し側で `clone()`)。または `Scale` に `Copy` を追加。→ **`Scale` に `Copy` を derive 追加**(enum 全バリアントがユニット)して取り回しを楽にする。

### A-2. `Scale` に旋法追加

`chord_midi/src/model/scale.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Scale {
    Major,      // Ionian
    Minor,      // Aeolian
    Dorian,
    Phrygian,
    Lydian,
    Mixolydian,
    Locrian,
}

impl Scale {
    pub fn degrees(&self) -> Vec<u8> {
        match self {
            Scale::Major => vec![2, 2, 1, 2, 2, 2, 1],
            Scale::Minor => vec![2, 1, 2, 2, 1, 2, 2],
            Scale::Dorian => vec![2, 1, 2, 2, 2, 1, 2],
            Scale::Phrygian => vec![1, 2, 2, 2, 1, 2, 2],
            Scale::Lydian => vec![2, 2, 2, 1, 2, 2, 1],
            Scale::Mixolydian => vec![2, 2, 1, 2, 2, 1, 2],
            Scale::Locrian => vec![1, 2, 2, 1, 2, 2, 2],
        }
    }
    // semitone(), semitones() 現状維持
}
```

- `FromStr for Scale`(ディレクティブ用): `"major"/"ionian" -> Major`, `"minor"/"aeolian" -> Minor`, `"dorian" -> Dorian` … 大文字小文字無視。
- `Display for Scale`(ディレクティブ逆シリアライズ): 小文字名。
- `Chord::scale()`(コード質の major/minor 3rd 判定, `chord.rs`)は従来どおり `Scale::Major`/`Scale::Minor` を使い続ける。KEY のスケールとは別概念。**変更しない**。

### A-2b. `Degree` Display の修正 (既存バグ)

`scale.rs` の `Degree(度数番号, 臨時記号)` の Display は現状 `to_roman_str(self.0)` を使うが、`to_roman_str` は**半音(0-11)**→ローマ数字の写像。`self.0` は**度数番号(1-7)**なので誤ったローマ数字になる(例 `Degree(3, Natural)` が `II#` と表示される。正しくは `III`)。

`Key::Relative(Degree)` 化で Display が実際に使われるため、ここを直す:

```rust
fn degree_to_roman(n: u8) -> &'static str {
    match n { 1=>"I",2=>"II",3=>"III",4=>"IV",5=>"V",6=>"VI",7=>"VII", _=>"?" }
}
impl Display for Degree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.1, degree_to_roman(self.0)) // 臨時記号を前置: bIII
    }
}
```

臨時記号は前置(`bIII`, `#IV`)にし、パーサ(`opt(accidental_parser)` 前置)と一致させる。`Degree::from_semitone` が返す `(番号, 臨時記号)` は正しい度数表現なので、この Display 修正だけで `from_semitone(4)=Degree(3,Natural)→"III"`、`from_semitone(3)=Degree(2,Sharp)→"#II"` と整合する。既存 `to_roman_str` は他に使用箇所が無ければ削除。

注: `Tension` 側の `Degree` 表示(`modifier.rs`)はテンション記法(`b9` 等、番号=9,11,13)なので**別扱い**。`modifier.rs` の `Tension(Degree(number, accidental))` Display は前フェーズで `{accidental}{number}` に修正済み。度数ローマ変換は使わない(9/11/13 はローマにしない)。→ `Degree` の `Display` をローマにすると Tension 表示が壊れる懸念があるため、**`Degree` の `Display` はローマ(度数用)にし、Tension 側は `Degree` の Display を使わず `write!("{}{}", accidental, number)` を直接使う**(既にそう実装済みか確認し、そうでなければ直接展開する)。

### A-3. パーサの degree 対応

`chord_midi/src/import/chord.rs`

`degree_parser` は現状 `Scale::Major.semitone(d) + accidental` で半音 u8 を返し `key_parser` が `Key::Relative(u8)` を作る。

**変更**: `degree_parser` は `Degree(度数, 臨時記号)` を返し、`key_parser` が `Key::Relative(Degree)` を作る。旋法解決はしない。

```rust
fn degree_parser(s: Span) -> IResult<Span, Degree> {
    map(tuple((opt(accidental_parser), degree_name_parser)), |(a, d)| {
        Degree(d, a.unwrap_or(Accidental::Natural))
    })(s)
}
// key_parser: map(degree_parser, Key::Relative)
```

注: 現状 `accidental_parser` は必須だが、`bIII` も `III` も許容するため `opt(accidental_parser)` にする(度数は臨時記号無しが普通)。`degree_name_parser`(ローマ数字→u8)は既存流用。

`ChordNode::relative(semitone: u8)` ヘルパ(`ast.rs`)は `relative(degree: u8)` 相当に用途が変わる。**シグネチャを `relative(degree: Degree)` に変更**し、呼び出し側テストを更新。デフォルト臨時記号無しの簡易版として `ChordNode::relative_natural(n: u8)` を追加してもよい(テスト簡潔化)。

### A-4. ディレクティブ

`chord_midi/src/model/ast.rs` / `chord_midi/src/import/rechord.rs`

```rust
// ast.rs
pub enum Directive {
    Key(Pitch),
    Scale(Scale),
}
pub enum Ast {
    Comment(String),
    Directive(Directive), // 追加
    Measure(Vec<Node>, bool),
    Score(Vec<Box<Ast>>),
}
```

`rechord.rs` に `directive_parser` を追加し `ast_parser` の `alt` に(comment より前 or 後)加える:

```
key: <pitch>\n     -> Ast::Directive(Directive::Key(pitch))
scale: <name>\n    -> Ast::Directive(Directive::Scale(scale))
```

構文: 行頭 `key:` または `scale:`、`space0`、値、行末。`tag("key:")` 等で判定するので `C`(コード)や他のノードと衝突しない(コードは `key:` で始まらない)。

`Display for Ast`: `Directive::Key(p)` → `key: {p}\n`、`Directive::Scale(s)` → `scale: {s}\n`。

### A-5. 変換関数ノード

`chord_midi/src/model/ast.rs`

```rust
pub enum Transform { S25, Sub, Dom, Blk }

pub enum Node {
    Chord(ChordNode),
    Arpeggio(ChordNode),
    Transform(Transform, ChordNode), // 追加
    Rest,
    Sustain,
    Repeat,
}
```

パーサ(`import/chord.rs` `node_parser`)に `arp(` と同様の分岐を追加(chord 分岐より前):

```rust
map(delimited(tag("blk("), chord_node_parser, tag(")")), |c| Node::Transform(Transform::Blk, c)),
map(delimited(tag("s25("), chord_node_parser, tag(")")), |c| Node::Transform(Transform::S25, c)),
map(delimited(tag("sub("), chord_node_parser, tag(")")), |c| Node::Transform(Transform::Sub, c)),
map(delimited(tag("dom("), chord_node_parser, tag(")")), |c| Node::Transform(Transform::Dom, c)),
```

`Display for Node`(`export/rechord.rs`): `Transform(kind, c)` → `blk({c})` 等(kind→接頭辞)。`Transform` に `Display`(または接頭辞関数)。

`Display for Transform`: `S25 -> "s25"`, `Sub -> "sub"`, `Dom -> "dom"`, `Blk -> "blk"`。

### A-6. 展開ロジック `Transform::expand`

変換は Key の半音演算を伴うため、実音(Absolute)化後に展開する。`Key` に半音シフトのヘルパを追加:

```rust
// key.rs
impl Key {
    /// 半音シフト。Absolute は Pitch を回し、Relative は scale で解決してからシフトし度数へ戻す
    pub fn shift(self, semitones: i8, tonic: Pitch, scale: Scale) -> Key { ... }
}
```

`ast.rs` に展開:

```rust
impl Node {
    /// Transform を通常 Node 列へ。tonic/scale は Relative の半音演算に使用
    pub fn expand(self, tonic: Pitch, scale: Scale) -> Vec<Node> {
        match self {
            Node::Transform(t, c) => t.expand(c, tonic, scale),
            other => vec![other],
        }
    }
}

impl Transform {
    fn expand(self, c: ChordNode, tonic: Pitch, scale: Scale) -> Vec<Node> {
        match self {
            // s25(X) = (X.root+7)m7, X
            Transform::S25 => vec![
                Node::Chord(chord_at(c.key.clone().shift(7, tonic, scale), &[Modifier::Minor(7)], None)),
                Node::Chord(c),
            ],
            // sub(X) = (X.root+6)dom7
            Transform::Sub => vec![Node::Chord(chord_at(c.key.shift(6, tonic, scale), &[Modifier::Dominant(7)], None))],
            // dom(X) = (X.root)dom7
            Transform::Dom => vec![Node::Chord(chord_at(c.key, &[Modifier::Dominant(7)], None))],
            // blk(X) = (X.root+6)aug / X
            Transform::Blk => vec![Node::Chord(chord_at(
                c.key.clone().shift(6, tonic, scale),
                &[Modifier::Aug],
                Some(c.key), // on = X
            ))],
        }
    }
}

// ヘルパ: 指定 key + modifiers(+デフォルト Major(5)) + on の ChordNode を作る
fn chord_at(key: Key, mods: &[Modifier], on: Option<Key>) -> ChordNode { ... }
```

`chord_at` は `chord_node_parser` と同じくデフォルト `Modifier::Major(5)` を注入し、渡された modifiers を足す(dom/m7/aug 等がトライアドを上書き)。`on` は Blackadder のベース。

**構成音検証** (`blk(F)` を tonic なしの Absolute で):
- `F` の key = Absolute(F=5)。shift(6) → Absolute(B=11)。mods {Major(5), Aug}, on=Absolute(F)。
- `to_chord`: Aug トライアド(B基準 {0,4,8}) + bass F。into_note_numbers で {B, D#, G} + F。半音(F基準){0,2,6,10}。期待一致。

### A-7. 解決フロー `Ast::resolve`

`chord_midi/src/model/transform.rs`(または新規 `resolve.rs`)

```rust
impl Ast {
    /// key/scale ディレクティブを適用し、度数→実音 transpose と Transform 展開を行い、
    /// MIDI/演奏用の AST(Absolute chords, Directive/Comment 除去)を返す。
    pub fn resolve(self) -> Ast { ... }
}
```

手順:
1. Score を走査し最後の `Directive::Key` を tonic、最後の `Directive::Scale` を scale(既定 `Major`)として取得。
2. tonic があれば `into_pitch(tonic, scale)` を AST 全体に適用(度数→Absolute)。tonic 無しなら degree は Relative のまま(この場合 MIDI 生成は従来どおり degree で失敗しうる)。
3. 各 Measure の Node 列を `expand(tonic_or_default, scale)` でフラット化(Transform を通常 Chord 列へ)。展開で 1 Node が複数 Node になり、その小節のノード数が増える(既存の `measure_unit_size` が尺を再分割)。
4. `Comment`/`Directive` は演奏に無関係なので Score から除いてよい(Measure と Sustain/Rest 等は保持)。

注: `into_pitch`/`into_degree` は `Ast`/`Node`/`ChordNode` レベルにも scale 引数を追加でスレッドする(`transform.rs` 既存の再帰に scale を渡す)。`Node::Arpeggio` と `Node::Transform` の内部 ChordNode も transpose 対象に含める。

### A-8. MIDI/CLI 統合

- `MidiExporter::export` は「resolved 済み AST」を前提とする。または `export` 内で `ast.resolve()` を呼ぶ。→ **`export` 冒頭で `let ast = ast.resolve();` を呼ぶ**方式にし、呼び出し側の変更を最小化。
- `RechordExporter`(テキスト出力)は resolve せず元 AST を出す(記法保持)。

---

## B. web (`chord_midi_web/src/main.rs` + `presets.json`)

`compile` は現状 `import -> Display -> MidiExporter.export`。`export` が内部 resolve するので compile 側の変更は最小。

### B-1. キー上下

`key:` 行の pitch を ±1 半音して textarea を書換。無ければ先頭に `key: C\n` を挿入して上下。ボタン2つ(♭ / ♯ or − / +)。ヘルパ `shift_key_in_text(text, delta) -> String`(rust 側 or JS)。実装は Rust の文字列処理で `key:` 行を探し `Pitch` をパースして ±delta、書換。

### B-2. scale 選択

`<select>` に7旋法。選択で `scale:` 行を書換(無ければ挿入)。

### B-3. プリセット

- `chord_midi_web/presets.json`(または `src/presets.json`)を `include_str!` で埋め込み、`serde_json` でパース… だが web crate は `serde` 依存が無い。**依存を増やさない**方針。→ プリセットは Rust の静的配列 `const PRESETS: &[(&str, &str)]`(name, degrees)で持つ(実質同等、JSON 不要)。または最小手書きパーサ。**採用: Rust 静的配列**(`presets.rs`)。Issue の「JSON で持ち」は内部表現を JSON にこだわらず「データとして持つ」と解釈。
  - もし JSON 必須なら別途 `serde`/`serde_json` を web crate に追加する(要判断)。初期は静的配列。
- 各プリセットをボタン化。押下で textarea 末尾に degrees 文字列を append(改行区切り)。
- 定番例: `I | IV | V | I`、`IIm7 | V7 | IM7 | VI7`(ツーファイブ)、`I | V | VIm | IV`(王道) 等を数個。

---

## テスト戦略

- **core**: 構成音テーブル(`test_chord_semitones_table` を拡張)に旋法・変換・key解決を追加。
  - `Cdorian` 相当: `scale: dorian`+`key: C`+`III` → Eb(D#)。度数→実音を assert。
  - `blk(F)` → 半音 {0,2,6,10}(F基準)。
  - `s25(G7)` → 展開後 [Dm7, G7] の各構成音。
  - `sub(G7)` → Db7 {Db,F,Ab,B}。`dom(Dm7)` → D7 {D,F#,A,C}。
  - `resolve`: `key: C` + `V7` → G7 実音。`key: C`+`scale: dorian`+`bIII` 等。
- **web**: `cargo build --target wasm32-unknown-unknown` で通過(自動テスト無し)。`shift_key_in_text` は純関数なので core かユーティリティに置いて単体テスト可能なら追加。

## 実装フェーズ (別 PR、順に)

1. **フェーズ1 — コアモデル基盤**: `Key::Relative(Degree)`、`Scale` 旋法、`Scale: Copy`、`FromStr/Display`、`degree_parser` 改修、ディレクティブ(`Directive` + `directive_parser` + Display)、`into_pitch/into_degree(scale)` スレッド、`Ast::resolve`(transpose のみ、Transform 未対応でもコンパイル可)、`MidiExporter` で resolve 呼び出し。既存テスト更新(`test_transpose`、`ChordNode::relative`)。
2. **フェーズ2 — 変換関数**: `Transform` enum、`Node::Transform`、パーサ4分岐、Display、`Key::shift`、`Transform::expand`、`resolve` で展開、構成音テスト。
3. **フェーズ3 — web**: キー上下ボタン、scale 選択、プリセット(静的配列)+ append、`shift_key_in_text`。

各フェーズ TDD・spec/quality 二段レビュー・CI 緑を確認してから次へ。

## 影響ファイル

- core: `model/key.rs`, `model/scale.rs`, `model/ast.rs`, `model/chord.rs`(scale() 非変更確認), `model/transform.rs`(scale スレッド + resolve), `import/chord.rs`, `import/rechord.rs`, `import/sexp.rs`(`Key::Relative` 変更に追従), `export/rechord.rs`(Display), `export/midi.rs`(resolve 呼び出し)
- web: `chord_midi_web/src/main.rs`, `chord_midi_web/src/presets.rs`(新規)
- `import/sexp.rs` は `Key::Relative(u8)`→`Relative(Degree)` 変更にコンパイル追従が必要(既存 `degree_from_str` 等)。
