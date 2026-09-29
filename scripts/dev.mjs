import { execFileSync, spawn } from "node:child_process";
import { join } from "node:path";

const children = new Set();
let stopping = false;

function terminate(child, signal) {
  if (!child.pid) return;
  try {
    if (process.platform === "win32") child.kill(signal);
    else process.kill(-child.pid, signal);
  } catch (error) {
    if (error.code !== "ESRCH") throw error;
  }
}

function stop(code = 0) {
  if (stopping) return;
  stopping = true;
  process.exitCode = code;
  for (const child of children) terminate(child, "SIGTERM");
  const timer = setTimeout(() => {
    for (const child of children) terminate(child, "SIGKILL");
  }, 3000);
  timer.unref();
}

process.on("SIGINT", () => stop());
process.on("SIGTERM", () => stop());

function start(command, args, env = process.env) {
  const child = spawn(command, args, {
    stdio: "inherit",
    env,
    detached: process.platform !== "win32",
  });
  children.add(child);
  return new Promise((resolve) => {
    child.once("error", (error) => console.error(error.message));
    child.once("close", (code) => {
      terminate(child, "SIGTERM");
      children.delete(child);
      resolve(code ?? 1);
    });
  });
}

const wasmCode = await start(process.execPath, ["scripts/build-wasm.mjs"]);
if (!stopping && wasmCode === 0) {
  const compiler = (tool) => execFileSync("rustup", [
    "which", "--toolchain", "1.98.1", tool,
  ], { encoding: "utf8" }).trim();
  const env = { ...process.env, RUSTC: compiler("rustc"), RUSTDOC: compiler("rustdoc") };
  console.log("[dev] 建置 Rust 房間伺服器…");
  const buildCode = await start("rustup", [
    "run", "1.98.1", "cargo", "build", "--release", "--features", "server", "--bin", "qkmj-server",
  ], env);
  if (!stopping && buildCode === 0) {
    const executable = process.platform === "win32" ? "qkmj-server.exe" : "qkmj-server";
    const server = join(process.env.CARGO_TARGET_DIR || "target", "release", executable);
    console.log("[dev] 啟動 Rust :3000 與 Vite；按 Ctrl+C 同時關閉。");
    const serverExit = start(server, [], { ...env, PORT: "3000" });
    const viteExit = start(process.execPath, [
      "node_modules/vite/bin/vite.js", "--host", "0.0.0.0", ...process.argv.slice(2),
    ]);
    stop(await Promise.race([serverExit, viteExit]));
  } else if (!stopping) stop(buildCode);
} else if (!stopping) stop(wasmCode);
