#!/usr/bin/env python3
"""docs/easy_guiter_chords.json から運指を導出し、Rust のテーブルと docs を生成する。

使い方:
    just gen-fingerings      (= python3 scripts/gen_fingerings.py)

出力:
    chord_midi/src/fingering/data.rs   Rust の const テーブル
    docs/easy_guiter_chords.json       fingers フィールドを追記
    docs/easy_guiter_chords.md         運指の列を追記
"""
import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parent.parent
JSON_PATH = ROOT / "docs" / "easy_guiter_chords.json"
MD_PATH = ROOT / "docs" / "easy_guiter_chords.md"
RS_PATH = ROOT / "chord_midi" / "src" / "fingering" / "data.rs"
POWER_PATH = ROOT / "docs" / "power_chords.json"

PC = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"]
# 標準チューニングの開放弦のピッチクラス。6弦→1弦
OPEN_STRINGS = [4, 9, 2, 7, 11, 4]
STRING_LABEL = ["6弦", "5弦", "4弦", "3弦", "2弦", "1弦"]
FINGER_RS = {
    "index": "Finger::Index",
    "middle": "Finger::Middle",
    "ring": "Finger::Ring",
    "thumb": "Finger::Thumb",
}
FINGER_JP = {"index": "人", "middle": "中", "ring": "薬", "thumb": "親"}


def assign_fingers(frets):
    """frets は 6弦→1弦。None=ミュート、0=開放。同じ並びの指名リストを返す。

    規則:
      1. 押さえる音をフレット昇順 → 弦番号 6弦→1弦 の順に並べる
      2. 4音のときは6弦を親指にし、残り3音を 人 → 中 → 薬
      3. 2〜3音のときは 人 → 中 → 薬
      4. 1音だけのときは中指（人差し指はミュート用に空ける）
    """
    fingers = [None] * 6
    pressed = [(i, f) for i, f in enumerate(frets) if f not in (None, 0)]
    if not pressed:
        return fingers
    if len(pressed) > 4:
        raise ValueError(f"too many pressed strings: {frets}")
    if len(pressed) == 1:
        fingers[pressed[0][0]] = "middle"
        return fingers
    rest = pressed
    if len(pressed) == 4:
        if pressed[0][0] != 0:
            raise ValueError(f"4 pressed strings without the 6th string: {frets}")
        fingers[0] = "thumb"
        rest = pressed[1:]
    ordered = sorted(rest, key=lambda p: (p[1], p[0]))
    for finger, (string, _) in zip(["index", "middle", "ring"], ordered):
        fingers[string] = finger
    return fingers


def pitch_classes(frets):
    pcs = 0
    for i, f in enumerate(frets):
        if f is not None:
            pcs |= 1 << ((OPEN_STRINGS[i] + f) % 12)
    return pcs


def bass(frets):
    for i, f in enumerate(frets):
        if f is not None:
            return (OPEN_STRINGS[i] + f) % 12
    raise ValueError(f"no sounding string: {frets}")


def notes(frets):
    return [PC[(OPEN_STRINGS[i] + f) % 12] for i, f in enumerate(frets) if f is not None]


def detail(frets):
    parts = []
    for i, f in enumerate(frets):
        if f is None:
            parts.append(f"{STRING_LABEL[i]}ミュート")
        elif f == 0:
            parts.append(f"{STRING_LABEL[i]}開放")
        else:
            parts.append(f"{STRING_LABEL[i]}{f}F")
    return " / ".join(parts)


def finger_detail(frets, fingers):
    parts = [
        f"{FINGER_JP[fingers[i]]}={STRING_LABEL[i]}{f}F"
        for i, f in enumerate(frets)
        if f not in (None, 0)
    ]
    return " / ".join(parts) if parts else "—"


def lowest_string(frets):
    for i, f in enumerate(frets):
        if f is not None:
            return i
    raise ValueError(f"no sounding string: {frets}")


def assign_power_fingers(frets):
    """可動パワーコードは ルート=人差し指 / 5th=薬指。
    開放弦がルートの形は押さえるのが5thだけなので薬指1本になる"""
    fingers = [None] * 6
    pressed = [i for i, f in enumerate(frets) if f not in (None, 0)]
    if len(pressed) == 1:
        fingers[pressed[0]] = "ring"
    elif len(pressed) == 2:
        fingers[pressed[0]] = "index"
        fingers[pressed[1]] = "ring"
    else:
        raise ValueError(f"unexpected power chord shape: {frets}")
    return fingers


def opt_u8(v):
    return "None" if v is None else f"Some({v})"


