import assert from "node:assert/strict";
import net from "node:net";
import { spawn } from "node:child_process";
import test from "node:test";

const serverCommand = process.env.QKMJ_SERVER || "target/release/qkmj-server";

async function pickPort() {
  const probe = net.createServer();
  await new Promise((resolve, reject) => {
    probe.once("error", reject);
    probe.listen(0, "127.0.0.1", resolve);
  });
  const port = probe.address().port;
  await new Promise((resolve, reject) => probe.close((error) => error ? reject(error) : resolve()));
  return port;
}

async function startServer(t) {
  const port = await pickPort();
  const child = spawn(serverCommand, [], {
    cwd: process.cwd(),
    env: { ...process.env, PORT: String(port) },
    stdio: ["ignore", "pipe", "pipe"],
  });
  let stderr = "";
  let spawnError = null;
  child.stderr.setEncoding("utf8");
  child.stderr.on("data", (chunk) => { stderr += chunk; });
  child.once("error", (error) => { spawnError = error; });
  const exited = new Promise((resolve) => child.once("exit", (code, signal) => resolve({ code, signal })));
  t.after(async () => {
    if (child.exitCode === null && child.signalCode === null) child.kill();
    const stopWait = new Promise((resolve) => {
      const timer = setTimeout(resolve, 1000);
      timer.unref?.();
    });
    const result = await Promise.race([exited, stopWait]);
    if (result?.code !== undefined && result.code !== null && result.code !== 0) {
      throw new Error(`server exited with code ${result.code}: ${stderr}`);
    }
    assert.doesNotMatch(stderr, /panic/i, `server stderr: ${stderr}`);
  });
  await waitForHealth(port, child, () => spawnError, () => stderr);
  return { child, port, get stderr() { return stderr; } };
}

async function waitForHealth(port, child, getSpawnError, getStderr) {
  const deadline = Date.now() + 10000;
  while (Date.now() < deadline) {
    if (getSpawnError()) throw getSpawnError();
    if (child.exitCode !== null) {
      throw new Error(`server exited before health: ${getStderr()}`);
    }
    try {
      if ((await fetch(`http://127.0.0.1:${port}/health`, { signal: AbortSignal.timeout(250) })).ok) return;
    } catch {
      // The child is still binding its port.
    }
    await new Promise((resolve) => {
      const timer = setTimeout(resolve, 50);
      timer.unref?.();
    });
  }
  throw new Error(`server did not become healthy: ${getStderr()}`);
}

function timeoutPromise(message, timeout = 5000) {
  return new Promise((_, reject) => {
    const timer = setTimeout(() => reject(new Error(message)), timeout);
    timer.unref?.();
  });
}

function client(port) {
  const socket = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  const messages = [];
  const waiters = [];
  let lastState = null;
  let pendingAction = false;
  let lastActionAt = 0;
  socket.addEventListener("message", (event) => {
    const message = JSON.parse(event.data);
    messages.push(message);
    if (message.type === "state") {
      lastState = message.state;
      if (pendingAction) {
        const actions = message.state.private?.legal_actions || [];
        const stillOffered = actions.some((entry) => JSON.stringify(entry.kind) === JSON.stringify(pendingAction.kind));
        if (message.state.public?.revision !== pendingAction.revision || !stillOffered) pendingAction = false;
      }
    }
    if (message.type === "error") pendingAction = false;
    for (let index = waiters.length - 1; index >= 0; index -= 1) {
      if (waiters[index].predicate(message)) {
        const waiter = waiters.splice(index, 1)[0];
        clearTimeout(waiter.timer);
        waiter.resolve(message);
      }
    }
  });
  const opened = new Promise((resolve, reject) => {
    socket.addEventListener("open", resolve, { once: true });
    socket.addEventListener("error", reject, { once: true });
  });
  const closed = new Promise((resolve) => socket.addEventListener("close", resolve, { once: true }));
  socket.addEventListener("close", () => {
    for (const waiter of waiters.splice(0)) {
      clearTimeout(waiter.timer);
      waiter.reject(new Error("WebSocket closed before expected message"));
    }
  });
  return {
    socket,
    opened,
    closed,
    messages,
    get lastState() { return lastState; },
    get pendingAction() { return pendingAction; },
    get lastActionAt() { return lastActionAt; },
    set pendingAction(value) { pendingAction = value; },
    set lastActionAt(value) { lastActionAt = value; },
    send(message) { socket.send(JSON.stringify(message)); },
    sendRaw(message) { socket.send(message); },
    next(predicate, timeout = 5000) {
      const found = messages.find(predicate);
      if (found) return Promise.resolve(found);
      return new Promise((resolve, reject) => {
        const waiter = { predicate, resolve, reject, timer: null };
        waiter.timer = setTimeout(() => {
          const index = waiters.indexOf(waiter);
          if (index !== -1) waiters.splice(index, 1);
          reject(new Error("WebSocket message timeout"));
        }, timeout);
        waiter.timer.unref?.();
        waiters.push(waiter);
      });
    },
    closedWithin(timeout = 5000) {
      return Promise.race([closed, timeoutPromise("WebSocket close timeout", timeout)]);
    },
    close() {
      if (socket.readyState === WebSocket.OPEN || socket.readyState === WebSocket.CONNECTING) socket.close();
    },
  };
}

