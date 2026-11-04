// Cargo test runner for wasm32-wasip1 using Node's built-in WASI.
// Lets the golden/property tests run as WebAssembly to prove native == WASM output.
// Usage (set by CI): CARGO_TARGET_WASM32_WASIP1_RUNNER="node scripts/wasi-runner.mjs"
import { readFile } from "node:fs/promises";
import { WASI } from "node:wasi";
import process from "node:process";

const [wasmPath, ...args] = process.argv.slice(2);
const wasi = new WASI({
  version: "preview1",
  args: [wasmPath, ...args],
  env: process.env,
  preopens: { "/": "/" }, // tests read golden files by absolute path
  returnOnExit: true,
});
const module = await WebAssembly.compile(await readFile(wasmPath));
const instance = await WebAssembly.instantiate(module, wasi.getImportObject());
process.exitCode = wasi.start(instance);
