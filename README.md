# chord-midi
rechord format to midi file

- [ ] optimize voicing
- [ ] arpeggio

## CLI

web 版でできること（再生を除く）は CLI からも使える。

    # 変換
    cargo run -p chord_midi_cli -- -i song.txt -o song.midi --bpm 120
    cargo run -p chord_midi_cli -- -i song.txt -o song.rechord

    # 編集（出力先を省くと書き換えた進行を標準出力に返す）
    cargo run -p chord_midi_cli -- -i song.txt --transpose -2
    cargo run -p chord_midi_cli -- -i song.txt --scale minor

    # 表示
    cargo run -p chord_midi_cli -- -i song.txt --print-ast
    cargo run -p chord_midi_cli -- -i song.txt --fingering

    # プリセット進行（`-i` の代わりに使う）
    cargo run -p chord_midi_cli -- --list-presets
    cargo run -p chord_midi_cli -- --preset 小室進行 --fingering

`--transpose` と `--scale` は AST ではなく入力テキストを書き換える。
web 版の移調ボタン / スケール選択と同じ扱いなので、結果をそのまま
入力ファイルに書き戻せる。

`--fingering` は web 版のコード図を等幅文字で描く。指は `1` = 人差し指、
`2` = 中指、`3` = 薬指、`T` = 親指。行頭の `o` は開放弦、`x` はミュート。

進行プリセットは `chord_midi/src/presets.rs`（`docs/progressions.json` からの
生成物）を web と CLI で共有している。

## 逆引き（運指タブ）

web 版の「運指」タブでは、指板をクリックして作った押さえ方から、それが何の
コードで rechord 記法ではどう書くのかを引ける。

- 指板を押すとその弦の音が決まる。1弦につき1音で、同じところをもう一度押すと
  その弦はミュートになる
- 候補は確度の高い順に最大5件。バッジの「原典」は押さえ方データに同じ形がある
  もの、「計算」は鳴っている音から名前を出しただけのもの
- 「挿入」を押すと、その記法がエディタの入力末尾に足される

候補の記法を書いてもプレビュータブで同じ形にならないときは、カードにその旨が
出る。押さえ方データに無い形は記法で名指しできないため。

逆引きは CLI にはまだ無い。

## 押さえ方データ

ギターの押さえ方は `docs/easy_guiter_chords.json` が原本。
運指の導出と Rust テーブルの生成は次で行う。

    just gen-fingerings

`chord_midi/src/fingering/data.rs` は生成物なので手で編集しない。

## 関数

    arp(C)              アルペジオ
    blk(F)              ブラックアダーコード
    s25(G7)             ツーファイブ分割
    sub(G7)             裏コード化
    dom(Dm7)            ドミナント化

小節列を引数に取る関数もある。

    pow(C | E D)              パワーコード化（ルートと5thだけ鳴らす）
    oct(A | G#)               オクターブ奏法（ルートと1オクターブ上のルート）
    pat("o.o.oxox", C | E D)  リズムパターン
    pat("oxox", pow(C))       ネストできる

`pat` のパターンは `o` が打弦、`.` が休符、`x` がブラッシング。
文字数が1小節の分割数になり、1/2/4/8/16 のいずれかだけ書ける。

### 弦指定

コードの末尾に `@N` (N = 1〜6) を書くと「最低音をこの弦で取る」という
押さえ方への指示になる。音程には影響しない。`/on` より後ろに置く。

    pow(A@6 | D@5 | E@5)    6弦5fr / 5弦5fr / 5弦7fr のパワーコード
    A/E@6 |

指定した弦を最低音にする押さえ方がデータに無いときは「データなし」になる。
今のデータで最低音になり得るのは 6弦 / 5弦 / 4弦 だけなので、実質 `@6` と
`@5`（一部 `@4`）が使える。

`pow()` と `oct()` はそれぞれ専用の押さえ方（12キー × 6弦/5弦ルート）を持つ。
`oct()` は間の弦をミュートする。

    pow(A@6)    6弦5fr + 5弦7fr    (A2 + E3)
    oct(A@6)    6弦5fr + 4弦7fr    (A2 + A3)
    oct(A@5)    5弦0fr + 3弦2fr    (A2 + A3)