async function waitUntil(predicate, timeout = 30000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    if (predicate()) return;
    await new Promise((resolve) => {
      const timer = setTimeout(resolve, 20);
      timer.unref?.();
    });
  }
  throw new Error("condition timeout");
}

async function expectCleanError(item, code) {
  const error = await item.next((message) => message.type === "error");
  assert.equal(error.code, code);
  const close = await item.closedWithin();
  assert.equal(close.wasClean, true);
  assert.notEqual(close.code, 1006);
}

function sendNextAction(item) {
  const snapshot = item.lastState;
  const actions = snapshot?.private?.legal_actions;
  if (!actions?.length || item.pendingAction || snapshot.public.phase === "result") return;
  if (Date.now() - item.lastActionAt < 180) return;
  const action = actions.find((entry) => entry.kind.type === "win") || actions[0];
  item.pendingAction = { revision: action.revision, kind: action.kind };
  item.lastActionAt = Date.now();
  item.send({ type: "action", revision: action.revision, kind: action.kind });
}

test("four clients exercise ownership, AI takeover, privacy, reconnect, result, and next hand", async (t) => {
  const clients = [];
  const server = await startServer(t);
  t.after(() => clients.forEach((item) => item.close()));

  const first = client(server.port);
  clients.push(first);
  t.after(() => first.close());
  await first.opened;
  first.send({ type: "create", name: "Alice" });
  const created = await first.next((message) => message.type === "created");
  assert.match(created.room_code, /^[0-9a-f]{16}$/);
  assert.match(created.seat_token, /^[0-9a-f]{64}$/);
  const roomCode = created.room_code;
  const tokens = [created.seat_token];

  for (const name of ["Bob", "Carol", "Dylan"]) {
    const item = client(server.port);
    clients.push(item);
    t.after(() => item.close());
    await item.opened;
    item.send({ type: "join", room_code: roomCode, name });
    const joined = await item.next((message) => message.type === "joined");
    tokens.push(joined.seat_token);
  }
  assert.equal(new Set(tokens).size, 4);
  for (const item of clients.slice(0, 4)) item.send({ type: "ready", ready: true });
  await waitUntil(() => clients.slice(0, 4).every((item) => item.lastState?.room.started));

  for (const item of clients.slice(0, 4)) {
    assert.ok(item.lastState.private);
    assert.equal(item.lastState.private.seat, item.lastState.viewer_seat);
    assert.equal(JSON.stringify(item.lastState).includes('"wall"'), false);
  }

  const oldAlice = first;
  oldAlice.close();
  const active = clients.slice(1, 4);
  await waitUntil(() => active.some((item) => item.lastState?.room.seats[0]?.ai));
  const resumed = client(server.port);
  clients.push(resumed);
  t.after(() => resumed.close());
  await resumed.opened;
  resumed.send({ type: "join", room_code: roomCode, reconnect_token: tokens[0] });
  const rejoined = await resumed.next((message) => message.type === "joined");
  assert.equal(rejoined.seat, 0);
  assert.equal(rejoined.state.controller, false);
  assert.equal(rejoined.state.private, undefined);
  assert.equal(rejoined.state.room.seats[0].watching, true);
  assert.equal(rejoined.state.room.seats[0].ai, false);

  let driver = setInterval(() => active.forEach(sendNextAction), 25);
  t.after(() => clearInterval(driver));
  await waitUntil(() => active.some((item) => item.lastState?.public?.revision > 0
    && item.lastState.private?.legal_actions?.length), 15000);
  clearInterval(driver);
  driver = 0;

  const staleClient = active.find((item) => item.lastState?.public?.revision > 0
    && item.lastState.private?.legal_actions?.length);
  assert.ok(staleClient);
  const current = staleClient.lastState;
  const action = current.private.legal_actions[0];
  staleClient.send({ type: "action", revision: current.public.revision - 1, kind: action.kind });
  const stale = await staleClient.next((message) => message.type === "error" && message.code === "STALE_REVISION");
  assert.equal(stale.state.public.revision, current.public.revision);
  assert.ok(stale.state.private.legal_actions.some((entry) => JSON.stringify(entry.kind) === JSON.stringify(action.kind)));
  staleClient.send({ type: "action", revision: current.public.revision, kind: action.kind });
  await waitUntil(() => staleClient.lastState?.public?.revision > current.public.revision, 5000);

  driver = setInterval(() => active.forEach(sendNextAction), 25);
  await waitUntil(() => active.concat(resumed).some((item) => item.lastState?.public?.result), 60000);
  clearInterval(driver);
  driver = 0;
  assert.ok(active.concat(resumed).every((item) => item.socket.readyState === WebSocket.OPEN));

  for (const item of active.concat(resumed)) item.send({ type: "ready", ready: true });
  await waitUntil(() => resumed.lastState?.public?.result === null && resumed.lastState?.controller, 10000);
  assert.ok(resumed.lastState.private);
});

