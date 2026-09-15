# chord-midi
rechord format to midi file

- [ ] optimize voicing
- [ ] arpeggio

## CLI

web 版でできること（再生と逆引きを除く）は CLI からも使える。

    # 変換
    cargo run -p chord_midi_cli -- -i song.txt -o song.midi --bpm 120
    cargo run -p chord_midi_cli -- -i song.txt -o song.rechord

    # 編集（出力先を省くと書き換えた進行を標準出力に返す）
    cargo run -p chord_midi_cli -- -i song.txt --transpose -2
    cargo run -p chord_midi_cli -- -i song.txt --scale minor

    # 表示
    cargo run -p chord_midi_cli -- -i song.txt --print-ast            # 構文木 (JSON)
    cargo run -p chord_midi_cli -- -i song.txt --print-ast expanded   # 解決後 (JSON)
    cargo run -p chord_midi_cli -- -i song.txt --print-ast text       # rechord 記法に書き戻し
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

## 押さえ方データ

ギターの押さえ方は `docs/easy_guiter_chords.json` が原本。
運指の導出と Rust テーブルの生成は次で行う。

    just gen-fingerings

`chord_midi/src/fingering/data.rs` は生成物なので手で編集しない。

## キーとスケール

    key: C
    scale: dorian

`key:` と `scale:` は**それ以降の行に効く**。曲の途中で書き直すと、そこから
後ろだけがそのキーになる。

    key: C
    I | IV |
    key: D
    I | IV |     # ここは D のキーで解釈される

`--print-ast` と web 版の「解釈結果」で、この入れ子が JSON として見える。

## 定義と参照

`$名前 = ...` で進行やリズムに名前を付け、`$名前` で呼び出せる。
定義行は曲に小節を足さない。**定義行以外のすべてが曲**になる。

    key: C
    $eight = "o.o.o.o."
    $riff  = pow(C | E D)
    $A     = pat($eight, C | Am | F | G)
    $B     = $riff x2 | pat($eight, F | G)

    $A x2 | $B | $A

最後の1行が曲の本体で、これで 18 小節になる。

- 定義は行頭に書く。本体は1行
- 名前は英数字・`_`・日本語が使える（`$Aメロ`）
- 使う前に定義する。前方参照・循環参照・同じ名前の二重定義はエラー
- `$名前` は小節の位置、`pat` の第1引数、関数や `( )` の中に書ける
- 定義が小節より後ろにあると警告が出る（エラーではないので再生は止まらない）
- 定義行だけは行末コメントを書けない。コメントは別の行に置く

`$名前` は中身をその場に貼り付けるだけなので、定義を使って書いても
手で展開して書いても同じ MIDI になる。

### キーは使う位置で決まる

度数表記のリフを定義しておくと、キーを変えて使い回せる。度数が実音に変わるのは
定義した位置ではなく**使う位置**なので、同じ `$a` がキーごとに違う音になる。

    key: C
    $a = I | IV |
    $a          # C | F |
    key: D
    $a          # D | G |

### 繰り返し

`xN` は直前の1項目を N 回繰り返す。`N` は 2〜32 で、範囲外はパースエラー。

    C | Am x3 |          # C, Am, Am, Am （直前の小節だけ）
    $A x2 | $B           # A A B

`( )` で範囲を明示できる。

    (C | Am | F | G) x4  # 4小節を4回で 16 小節

展開後の小節数には 4096 小節の上限がある。`$b = $a x32` のような入れ子は
乗算的に増えるので、打ち間違い1つで膨れ上がるのを防いでいる。

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
