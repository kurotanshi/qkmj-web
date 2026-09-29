import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";

const runner = resolve("scripts/dev.mjs");

async function waitFor(predicate) {
  for (let attempt = 0; attempt < 200; attempt += 1) {
    if (await predicate()) return;
    await new Promise((done) => setTimeout(done, 25));
  }
  assert.fail("dev runner did not reach the expected state within 5 seconds");
}

function alive(pid) {
  try { process.kill(pid, 0); return true; }
  catch (error) { if (error.code === "ESRCH") return false; throw error; }
}

for (const scenario of ["interrupt", "repeated interrupt", "server failure", "vite failure", "build failure"]) {
  test(`dev runner cleans up after ${scenario}`, { skip: process.platform === "win32" }, async (t) => {
    const cwd = await mkdtemp(join(tmpdir(), "qkmj-dev-test-"));
    const pids = [];
    let child;
    t.after(async () => {
      if (child?.exitCode === null && child?.signalCode === null) child.kill("SIGTERM");
      for (const pid of pids) if (alive(pid)) process.kill(pid, "SIGKILL");
      await rm(cwd, { recursive: true, force: true });
    });
    for (const directory of ["scripts", "bin", "target/release", "node_modules/vite/bin"]) {
      await mkdir(join(cwd, directory), { recursive: true });
    }
    // Stub only the toolchain and services; execute the real supervisor in another process.
    await writeFile(join(cwd, "scripts/build-wasm.mjs"),
      scenario === "build failure" ? "process.exit(2);" : "");
    await writeFile(join(cwd, "bin/rustup"),
      "#!/usr/bin/env node\nif (process.argv[2] === 'which') console.log('/unused/compiler');\n",
      { mode: 0o755 });
    const service = (name) => `#!/usr/bin/env node
require('node:fs').writeFileSync('${name}.pid', String(process.pid));
${scenario === "repeated interrupt" ? "process.on('SIGTERM', () => setTimeout(() => process.exit(0), 100));" : ""}
setInterval(() => {}, 1000);
`;
    await writeFile(join(cwd, "target/release/qkmj-server"), service("server"), { mode: 0o755 });
    await writeFile(join(cwd, "node_modules/vite/bin/vite.js"), service("vite"));
    child = spawn(process.execPath, [runner], {
      cwd,
      env: { ...process.env, PATH: `${join(cwd, "bin")}:${process.env.PATH}`, CARGO_TARGET_DIR: "target" },
      stdio: "ignore",
    });
    const exited = new Promise((done, reject) => {
      child.once("error", reject);
      child.once("close", (code) => done(code));
    });
    if (scenario === "build failure") {
      await waitFor(() => child.exitCode !== null);
      assert.equal(await exited, 2);
      for (const name of ["server", "vite"]) {
        await assert.rejects(readFile(join(cwd, `${name}.pid`)), { code: "ENOENT" });
      }
      return;
    }
    await waitFor(async () => {
      try {
        const values = await Promise.all(["server", "vite"].map((name) => readFile(join(cwd, `${name}.pid`), "utf8")));
        pids.push(...values.map(Number));
        return true;
      } catch (error) { if (error.code === "ENOENT") return false; throw error; }
    });
    const interrupted = scenario.includes("interrupt");
    if (interrupted) {
      child.kill("SIGINT");
      if (scenario === "repeated interrupt") {
        await new Promise((done) => setTimeout(done, 25));
        child.kill("SIGINT");
      }
    } else process.kill(pids[scenario === "server failure" ? 0 : 1], "SIGTERM");
    await waitFor(() => child.exitCode !== null || child.signalCode !== null);
    assert.equal(await exited, interrupted ? 0 : 1);
    assert.ok(pids.every((pid) => !alive(pid)), "both owned services must exit");
  });
}
