# Part B: 編集で音が止まらない 実装計画

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`).

**Goal:** realtime エディタで再生中に textarea を編集しても音を止めない。compile(編集反映)と player の `src` 差し替えを分離し、再生中は差し替えを保留、停止時に反映する。

**Architecture:** `<midi-player>` の `start`/`stop` イベントを監視して再生状態を追跡。編集で作った最新 MIDI URL は、非再生時は即 `src` に反映、再生中は `pending` に退避して `stop` 時に反映。

**Tech Stack:** Yew 0.21 + web-sys。対象 `chord_midi_web/src/main.rs` のみ。native テスト不可 → `cargo build --target wasm32-unknown-unknown` で検証、手動確認。

参照 spec: `docs/superpowers/specs/2026-07-08-nested-functions-and-playback-continuity-design.md` の Part B。

---

## Task 1: 再生継続(pending/playing 分離 + イベント監視)

**Files:**
- Modify: `chord_midi_web/src/main.rs`

現状: `(input,bpm)` 依存 effect が compile→新 object URL→`url` state 更新→別 effect が `<midi-player src>` を毎回更新→再生停止。

変更後:
- `playing: Rc<RefCell<bool>>`(`use_mut_ref`)— 再生中フラグ。player の start/stop で更新。
- `pending: Rc<RefCell<Option<String>>>`(`use_mut_ref`)— 再生中に作られた未反映 URL。
- `url: use_state<String>` — 現在 player に反映済みの URL(従来どおり src-apply effect が反映)。
- compile effect: 非再生なら `url.set(new)`(即反映)、再生中なら `pending` に退避(旧 pending は revoke)。
- player NodeRef に start/stop リスナ登録(1回)。start→playing=true。stop→playing=false かつ pending あれば `url.set(pending)`。

- [ ] **Step 1: import 追加**

`chord_midi_web/src/main.rs` 冒頭の use に追加:
```rust
use web_sys::wasm_bindgen::closure::Closure;
use web_sys::wasm_bindgen::JsCast;
```
(既存 `use web_sys::{...}` はそのまま。`use yew::prelude::*;` に `use_mut_ref` 等含む。)

- [ ] **Step 2: 状態(ref)を追加**

`app()` の state 宣言部(`let url = use_state(String::new);` の近く)に追加:
```rust
    let playing = use_mut_ref(|| false);
    let pending = use_mut_ref(|| Option::<String>::new());
```

- [ ] **Step 3: compile effect を再生状態で分岐**

既存の `(input,bpm)` 依存 effect を変更。非再生時のみ即 `url.set`、再生中は pending 退避。effect のキャプチャに `playing`/`pending` を追加:

```rust
    {
        let result = result.clone();
        let error = error.clone();
        let url = url.clone();
        let playing = playing.clone();
        let pending = pending.clone();
        use_effect_with(((*input).clone(), *bpm), move |(input, bpm)| {
            if input.trim().is_empty() {
                error.set(String::new());
                result.set(String::new());
                // clear pending; clear url only if not playing
                if let Some(old) = pending.borrow_mut().take() {
                    let _ = web_sys::Url::revoke_object_url(&old);
                }
                if !*playing.borrow() {
                    let prev = (*url).clone();
                    if !prev.is_empty() {
                        let _ = web_sys::Url::revoke_object_url(&prev);
                    }
                    url.set(String::new());
                }
                return;
            }
            match compile(input, *bpm) {
                Ok((ast_str, bytes)) => {
                    error.set(String::new());
                    result.set(ast_str);
                    let blob = new_blob(&bytes, "audio/midi");
                    let object_url =
                        web_sys::Url::create_object_url_with_blob(&blob).unwrap();
                    if *playing.borrow() {
                        // stash; don't disturb the currently-playing src
                        if let Some(old) = pending.borrow_mut().replace(object_url) {
                            let _ = web_sys::Url::revoke_object_url(&old);
                        }
                    } else {
                        let prev = (*url).clone();
                        if !prev.is_empty() {
                            let _ = web_sys::Url::revoke_object_url(&prev);
                        }
                        url.set(object_url);
                    }
                }
                Err(e) => error.set(e),
            }
        });
    }