test("host starts with AI and late joiners take control only on the next hand", async (t) => {
  const server = await startServer(t);
  const clients = [];
  t.after(() => clients.forEach((item) => item.close()));
  const host = client(server.port);
  clients.push(host);
  await host.opened;
  host.send({ type: "create", name: "Host" });
  const created = await host.next((message) => message.type === "created");
  assert.equal(created.state.room.host_seat, 0);
  host.send({ type: "start" });
  await waitUntil(() => host.lastState?.room.started);
  assert.equal(host.lastState.room.seats.filter((seat) => seat.ai).length, 3);

  const late = client(server.port);
  clients.push(late);
  await late.opened;
  late.send({ type: "join", room_code: created.room_code, name: "Late" });
  const joined = await late.next((message) => message.type === "joined");
  assert.equal(joined.seat, 1);
  assert.equal(joined.state.controller, false);
  assert.equal(joined.state.private, undefined);
  assert.equal(joined.state.room.seats[1].watching, true);
  late.send({ type: "action", revision: joined.state.public.revision, kind: { type: "draw" } });
  assert.equal((await late.next((message) => message.type === "error")).code, "WATCH_ONLY");

  const driver = setInterval(() => sendNextAction(host), 25);
  t.after(() => clearInterval(driver));
  await waitUntil(() => late.lastState?.public?.result, 60000);
  clearInterval(driver);
  for (const message of late.messages) {
    if (message.state) assert.equal(message.state.private, undefined);
  }
  for (const message of host.messages) {
    const actions = message.state?.private?.legal_actions || [];
    assert.ok(actions.length !== 1 || actions[0].kind.type !== "pass");
  }
  const scores = late.lastState.public.players.map((player) => player.score);
  host.send({ type: "ready", ready: true });
  await waitUntil(() => late.lastState?.room.seats[0].ready);
  assert.ok(late.lastState.public.result);
  late.send({ type: "ready", ready: true });
  await waitUntil(() => late.lastState?.controller && !late.lastState?.public?.result);
  assert.equal(late.lastState.private.seat, 1);
  assert.equal(late.lastState.public.players[1].name, "Late");
  assert.deepEqual(late.lastState.public.players.map((player) => player.score), scores);
  assert.equal(late.lastState.room.seats.filter((seat) => seat.ai).length, 2);
});

