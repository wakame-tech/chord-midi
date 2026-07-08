# 関数の入れ子・小節跨ぎ引数・再生継続 設計

日付: 2026-07-08

## 目的

3つの課題を解決する:

1. **関数の入れ子** — `arp(s25(C))` のように関数の引数に別の関数を書けない。
2. **小節を跨いだ引数** — `arp(C | B \n C)` のように関数の引数に複数コード/小節を書けない。
3. **編集で音が止まる** — realtime エディタで textarea を編集するたび MIDI が差し替わり再生が停止する。

1・2 は関数(arp/s25/sub/dom/blk)の引数モデルを「単一 ChordNode」から「サブ進行(小節列)」へ拡張する core 再設計。3 は web の再生継続 UX。

## 確定した意味論(ユーザー承認済み)

- 複数コードを包む関数は **コードごとに適用 (map)**。`sub(C | G)` = `sub(C) | sub(G)`。
- 関数の引数の `|`/改行は **小節を保持**。関数は **小節を跨げる**。`arp(C | B\nC)` は 3 小節に展開し各々 arp。
- 入れ子は **inside-out**(内側を先に展開)。`arp(s25(C))` = s25 展開 `Dm7 G7` を各 arp。`s25(arp(C))` = arp マーカーを保持したまま s25 適用。
- 編集で **再生中は止めない**。再生中の編集は次の停止/ループ/手動再生で反映。

## スコープ外 (YAGNI)

