import { existsSync, mkdirSync } from "node:fs";
import { spawnSync } from "node:child_process";

function run(command, args, env = process.env) {
  const result = spawnSync(command, args, { stdio: "inherit", env });
  if (result.error || result.status !== 0) {
    throw result.error || new Error(`${command} ${args.join(" ")} failed`);
  }
}

function capture(command, args) {
  const result = spawnSync(command, args, { encoding: "utf8" });
  if (result.error || result.status !== 0) {
    throw result.error || new Error(`${command} ${args.join(" ")} failed`);
  }
  return result.stdout.trim();
}

const rustc = capture("rustup", ["which", "--toolchain", "1.98.1", "rustc"]);
const rustdoc = capture("rustup", ["which", "--toolchain", "1.98.1", "rustdoc"]);
run("rustup", [
  "run",
  "1.98.1",
  "cargo",
  "build",
  "--release",
  "--target",
  "wasm32-unknown-unknown",
], { ...process.env, RUSTC: rustc, RUSTDOC: rustdoc });

const bindgen = ".tools/bin/wasm-bindgen";
if (!existsSync(bindgen)) {
  throw new Error("Missing .tools/bin/wasm-bindgen. Install the pinned CLI with: rustup run 1.98.1 cargo install wasm-bindgen-cli --version 0.2.128 --locked --root .tools");
}
const bindgenVersion = capture(bindgen, ["--version"]);
if (!bindgenVersion.endsWith("0.2.128")) {
  throw new Error(`Expected wasm-bindgen-cli 0.2.128, found ${bindgenVersion}. Reinstall it with: rustup run 1.98.1 cargo install wasm-bindgen-cli --version 0.2.128 --locked --root .tools`);
}
mkdirSync("web/pkg", { recursive: true });
run(bindgen, [
  "--target",
  "web",
  "--out-dir",
  "web/pkg",
  "target/wasm32-unknown-unknown/release/qkmj_browser.wasm",
]);