```

- [ ] **Step 4: player の start/stop リスナを登録(1回)**

既存 effect 群の後に、`player_ref` にイベントリスナを付ける effect を追加。`use_effect_with((), ...)`(初回マウント後1回)で NodeRef を要素にキャストし listener 登録。Closure は `.forget()` で保持(単一マウント SPA):

```rust
    {
        let player_ref = player_ref.clone();
        let playing = playing.clone();
        let pending = pending.clone();
        let url = url.clone();
        use_effect_with((), move |_| {
            if let Some(el) = player_ref.cast::<web_sys::EventTarget>() {
                // start -> playing = true
                {
                    let playing = playing.clone();
                    let cb = Closure::<dyn FnMut()>::new(move || {
                        *playing.borrow_mut() = true;
                    });
                    let _ = el.add_event_listener_with_callback(
                        "start",
                        cb.as_ref().unchecked_ref(),
                    );
                    cb.forget();
                }
                // stop -> playing = false; apply pending if any
                {
                    let playing = playing.clone();
                    let pending = pending.clone();
                    let url = url.clone();
                    let cb = Closure::<dyn FnMut()>::new(move || {
                        *playing.borrow_mut() = false;
                        if let Some(next) = pending.borrow_mut().take() {
                            let prev = (*url).clone();
                            if !prev.is_empty() && prev != next {
                                let _ = web_sys::Url::revoke_object_url(&prev);
                            }
                            url.set(next);
                        }
                    });
                    let _ = el.add_event_listener_with_callback(
                        "stop",
                        cb.as_ref().unchecked_ref(),
                    );
                    cb.forget();
                }
            }
            || ()
        });
    }
```

注: `use_effect_with((), ...)` は初回レンダ後に1回実行。Yew の effect はレンダ後に走るので `player_ref` は要素にマウント済み。`EventTarget` は `web_sys` の features に必要(`Element` は既に有効。`EventTarget` は Element の supertrait で通常有効だが、ビルドエラーが出たら `chord_midi_web/Cargo.toml` の web-sys features に `EventTarget` を追加)。

- [ ] **Step 5: src-apply effect と保存ボタンは現状維持**

`(url, loop_on)` 依存の src-apply effect はそのまま(url が変わったら src 反映)。保存ボタンの `<a href={(*url).clone()}>` もそのまま(反映済み url を指す。再生中の最新は次反映後にダウンロード可)。

- [ ] **Step 6: ビルド**

Run: `cd chord_midi_web && cargo build --target wasm32-unknown-unknown 2>&1 | tail -6`
Expected: 成功。想定エラーと対処:
- `Closure`/`JsCast` の import パス: `web_sys::wasm_bindgen::closure::Closure` / `web_sys::wasm_bindgen::JsCast`。解決しなければ `wasm_bindgen` を直接依存に追加せず、`web_sys::wasm_bindgen::*` を使う(web-sys が re-export)。
- `use_mut_ref` が見つからない → `yew::prelude::*` に含まれる(Yew 0.21)。含まれなければ `use yew::functional::use_mut_ref;`。
- `EventTarget` cast 不可 → web-sys features に `EventTarget` 追加(Cargo.toml、additive)。
- `add_event_listener_with_callback` が無い → `EventTarget` feature。

- [ ] **Step 7: 手動確認手順を README 的にコミットメッセージへ(自動テスト不可)**

手動確認(実装者はビルドまで、動作確認はユーザー):
1. `trunk serve` で開く。
2. 進行を入力し再生開始(ループ off)。
3. 再生中に textarea を編集 → **音が止まらない**ことを確認。
4. 再生終了後、再度再生 → 編集後の内容が反映されていることを確認。
5. 非再生時に編集 → すぐ次の再生に反映。

- [ ] **Step 8: コミット**

```bash
cd /Users/kmt/dev/chord-midi && git add chord_midi_web/src/main.rs
# Cargo.toml を変更した場合は追加
git commit -m "fix: keep playback going while editing (defer src swap until stop)"
```

---

## Self-Review メモ

- **Spec coverage:** Part B「再生中は止めない」。compile と src 反映を分離、再生中は pending 退避、stop で反映。
- **Type consistency:** `playing: Rc<RefCell<bool>>`、`pending: Rc<RefCell<Option<String>>>`、`url: UseStateHandle<String>`(反映済み)。
- **非破壊:** BPM/loop/key/scale/preset/保存/realtime parse・エラー表示は維持。src-apply effect 不変。
- **既知の限界(spec 記載):** ループ連続再生中に stop が発火しない実装なら、編集内容は手動停止まで反映されない。html-midi-player のループ時 stop 発火は実装時に要確認。発火しなくても「再生中は止めない」は満たす。
- **リーク対策:** pending 差し替え時・stop 反映時に旧 URL を revoke。
