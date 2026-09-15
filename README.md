# chord-midi
rechord format to midi file

- [ ] optimize voicing
- [ ] arpeggio

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
    pat("o.o.oxox", C | E D)  リズムパターン
    pat("oxox", pow(C))       ネストできる

`pat` のパターンは `o` が打弦、`.` が休符、`x` がブラッシング。
文字数が1小節の分割数になり、1/2/4/8/16 のいずれかだけ書ける。
