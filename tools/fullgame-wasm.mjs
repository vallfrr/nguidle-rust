// Runs the engine's full-game script as WebAssembly (the build the web port uses) under Node's WASI.
// Usage: node tools/fullgame-wasm.mjs <clr.wasm> <prepared folder> <work folder> [game hours]
import { WASI } from "node:wasi";
import { mkdirSync, readFileSync } from "node:fs";

const [wasmPath, data, work, hours = "0.05"] = process.argv.slice(2);
if (!wasmPath || !data || !work) {
  console.error("usage: node tools/fullgame-wasm.mjs <clr.wasm> <prepared folder> <work folder> [game hours]");
  process.exit(2);
}
mkdirSync(work, { recursive: true });
const wasi = new WASI({
  args: ["clr", "/data/Assembly-CSharp.dll", "fullgame", "/data", "/work/report.txt", hours],
  env: { NGU_FULLGAME_WORK: "/work/run" },
  preopens: { "/data": data, "/work": work },
  returnOnExit: true,
});
const module = await WebAssembly.compile(readFileSync(wasmPath));
const instance = await WebAssembly.instantiate(module, { wasi_snapshot_preview1: wasi.wasiImport });
const start = Date.now();
const code = wasi.start(instance);
console.log(`exit ${code} in ${((Date.now() - start) / 1000).toFixed(1)} s`);
process.exit(code);