test("invalid input, oversize input, and idle admission are observable", async (t) => {
  const server = await startServer(t);
  const clients = [];
  t.after(() => clients.forEach((item) => item.close()));

  const unknown = client(server.port);
  clients.push(unknown);
  t.after(() => unknown.close());
  await unknown.opened;
  unknown.send({ type: "create", name: "Alice", actor: 0 });
  assert.equal((await unknown.next((message) => message.type === "error")).code, "INVALID_MESSAGE");

  const malformed = client(server.port);
  clients.push(malformed);
  t.after(() => malformed.close());
  await malformed.opened;
  malformed.sendRaw("{");
  assert.equal((await malformed.next((message) => message.type === "error")).code, "INVALID_MESSAGE");

  const oversized = client(server.port);
  clients.push(oversized);
  t.after(() => oversized.close());
  await oversized.opened;
  oversized.sendRaw("x".repeat(4097));
  await Promise.race([
    oversized.closed,
    timeoutPromise("oversize socket stayed open"),
  ]);

  const idle = client(server.port);
  clients.push(idle);
  t.after(() => idle.close());
  await idle.opened;
  await Promise.race([
    idle.closed,
    timeoutPromise("idle admission stayed open"),
  ]);
});

test("admission errors flush before close and release socket permits", async (t) => {
  const server = await startServer(t);
  const clients = [];
  t.after(() => clients.forEach((item) => item.close()));

  for (let index = 0; index < 136; index += 1) {
    const item = client(server.port);
    clients.push(item);
    await item.opened;
    item.send({ type: "join", room_code: "0000000000000000", reconnect_token: "missing" });
    await expectCleanError(item, "ROOM_NOT_FOUND");
  }
});

test("invalid and duplicate tokens do not steal an active seat", async (t) => {
  const server = await startServer(t);
  const clients = [];
  t.after(() => clients.forEach((item) => item.close()));

  const owner = client(server.port);
  clients.push(owner);
  await owner.opened;
  owner.send({ type: "create", name: "Alice" });
  const created = await owner.next((message) => message.type === "created");

  const invalid = client(server.port);
  clients.push(invalid);
  await invalid.opened;
  invalid.send({ type: "join", room_code: created.room_code, reconnect_token: "not-a-token" });
  await expectCleanError(invalid, "INVALID_TOKEN");

  const duplicate = client(server.port);
  clients.push(duplicate);
  await duplicate.opened;
  duplicate.send({ type: "join", room_code: created.room_code, reconnect_token: created.seat_token });
  await expectCleanError(duplicate, "DUPLICATE_CONNECTION");

  owner.send({ type: "ready", ready: false });
  await owner.next((message) => message.type === "state" && message.state.room.seats[0].connected);
  owner.sendRaw("{");
  await expectCleanError(owner, "INVALID_MESSAGE");

  const reconnected = client(server.port);
  clients.push(reconnected);
  await reconnected.opened;
  reconnected.send({ type: "join", room_code: created.room_code, reconnect_token: created.seat_token });
  const joined = await reconnected.next((message) => message.type === "joined");
  assert.equal(joined.seat, 0);

  reconnected.send({ type: "create", name: "Not Allowed" });
  await expectCleanError(reconnected, "PROTOCOL");

  const rateLimited = client(server.port);
  clients.push(rateLimited);
  await rateLimited.opened;
  rateLimited.send({ type: "join", room_code: created.room_code, reconnect_token: created.seat_token });
  await rateLimited.next((message) => message.type === "joined");
  for (let index = 0; index < 11; index += 1) rateLimited.send({ type: "ready", ready: false });
  await expectCleanError(rateLimited, "RATE_LIMIT");
});