- 関数への複数引数(`f(a, b)`)。引数は単一のサブ進行のみ。
- 再生位置のシームレスなホットスワップ(再生を止めずに即差し替え)。「次の停止/ループで反映」で十分。
- コンスタントストラクチャー等の新変換(既存 issue #12 スコープのまま)。

---

## Part A. 関数引数 = サブ進行(core 再設計)

### A-1. 文法

```
f( <サブ進行> )      f ∈ { arp, s25, sub, dom, blk }
```

サブ進行 = 小節列。空白区切りのノード、`|`/改行で小節区切り、入れ子の `f(...)` を含みうる。括弧の対応で範囲が決まる。

例:
```
arp(C)               # 単一コード(従来どおり)
arp(C | Am)          # 2小節、各 arp
arp(C B C)           # 1小節3コード、各 arp
arp(C | B\nC)        # 3小節、各 arp
arp(s25(C))          # 入れ子: s25(C)=Dm7 G7 を各 arp
sub(C | G)           # 各コードに sub → Gb7 | Db7
```

### A-2. AST

現状の `Node::Arpeggio(ChordNode)` と `Node::Transform(Transform, ChordNode)` を廃し、統合する:

```rust
// ast.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Func { Arp, S25, Sub, Dom, Blk }

pub enum Node {
    Chord(ChordNode),
    Apply(Func, Box<Ast>),   // Box は Ast::Score(サブ小節列)
    Arpeggio(ChordNode),     // 展開後の「アルペジオされたコード」マーカー(MIDIまで残る)
    Rest,
    Sustain,
    Repeat,
}
```

- **`Apply(Func, Box<Ast>)`**: パース結果。Box は常に `Ast::Score(Vec<Box<Ast>>)`(小節列)。単一コード `arp(C)` も `Apply(Arp, Score[Measure[Chord(C)]])`。
- **`Arpeggio(ChordNode)`**: `Apply(Arp, ...)` を展開したときに各コードに付く「これはアルペジオ」マーカー。resolve 後も残り、MIDI interpret で順次発音される(現行の Arpeggio 挙動を流用)。harmonic 関数(s25 等)は resolve で完全展開されるため `Apply` は残らない。

注: `Transform` enum は `Func` に統合され不要になる(削除)。

### A-3. パーサ

`import/chord.rs` の関数分岐を、サブ進行を再帰パースする形に変更:

```
apply_parser := ("arp("|"s25("|"sub("|"dom("|"blk(") <subprogression> ")"
```

サブ進行のパースは `rechord.rs` の小節列パーサ(`many1(measure_parser)` 相当)を、閉じ括弧 `)` で停止するよう再利用する。実装案:

- `rechord.rs` に `pub(crate) fn subprogression_parser(s) -> IResult<Span, Vec<Box<Ast>>>` を用意(measure を `)` の手前まで繰り返す)。measure_parser の measure_sep は `|`/改行/eof に加え、閉じ括弧の直前を許容する必要がある(サブ進行末尾の measure は `)` で終わる)。
- `import/chord.rs` の `node_parser` の関数分岐を `map(tuple((func_tag, subprogression, tag(")"))), |(f, sub)| Node::Apply(f, Box::new(Ast::Score(sub))))` に。
- 入れ子は `node_parser` が再帰的に `Apply` を返すことで自然に対応(サブ進行内のノードも `node_parser`)。
- 括弧バランス: nom のパーサは `)` で measure を打ち切る。ネストは各 `apply_parser` が自分の `)` を食う。

循環参照(node_parser ↔ subprogression_parser ↔ measure_parser)は Rust の関数分割で解決。`chord.rs` と `rechord.rs` の相互 `pub(crate)` 参照。

### A-4. 展開ロジック

resolve 時に関数を展開する(現行は MIDI interpret 時展開だが、小節跨ぎ・splice のため resolve に移す)。

`ast.rs`(または新規 `apply.rs`)に:

```rust
impl Node {
    /// Apply を展開して 0..n 個の「小節」または「同一小節内ノード列」に。
    /// harmonic 関数はコードを置換、arp はコードを Arpeggio マーカーに。
    /// 入れ子は inside-out。
    fn expand_apply(self, tonic: Pitch, scale: Scale) -> ExpandResult { ... }
}
```

方針:
- **per-chord map**: サブ進行の各 `Node::Chord(c)` に対し関数を適用。
  - harmonic (S25/Sub/Dom/Blk): `c` → 1..n 個の `Node::Chord`(既存 issue #12 の変換定義を流用: s25(X)=[（X+7）m7, X], sub/dom/blk は 1 個)。
  - Arp: `c` → `Node::Arpeggio(c)`。
- **Arpeggio マーカーへの harmonic 適用**: サブ内に `Node::Arpeggio(c)` があれば(入れ子 `s25(arp(...))` 等)、内側 `c` に harmonic を適用し結果を再び `Arpeggio` で包む。
- **入れ子 Apply**: サブ内の `Node::Apply(...)` は先に再帰展開(inside-out)してから外側を適用。
- **Rest/Sustain/Repeat**: そのまま通す。
- **小節構造**: サブが複数 measure を持つなら、展開結果も複数 measure。単一 measure なら親小節内にインライン展開。

**splice(親への差し込み):** resolve が Score を走査し、ある Measure 内の `Node::Apply` を展開する:
- Apply のサブが単一小節 → その小節内のノード列に置換(インライン)。
- Apply のサブが複数小節 → その Apply を含む Measure を、展開された複数 Measure で置換(Score レベルで splice)。この場合、当該 Apply は小節の唯一のノードである前提(複数小節に展開される Apply が他ノードと同居する場合は、前後を別小節に分割 or エラー。初期実装は「唯一のノード」を前提にし、混在時は素直に前後ノードを別小節化)。

### A-5. resolve / MIDI / Display の変更

- **`Ast::resolve`**: 既存の「into_pitch → Directive/Comment 除去」に加え、**Apply 展開 + splice** を追加。順序: (1) into_pitch(サブ内コードも transpose)、(2) Apply 展開(inside-out, splice)、(3) Directive/Comment 除去。展開後は Chord/Arpeggio/Rest/Sustain/Repeat のみ。
- **MIDI `interpret_node`**: `Node::Apply` は resolve で消えるので interpret では扱わない(exhaustive match のため `Node::Apply(..) => unreachable/expand` のアームは必要 → 防御的に resolve 済み前提でエラー or その場展開)。`Node::Arpeggio` は現行どおり順次発音(維持)。現行の `Node::Transform` interpret アームは削除。
- **`into_pitch`/`into_degree`**: `Node::Apply(f, sub)` は サブ Ast を再帰 transpose。`Node::Arpeggio(c)` は現行どおり内側 transpose。`Node::Transform` アームは削除。
- **Display (`export/rechord.rs`)**: `Node::Apply(f, sub)` → `f(<sub の Display>)`。サブ Ast の Display はサブ小節列を `|` 区切りで。`Node::Arpeggio` の Display はマーカーなので `arp(<c>)`(展開前の記法に近い形。ただし通常 Display 対象はパース直後の Apply なので Arpeggio マーカーの Display はテスト用途)。

### A-6. テスト(core)

- パーサ: `arp(C)`, `arp(C | Am)`, `arp(C B C)`, `arp(s25(C))`, `sub(C | G)`, `s25(arp(C))` が全消費でパースでき `Node::Apply` を返す。
- 展開(resolve 後の構成音/構造):
  - `arp(C | Am)` → 2小節、各 Arpeggio。
  - `sub(C | G)` → 2小節 `Gb7` / `Db7`(構成音 {0,4,7,10} 各ルート)。
  - `arp(s25(C))` → 1小節 [Arpeggio(Dm7), Arpeggio(G7)]。
  - `s25(arp(C))` → 1小節 [Arpeggio(Dm7), Arpeggio(C)]。
  - MIDI: `key: C\narp(I | IV)` が note を生成、arp が順次発音。
- Display roundtrip: `arp(C | Am)` → parse → Display → 同一(または再パースで同 AST)。

---

## Part B. 編集で音が止まらない(web)

### B-1. 現状の問題

`chord_midi_web/src/main.rs`: `(input, bpm)` 依存の effect が毎編集で `compile` → 新 object URL を作り、別 effect が `<midi-player src>` を更新。src 変更で html-midi-player がリロードし再生が停止する。

### B-2. 方針

compile(編集反映)と player の src 更新(音の差し替え)を分離し、**再生中は src を差し替えない**:

- 編集ごとに compile して AST/エラー/最新 MIDI URL を更新するのは維持。ただし最新 URL は `pending_url` として保持。
- `<midi-player>` の再生状態を監視(NodeRef + `start`/`stop` イベント、または要素の `playing` プロパティ)。
- **再生していない** ときのみ `pending_url` を `src` に反映(即時)。
- **再生中** に編集された場合は反映を保留し、`stop` イベント(曲末/手動停止/ループ境界)で `pending_url` を `src` に適用。→ 音は途切れず、新内容は次の再生/ループから。

### B-3. 実装(`chord_midi_web/src/main.rs`)

- 状態追加: `pending_url`(最新 compile 結果の object URL)、`playing`(bool、player イベントで更新)。
- compile effect: `url`(=即時反映用)ではなく `pending_url` を更新。
- player NodeRef に `start`/`stop` の event listener を登録(`use_effect` で一度だけ、`web_sys::EventTarget::add_event_listener_with_callback`。Closure を保持):
  - `start` → `playing=true`。
  - `stop` → `playing=false`、かつ `pending_url` があれば `src` に適用(NodeRef 経由 `set_attribute("src", ...)`)。
- src 反映 effect: `(pending_url, playing)` を監視し、`!playing` なら `pending_url` を player の `src` に反映(NodeRef)。playing なら何もしない(stop 時に反映)。
- object URL リーク対策: 差し替え時に前の URL を `revoke_object_url`(再生中に使っている URL は解放しない — 適用時に旧を解放)。

注: html-midi-player のイベント名は `start`/`stop`/`load`/`note`。`stop` は再生終了で発火。ループ属性使用時にループ境界で `stop`→`start` が発火するかは実装時に確認し、しなければ「手動停止/曲末で反映」に留める(仕様: 再生中は止めない、が満たせればよい)。

### B-4. テスト

- web は native テスト不可 → `cargo build --target wasm32-unknown-unknown` でビルド確認。ロジックのうち純粋な部分(URL 選択の条件)は最小限なので目視レビュー。手動確認: 再生中に textarea 編集 → 音が止まらない、停止後/次ループで新内容反映。

---

## 実装フェーズ(別 PR、順に)

1. **フェーズ1 — Part B(再生継続)**: web のみ。小さく体感が大きい。`pending_url`/`playing` 分離、player イベント監視。
2. **フェーズ2 — Part A(関数再設計)**: core 大改修。`Node::Apply(Func, Box<Ast>)`、サブ進行パーサ、resolve での展開+splice、MIDI/Display/into_pitch 追従、`Transform` 廃止。

各フェーズ TDD・spec/quality 二段レビュー・CI 緑を確認してから次へ。

## 影響ファイル

- **Part A (core)**: `model/ast.rs`(Func/Apply, Arpeggio マーカー, Transform 廃止), `import/chord.rs`(apply_parser), `import/rechord.rs`(subprogression_parser), `model/transform.rs`(resolve に展開+splice, into_pitch/into_degree の Apply 対応), `export/midi.rs`(interpret の Apply 除去/Arpeggio 維持), `export/rechord.rs`(Display)。
- **Part B (web)**: `chord_midi_web/src/main.rs`。
- 既存の `Node::Transform` を参照する全 match 箇所を `Node::Apply` へ移行。
