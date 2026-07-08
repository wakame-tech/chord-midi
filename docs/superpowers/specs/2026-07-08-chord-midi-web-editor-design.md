# chord_midi_web エディタ化 設計

日付: 2026-07-08

## 目的

`chord_midi_web` を、コード表記・ディグリー表記のコード進行を書きながらリアルタイムに再生・MIDI出力できるエディタにする。現状は textarea + submit ボタンで、押すたびに parse → AST 表示 → MIDI blob 生成 → `html-midi-player` で再生する最小構成。

ユーザーが求めるのは **入力補助** と **再生強化**。ビジュアル編集・保存/共有は今回スコープ外。

## スコープ

1. リアルタイム parse + エラー表示（web、軽量実装）
2. BPM スライダー（web + 既存 exporter）
3. ループ再生（web、`midi-player` の `loop` 属性）
4. `arp(...)` アルペジオ記法（core ライブラリ: AST + parser + exporter）

明示的に**やらないこと**（YAGNI）:
- 楽器（GM voice）選択
- ビジュアル編集（ピアノロール/コード表からの編集）
- 保存・URL 共有・プロジェクト管理
- シンタックスハイライトの色付け、コード補完（realtime parse + エラー下線までで止める）
- アルペジオの down / updown 方向、per-chord 速度指定

## A. アルペジオ記法 `arp(...)`（core ライブラリ）

### 記法

関数呼び出し風のラッパー。単一コードを包む。

```
arp(C)        # C メジャーを昇順アルペジオ
arp(Cm7)      # 内部で既存 chord_node_parser が modifier を解釈
arp(C/E)      # on ベースも可（内部 chord_node_parser 任せ）
```

block（同時発音）が既定、`arp(...)` で囲んだノードのみ順次発音。

### 変更点

| 層 | ファイル | 変更 |
|---|---|---|
| AST | `chord_midi/src/model/ast.rs` | `enum Node` に `Arpeggio(ChordNode)` バリアント追加 |
| parser | `chord_midi/src/import/chord.rs` | `node_parser` の `alt` に `arp(...)` 分岐追加: `map(delimited(tag("arp("), chord_node_parser, tag(")")), Node::Arpeggio)`。既存 chord 分岐より前に置く（`arp` を先に食う）|
| exporter | `chord_midi/src/export/midi.rs` | `interpret_node` に `Node::Arpeggio` 分岐。pending sustain/rest を flush 後、chord 構成音を `dur` で等分し1音ずつ昇順に `Note` 列として `self.notes` に push |
| text export | `chord_midi/src/export/rechord.rs` | `Display for Node` に `Arpeggio(c) => write!(f, "arp({})", c)` |

### exporter 挙動の詳細

- block コードは従来通り: `pre` に chord をため、`sustain` を積み、次ノードで flush（全音同時に note-on / dur 後 note-off）。
- `arp(chord)` は独自に発音列を作る:
  - `into_note_numbers(&chord)` で音番号列（昇順）を取得。
  - 総尺 = `dur`（`measure_unit_size` 由来のノード尺）。
  - 発音数 `k = min(音数, dur)` に制限（`.max(1)` で下限保証）。`dur / k` を各音に割り当て、端数は最後の音に寄せる。これにより総尺は常に `dur` に一致し、密な小節（`dur < 音数`）でも持ち尺超過で後続ノードがずれない。`dur < 音数` のときは上位の音を落とす。
  - 各音を単音の `Note`（`Some(vec![n])`, 分割 duration）として昇順に push。
- arpeggio ノードは `pre`/`sustain` を汚さない（`=` サステインの対象にしない）。直後の `=` は arpeggio を延長せず、それ以前の block chord に作用する既存挙動を維持。

### テスト（core, `cargo test -p chord_midi`）

- parser: `arp(C)`, `arp(Cm7)`, `arp(C/E)` が残り空文字列で parse できる（既存 `chord_node_parser` テストと同形式）。
- AST 往復: `arp(C)` を parse → `Display` → `arp(C)` に戻る。
- exporter: `arp(C)` が単音 note を複数（3音）生成、block `C` は同時 note を生成、という差を note 数・duration で検証。

## B. web エディタ（`chord_midi_web/src/main.rs`）

### 構成

- **ヘルパ抽出**: `compile(input: &str, bpm: u8) -> Result<(String, Vec<u8>)>`
  - `RechordImporter.import(input)?` → `ast`
  - `format!("{}", ast)` を AST 文字列に
  - `MidiExporter { bpm }.export(&mut BufWriter, ast)?` で MIDI バイト列
  - `(ast_str, midi_bytes)` を返す
- **状態**（`use_state`）: `input`, `bpm`, `loop_enabled`, `result`(AST文字列), `error`, `url`(object URL)
- **再計算**: `input` か `bpm` が変わったら `compile` を呼び直し、`result`/`error`/`url` を更新。実装は「変更ハンドラ内で compile 呼び再生成」方式（`use_effect_with((input, bpm), ...)` でも可）。debounce なし（parse + MIDI 生成は軽量）。
- **object URL リーク対策**: 新 URL を作る前に前の URL を `Url::revoke_object_url` で解放。

### UI（Yew `html!`）

- `<textarea id="input" oninput=...>` — 入力で即再計算。**submit ボタンは廃止**。
- BPM: `<input type="range" min=40 max=240 value={bpm}>` + 現在値ラベル。変更で再計算。既定 120。
- ループ: `<input type="checkbox">` → `loop_enabled`。true のとき `<midi-player loop=...>` 属性付与。
- エラー: `<p style="color:red">` に `anyhow` メッセージ表示（位置情報は今回なし）。
- `<midi-player src={url} loop=... visualizer="#visualizer">` + `<midi-visualizer type="piano-roll">`（既存踏襲）。

### エラーの扱い

`import()` は `Result<Ast>`（anyhow）。realtime で入力途中は parse 失敗が普通に起きる → エラー時は `error` に文言をセットし、`url`/`result` は**前回値を保持**（再生を壊さない）。

## データフロー

```
textarea oninput / bpm slider
        │
        ▼
  compile(input, bpm)
        │  Ok((ast_str, midi_bytes))         Err(e)
        ├───────────────────────────┐        │
        ▼                           ▼        ▼
  result = ast_str          Blob→object URL  error = e.to_string()
                            (revoke 旧URL)    (result/url 据え置き)
        │                           │
        ▼                           ▼
   <p> 表示                  <midi-player src=url loop=…>
```

## ビルド / 検証

- core: `cargo test -p chord_midi`
- web: `cd chord_midi_web && trunk build`（既存 CI と同じ経路。`trunk serve` で手動確認）
- CLI は `arp(...)` 記法が新 Node を通るだけで動作継続（回帰確認: 既存 sexp/rechord テスト）。

## 影響範囲まとめ

- core: `model/ast.rs`, `import/chord.rs`, `export/midi.rs`, `export/rechord.rs`（+テスト）
- web: `chord_midi_web/src/main.rs`
- `Node` に網羅 match しているコードは全て新バリアント対応が必要（`export/midi.rs` interpret_node, `export/rechord.rs` Display, sexp import/export に Node match あれば）。実装時に `Node` の全 match 箇所を洗い出して埋める。
```