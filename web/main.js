import init, { as_degree } from "chord_midi_wasm.js?url";
await init();

document.getElementById("convert").addEventListener("click", () => {
    const score = document.getElementById("score").value;
    const res = as_degree(score);
    document.getElementById("result").innerText = res;
});