def opt_finger(v):
    return "None" if v is None else f"Some({FINGER_RS[v]})"


def build_markdown(data):
    md = []
    md.append("# 超簡単コード一覧（押さえ方）\n")
    md.append("出典: <https://daichan-easyguitar.com/easychord-list/>\n")
    md.append(
        "> このファイルは `scripts/gen_fingerings.py` が "
        "`easy_guiter_chords.json` から生成する。手で編集しない。\n"
    )
    md.append("## 条件\n")
    md.append("- バレーコードなし\n- 指1~3本で抑えられる\n- 小指は使わない\n")
    md.append("## 記法\n")
    md.append(
        "- 押さえ方は **6弦→1弦** の順で表記（例: `x32010`）\n"
        "- 数字はフレット番号、`0` は開放弦、`x` はミュート\n"
        "- チューニングは標準 (E A D G B E)\n"
        "- 運指は 人=人差し指 / 中=中指 / 薬=薬指 / 親=親指。"
        "原典に指の指定が無いため規則で導出している\n"
    )
    md.append(
        "同じコード名が複数ある場合は、原典に載っている複数の押さえ方"
        "（難易度・響きの違い）を並べたもの。\n"
    )
    md.append(
        "HTML 表示用の同じデータは "
        "[`easy_guiter_chords.json`](./easy_guiter_chords.json) にある。\n"
    )
    for group in data["groups"]:
        md.append(f"## {group['name']}\n")
        md.append("| コード | 押さえ方 | 構成音 | 運指 | 詳細 |")
        md.append("| --- | --- | --- | --- | --- |")
        for c in group["chords"]:
            fr = c["frets"]
            md.append(
                f"| {c['name']} | `{c['tab']}` | {', '.join(notes(fr))} "
                f"| {finger_detail(fr, c['fingers'])} | {detail(fr)} |"
            )
        md.append("")
    md.append("## 参考文献\n")
    md.append(
        "- <https://daichan-easyguitar.com/what-is-easychord/>\n"
        "- <https://daichan-easyguitar.com/easychord-list/>\n"
    )
    return "\n".join(md)


def build_rust(data, powers):
    lines = [
        "// @generated by scripts/gen_fingerings.py",
        "// from docs/easy_guiter_chords.json and docs/power_chords.json",
        "// 手で編集しない。`just gen-fingerings` で再生成する。",
        "use super::{Finger, Fingering};",
        "",
        "pub static FINGERINGS: &[Fingering] = &[",
    ]

    def entry(name, group, frets, fingers, is_power):
        b = bass(frets)
        lines.append("    Fingering {")
        lines.append(f'        name: "{name}",')
        lines.append(f'        group: "{group}",')
        lines.append("        frets: [" + ", ".join(opt_u8(v) for v in frets) + "],")
        lines.append("        fingers: [" + ", ".join(opt_finger(v) for v in fingers) + "],")
        lines.append(
            f"        pitch_classes: 0b{pitch_classes(frets):012b}, // {', '.join(notes(frets))}"
        )
        lines.append(f"        bass: {b}, // {PC[b]}")
        lines.append(f"        lowest_string: {lowest_string(frets)},")
        lines.append(f"        is_power: {'true' if is_power else 'false'},")
        lines.append("    },")

    for group in data["groups"]:
        for c in group["chords"]:
            entry(c["name"], group["name"], c["frets"], c["fingers"], False)
    for shape in powers["shapes"]:
        entry(
            f"{shape['root']}5",
            f"{shape['string']}弦ルート",
            shape["frets"],
            assign_power_fingers(shape["frets"]),
            True,
        )

    lines.append("];")
    return "\n".join(lines) + "\n"


def main():
    data = json.loads(JSON_PATH.read_text(encoding="utf-8"))
    powers = json.loads(POWER_PATH.read_text(encoding="utf-8"))

    for group in data["groups"]:
        for chord in group["chords"]:
            chord["fingers"] = assign_fingers(chord["frets"])

    data["fingersNotation"] = (
        "fingers は frets と同じ 6弦→1弦 の順。"
        "index=人差し指 / middle=中指 / ring=薬指 / thumb=親指、null は押さえない弦。"
    )

    JSON_PATH.write_text(
        json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    MD_PATH.write_text(build_markdown(data), encoding="utf-8")
    RS_PATH.parent.mkdir(parents=True, exist_ok=True)
    RS_PATH.write_text(build_rust(data, powers), encoding="utf-8")

    total = sum(len(g["chords"]) for g in data["groups"]) + len(powers["shapes"])
    print(f"wrote {total} fingerings")


if __name__ == "__main__":
    main()
