const $ = (selector) => document.querySelector(selector);
const windNames = ["", "東", "南", "西", "北"];
const positionWinds = { south: 1, east: 2, north: 3, west: 4 };
const handLetters = "ABCDEFGHIJKLMNOPQ".split("");
const tileNames = new Map([
  [31, "東風"], [32, "南風"], [33, "西風"], [34, "北風"],
  [41, "紅中"], [42, "白板"], [43, "青發"],
]);
const flowerNames = ["春1", "夏2", "秋3", "冬4", "梅1", "蘭2", "菊3", "竹4"];
const chineseRanks = ["", "一", "二", "三", "四", "五", "六", "七", "八", "九"];

let worker = null;
let workerEpoch = 0;
let state = null;
let botTimer = 0;
let pendingAction = false;
let nextHandPending = false;
let lastFocus = "";
let restoreRequested = false;
let lastLoggedRevision = null;
let resultRevision = null;
let logLines = [];

function tileLabel(tile) {
  if (tileNames.has(tile)) return tileNames.get(tile);
  if (tile >= 51 && tile <= 58) return flowerNames[tile - 51];
  if (tile >= 1 && tile <= 9) return chineseRanks[tile] + "萬";
  if (tile >= 11 && tile <= 19) return chineseRanks[tile - 10] + "索";
  if (tile >= 21 && tile <= 29) return chineseRanks[tile - 20] + "筒";
  return String(tile);
}

function appendTileContent(parent, tile) {
  const isSuited = tile >= 1 && tile <= 9 || tile >= 11 && tile <= 19 || tile >= 21 && tile <= 29;
  if (isSuited || tileNames.has(tile)) {
    const label = tileNames.has(tile)
      ? tileLabel(tile)
      : chineseRanks[tile < 10 ? tile : tile < 20 ? tile - 10 : tile - 20] + (tile < 10 ? "萬" : tile < 20 ? "索" : "筒");
    const [top, bottom] = Array.from(label);
    const rank = document.createElement("span");
    rank.className = "tile-rank";
    rank.textContent = top;
    const suitNode = document.createElement("span");
    suitNode.className = "tile-suit";
    suitNode.textContent = bottom;
    parent.append(rank, suitNode);
  } else {
    parent.textContent = tileLabel(tile);
  }
}

function tileClass(tile) {
  if (tile >= 31 && tile < 40) return "wind";
  if (tile >= 41 && tile < 50) return "dragon";
  if (tile >= 51) return "flower";
  if (tile >= 1 && tile < 10) return "wan";
  if (tile >= 11 && tile < 20) return "sou";
  return "pin";
}

function tileElement(tile, small) {
  const span = document.createElement("span");
  span.className = "tile " + tileClass(tile) + (small ? " tile-small" : "");
  appendTileContent(span, tile);
  span.dataset.tile = String(tile);
  span.setAttribute("aria-label", tileLabel(tile));
  return span;
}

function appendTiles(parent, tiles, small) {
  for (const tile of tiles || []) parent.append(tileElement(tile, small));
}

function phaseName(phase) {
  if (typeof phase === "string") return phase;
  if (!phase || typeof phase !== "object") return "";
  return Object.keys(phase)[0] || "";
}

function isResult(value) {
  const snapshot = value || state;
  return Boolean(snapshot?.public?.result) || phaseName(snapshot?.public?.phase) === "result";
}

function addLog(message) {
  logLines.push(message);
  if (logLines.length > 6) logLines.shift();
}

function recordState(snapshot) {
  const revision = snapshot?.public?.revision;
  if (revision === lastLoggedRevision) return;
  lastLoggedRevision = revision;
  addLog(snapshot.public.status || "狀態更新");
}

function rememberFocus() {
  const active = document.activeElement;
  if (active?.dataset?.focusKey) {
    lastFocus = active.dataset.focusKey;
    restoreRequested = true;
  }
}

function showError(message) {
  const text = "錯誤：" + message + "。可按「重新開始」恢復。";
  const setupError = $("#setup-error");
  setupError.hidden = false;
  setupError.textContent = text;
  addLog(text);
  if ($("#setup").hidden) setSetupEnabled(true, nextHandPending);
  if (state) {
    $("#status").textContent = text;
    renderControls();
    renderActionArea();
    requestAnimationFrame(restoreFocus);
    renderMessages();
  }
}

function setSetupEnabled(enabled, next) {
  $("#setup").hidden = !enabled;
  document.querySelectorAll("#setup select").forEach((control) => {
    control.disabled = !enabled;
  });
  $("#setup-title").textContent = next ? "設定下一局" : "選擇三位電腦的難度";
  $("#setup-hint").textContent = next
    ? "總分、莊家與連莊會保留；選好難度後開始下一局。"
    : "每局開始前可調整；牌局開始後鎖定。";
  $("#start").textContent = next ? "[ 開始下一局 ]" : "[ 開始牌局 ]";
}

function resetWorker() {
  if (worker) {
    worker.onmessage = null;
    worker.onerror = null;
    worker.terminate();
  }
  worker = null;
  workerEpoch += 1;
  if (botTimer) window.clearTimeout(botTimer);
  botTimer = 0;
  pendingAction = false;
}

function newWorker() {
  resetWorker();
  const epoch = workerEpoch;
  worker = new Worker("./worker.js", { type: "module" });
  worker.onmessage = (event) => {
    if (epoch !== workerEpoch) return;
    pendingAction = false;
    if (event.data.type === "error") {
      showError(event.data.message);
      return;
    }
    if (event.data.type !== "state") {
      showError("Worker 回傳格式錯誤");
      return;
    }
    state = event.data.state;
    if (nextHandPending) nextHandPending = false;
    recordState(state);
    render();
    pumpBots();
  };
  worker.onerror = (event) => {
    if (epoch !== workerEpoch) return;
    pendingAction = false;
    showError(event.message || "Worker 載入失敗");
  };
}

function selectedDifficulties() {
  return {
    1: $("#difficulty-1").value,
    2: $("#difficulty-2").value,
    3: $("#difficulty-3").value,
  };
}

function querySeed() {
  const raw = new URLSearchParams(location.search).get("seed");
  if (raw === null) {
    const values = new Uint32Array(1);
    if (globalThis.crypto?.getRandomValues) globalThis.crypto.getRandomValues(values);
    return values[0] || Math.floor(Math.random() * 4294967295) + 1;
  }
  if (!/^[1-9]\d*$/.test(raw)) throw new Error("網址 seed 必須是 1 到 4294967295");
  const seed = Number(raw);
  if (!Number.isSafeInteger(seed) || seed > 4294967295) {
    throw new Error("網址 seed 必須是 1 到 4294967295");
  }
  return seed;
}

function startRound() {
  $("#setup-error").hidden = true;
  const difficulties = selectedDifficulties();
  if (nextHandPending && worker && state) {
    pendingAction = true;
    setSetupEnabled(false, true);
    worker.postMessage({ type: "next", difficulties });
    return;
  }

  let seed;
  try {
    seed = querySeed();
  } catch (error) {
    showError(error.message);
    return;
  }

  state = null;
  nextHandPending = false;
  lastLoggedRevision = null;
  logLines = [];
  resultRevision = null;
  setSetupEnabled(false, false);
  $("#game").hidden = false;
  newWorker();
  pendingAction = true;
  worker.postMessage({ type: "start", seed, difficulties });
}

function restartSession() {
  resetWorker();
  state = null;
  nextHandPending = false;
  resultRevision = null;
  lastLoggedRevision = null;
  logLines = [];
  $("#game").hidden = true;
  $("#setup-error").hidden = true;
  setSetupEnabled(true, false);
  $("#start").focus();
}

function prepareNextHand() {
  nextHandPending = true;
  setSetupEnabled(true, true);
  $("#setup-error").hidden = true;
  $("#start").focus();
}

function sendAction(action) {
  if (!worker || !state || pendingAction || isResult()) return;
  rememberFocus();
  pendingAction = true;
  worker.postMessage({ type: "action", action });
  renderControls();
  renderActionArea();
}

function actionLabel(action) {
  const kind = action.kind;
  if (kind.type === "draw") return "摸牌";
  if (kind.type === "win") return "胡";
  if (kind.type === "pass") return "無";
  if (kind.type === "discard") return "打 " + tileLabel(kind.tile);
  if (kind.type === "pong") return "碰 " + tileLabel(kind.tile);
  if (kind.type === "kong") {
    const name = kind.kind === "concealed" ? "暗槓" : kind.kind === "added" ? "加槓" : "槓";
    return name + " " + tileLabel(kind.tile);
  }
  if (kind.type === "chow") return "吃 " + kind.tiles.map(tileLabel).join(" ");
  return "動作";
}

function actionButton(action, focusKey) {
  const button = document.createElement("button");
  button.type = "button";
  button.className = "action-button";
  button.textContent = actionLabel(action);
  button.dataset.focusKey = focusKey;
  button.dataset.actionType = action.kind.type;
  button.disabled = pendingAction;
  button.addEventListener("click", () => sendAction(action));
  return button;
}

function meldText(meld) {
  const group = document.createElement("span");
  group.className = "meld";
  appendTiles(group, meld.tiles, true);
  return group;
}

function discardElement(discard) {
  const tile = tileElement(discard.tile, true);
  tile.classList.add("discard-tile");
  if (discard.claimed) {
    tile.classList.add("claimed");
    tile.title = "已被吃、碰或槓";
    tile.setAttribute("aria-label", tileLabel(discard.tile) + "（已被宣告）");
  }
  return tile;
}

function playerForPosition(position) {
  const physicalWind = positionWinds[position];
  return state.public.players.find((player) => player.physical_wind === physicalWind);
}

function revealFor(player) {
  return state.public.reveal_hands?.[player.seat] || state.public.result?.revealed_hands?.[player.seat] || [];
}

function renderMelds(player) {
  const melds = document.createElement("div");
  melds.className = "melds";
  melds.setAttribute("aria-label", player.name + " 的副露");
  for (const meld of player.melds || []) melds.append(meldText(meld));
  return melds;
}

function renderFlowers(player) {
  const flowers = document.createElement("div");
  flowers.className = "edge-flowers";
  flowers.setAttribute("aria-label", player.name + " 的花牌");
  appendTiles(flowers, player.flowers, true);
  return flowers;
}

function renderRiver(player, position) {
  const river = document.createElement("div");
  river.className = "river river-" + position;
  river.setAttribute("aria-label", player.name + " 的捨牌區");
  for (const discard of player.discards || []) river.append(discardElement(discard));
  return river;
}

function renderRiverLane(player, position) {
  const lane = document.createElement("section");
  lane.className = "river-lane river-lane-" + position;
  lane.setAttribute("aria-label", player.name + " 的捨牌區");
  const label = document.createElement("span");
  label.className = "river-label";
  label.textContent = windNames[player.physical_wind] + " " + player.name;
  lane.append(label, renderRiver(player, position));
  return lane;
}

function renderOpponent(player, position) {
  const article = document.createElement("article");
  article.className = "edge-player edge-" + position;
  if (state.public.current_seat === player.seat) article.classList.add("is-current");
  article.setAttribute("aria-label", windNames[player.physical_wind] + "位 " + player.name);

  const heading = document.createElement("div");
  heading.className = "edge-heading";
  heading.textContent = windNames[player.physical_wind] + " " + player.name;
  article.append(heading);

  const hand = document.createElement("div");
  hand.className = "opponent-hand";
  hand.setAttribute("aria-label", isResult() ? player.name + " 的結算手牌" : player.name + " 的暗手牌");
  if (isResult()) {
    hand.classList.add("revealed-hand");
    appendTiles(hand, revealFor(player), true);
  } else {
    const backs = document.createElement("span");
    backs.className = "hand-backs";
    for (let index = 0; index < Math.min(player.concealed_count, 17); index += 1) {
      const back = document.createElement("span");
      back.className = "hand-back";
      back.textContent = "▮";
      back.setAttribute("aria-hidden", "true");
      backs.append(back);
    }
    hand.append(backs);
    const count = document.createElement("span");
    count.className = "hand-count";
    count.textContent = player.concealed_count + "張";
    hand.append(count);
  }
  article.append(hand, renderMelds(player), renderFlowers(player));
  return article;
}

function renderHumanSeat() {
  const player = playerForPosition("south");
  const heading = $("#human-seat-head");
  heading.className = "edge-heading human-heading";
  if (state.public.current_seat === player.seat) heading.classList.add("is-current");
  heading.textContent = windNames[player.physical_wind] + " " + player.name;
  $("#human-count").textContent = "手牌 " + state.private.hand.length + " 張";
  const flowers = $("#human-flowers");
  flowers.replaceChildren();
  flowers.setAttribute("aria-label", player.name + " 的花牌");
  appendTiles(flowers, player.flowers, true);
  const melds = $("#human-melds");
  melds.replaceChildren();
  for (const meld of player.melds || []) melds.append(meldText(meld));
}

function renderCenter() {
  const center = $("#center");
  center.replaceChildren();
  const result = state.public.result;
  if (result) {
    const frame = document.createElement("section");
    frame.className = "result-frame";
    const title = document.createElement("h2");
    title.id = "result-title";
    title.tabIndex = -1;
    title.className = "result-winner";
    const winner = result.winner === null || result.winner === undefined
      ? ""
      : state.public.players[result.winner]?.name || "";
    title.textContent = result.draw ? "流局" : winner + (result.source === "discard" ? " 榮和" : " 自摸");
    frame.append(title);

    if (!result.draw) {
      const winning = document.createElement("p");
      winning.className = "result-winning";
      winning.textContent = "勝牌 " + tileLabel(result.winning_tile);
      frame.append(winning);
      const selected = result.decomposition;
      if (selected) {
        const decomposition = document.createElement("p");
        const sets = (selected.sets || []).map((set) => "{" + set.map(tileLabel).join(" ") + "}").join(" ");
        decomposition.textContent = "將 " + tileLabel(selected.pair) + " " + sets;
        frame.append(decomposition);
      }
      const tai = result.tai || [];
      tai.slice(0, 4).forEach((item) => {
        const row = document.createElement("p");
        row.className = "result-tai";
        row.textContent = item.name + " " + item.value + "台";
        frame.append(row);
      });
      if (tai.length > 4) {
        const more = document.createElement("p");
        more.className = "result-tai";
        more.textContent = "…另 " + (tai.length - 4) + " 項";
        frame.append(more);
      }
      const total = document.createElement("p");
      total.className = "result-total";
      total.textContent = "共 " + result.total_tai + " 台";
      frame.append(total);
    } else {
      const draw = document.createElement("p");
      draw.className = "result-tai";
      draw.textContent = "牌山保留十六張";
      frame.append(draw);
      const total = document.createElement("p");
      total.className = "result-total";
      total.textContent = "0 台";
      frame.append(total);
    }
    center.append(frame);
    return;
  }

  const board = document.createElement("div");
  board.className = "river-board";
  board.setAttribute("aria-label", "四家捨牌區");
  for (const position of ["north", "west", "east", "south"]) {
    board.append(renderRiverLane(playerForPosition(position), position));
  }
  const readout = document.createElement("div");
  readout.className = "center-status";
  const current = state.public.current_seat === null || state.public.current_seat === undefined
    ? "結算"
    : state.public.players[state.public.current_seat].name;
  readout.append(
    infoLine("牌山 " + state.public.wall_remaining),
    infoLine("目前 " + current),
  );
  const phase = state.public.phase;
  if (phase && typeof phase === "object" && phase.claim) {
    const discard = document.createElement("p");
    discard.className = "last-discard";
    discard.textContent = "河牌 " + tileLabel(phase.claim.tile);
    readout.append(discard);
  }
  board.append(readout);
  center.append(board);
}

function renderTable() {
  for (const position of ["north", "west", "east"]) {
    const target = $("#seat-" + position);
    const player = playerForPosition(position);
    target.replaceChildren(renderOpponent(player, position));
  }
  renderHumanSeat();
  renderCenter();
}

function infoLine(text, className) {
  const line = document.createElement("p");
  line.className = className || "info-line";
  line.textContent = text;
  return line;
}

function renderInfo() {
  $("#round-label").textContent = windNames[state.public.round_wind] + "局";
  const human = state.public.players[state.public.human_seat];
  $("#wall-count").textContent = "牌山 " + state.public.wall_remaining;
  const round = $("#round-info");
  round.replaceChildren(
    infoLine(windNames[state.public.round_wind] + "局  莊" + windNames[state.public.dealer + 1] + "位"),
    infoLine("連莊 " + state.public.consecutive_dealer),
    infoLine("你  門風" + windNames[human.door_wind]),
  );
  const scores = $("#scores");
  scores.replaceChildren();
  const players = [...state.public.players].sort((a, b) => a.physical_wind - b.physical_wind);
  for (const player of players) {
    const row = document.createElement("div");
    row.className = "score-row";
    if (state.public.current_seat === player.seat) row.classList.add("is-current");
    row.setAttribute("aria-label", windNames[player.physical_wind] + "位 " + player.name + " " + player.score + "分");
    const wind = document.createElement("span");
    wind.className = "score-wind";
    wind.textContent = windNames[player.physical_wind];
    const name = document.createElement("span");
    name.className = "score-name";
    name.textContent = player.name;
    const score = document.createElement("span");
    score.className = "score-value";
    score.textContent = "$" + player.score.toLocaleString();
    const door = document.createElement("span");
    door.className = "score-door";
    door.textContent = "門" + windNames[player.door_wind];
    row.append(wind, name, score, door);
    scores.append(row);
  }
  renderActionArea();
}

function renderActionArea() {
  const tray = $("#action-tray");
  tray.replaceChildren();
  if (isResult()) {
    const next = document.createElement("button");
    next.type = "button";
    next.className = "action-button result-action";
    next.textContent = "下一局";
    next.dataset.focusKey = "result-next";
    next.addEventListener("click", prepareNextHand);
    const restart = document.createElement("button");
    restart.type = "button";
    restart.className = "action-button result-action";
    restart.textContent = "重新開始";
    restart.dataset.focusKey = "result-restart";
    restart.addEventListener("click", restartSession);
    tray.append(next, restart);
    return;
  }

  const actions = state.private.legal_actions || [];
  actions
    .filter((action) => action.kind.type !== "discard")
    .forEach((action, index) => {
      tray.append(actionButton(action, "action-" + index + "-" + action.kind.type));
    });
  if (!tray.children.length) {
    const waiting = document.createElement("p");
    waiting.className = "action-wait";
    const hasDiscard = actions.some((action) => action.kind.type === "discard");
    waiting.textContent = state.private.needs_human
      ? hasDiscard ? "請選牌出牌" : "請選擇動作"
      : "電腦思考中…";
    tray.append(waiting);
  }
}

function renderControls() {
  const hand = $("#human-hand");
  hand.replaceChildren();
  const actions = state.private.legal_actions || [];
  const discardByTile = new Map(
    actions
      .filter((action) => action.kind.type === "discard")
      .map((action) => [action.kind.tile, action]),
  );

  state.private.hand.forEach((tile, index) => {
    const slot = document.createElement("span");
    slot.className = "hand-slot";
    const key = document.createElement("span");
    key.className = "hand-key";
    key.textContent = handLetters[index] || String(index + 1);
    const button = document.createElement("button");
    button.type = "button";
    button.className = "tile tile-control tile-button " + tileClass(tile);
    appendTileContent(button, tile);
    button.dataset.focusKey = "tile-" + tile + "-" + index;
    button.setAttribute("aria-label", key.textContent + "：打出" + tileLabel(tile));
    const action = discardByTile.get(tile);
    button.disabled = !action || pendingAction || isResult();
    if (action && !isResult()) button.addEventListener("click", () => sendAction(action));
    slot.append(key, button);
    hand.append(slot);
  });
}

function decompositionText(result) {
  const selected = result.decomposition;
  if (!selected) return "分解資料不可用";
  const concealed = (selected.sets || []).map((set) => "{" + set.map(tileLabel).join(" ") + "}").join(" ");
  const exposed = (selected.exposed || [])
    .map((meld) => "[" + (meld.tiles || []).map(tileLabel).join(" ") + "]")
    .join(" ");
  return "將 " + tileLabel(selected.pair) + " " + concealed + (exposed ? " 明組 " + exposed : "");
}

function renderResultDetails() {
  const panel = $("#result-panel");
  const result = state.public.result;
  panel.replaceChildren();
  if (!result) {
    panel.hidden = true;
    return;
  }
  panel.hidden = false;
  const heading = document.createElement("h3");
  heading.textContent = "結算明細";
  panel.append(heading);
  if (result.draw) {
    panel.append(infoLine("流局：牌山保留十六張", "detail-line"));
  } else {
    const winner = state.public.players[result.winner]?.name || "";
    const source = result.source === "discard" ? "榮和" : "自摸";
    panel.append(infoLine("勝家：" + winner + "／" + source + "／勝牌 " + tileLabel(result.winning_tile), "detail-line"));
    panel.append(infoLine("分解：" + decompositionText(result), "detail-line"));
    panel.append(infoLine("台：" + (result.tai || []).map((item) => item.name + " " + item.value).join("、"), "detail-line"));
    const payer = result.payment_source === null || result.payment_source === undefined
      ? "其餘三家各自付款"
      : state.public.players[result.payment_source].name;
    panel.append(infoLine("付款：" + payer, "detail-line"));
  }
  panel.append(infoLine(
    "莊家：" + windNames[result.dealer_before + 1] + "位→" + windNames[result.dealer_after + 1]
      + "位；連莊 " + result.consecutive_dealer_before + "→" + result.consecutive_dealer_after
      + (result.dealer_continued ? "（續莊）" : "（換莊）")
      + (result.dealer_surcharge_tai ? "；加 " + result.dealer_surcharge_tai + " 台" : ""),
    "detail-line",
  ));
  panel.append(infoLine("四家捨牌（* 為已宣告）", "detail-line"));
  for (const player of [...state.public.players].sort((a, b) => a.physical_wind - b.physical_wind)) {
    const discards = (player.discards || [])
      .map((discard) => tileLabel(discard.tile) + (discard.claimed ? "*" : ""))
      .join(" ");
    panel.append(infoLine(windNames[player.physical_wind] + " " + player.name + "：" + (discards || "—"), "detail-line"));
  }
  for (const [seat, change] of result.changes.entries()) {
    const sign = change >= 0 ? "+" : "";
    panel.append(infoLine(state.public.players[seat].name + " " + sign + change.toLocaleString() + "分", "detail-line"));
  }
}

function renderMessages() {
  const log = $("#message-log");
  log.replaceChildren();
  for (const message of logLines) log.append(infoLine(message, "log-line"));
  $("#status").textContent = state.public.status || "";
  renderResultDetails();
}

function renderCompass() {
  const compass = $("#compass");
  const positions = ["west", "south", "east", "north"];
  for (const position of positions) {
    const physicalWind = positionWinds[position];
    const player = state.public.players.find((item) => item.physical_wind === physicalWind);
    const item = $("#compass-seat-" + position);
    item.className = "compass-seat compass-" + position;
    if (state.public.current_seat === player.seat) item.classList.add("is-current");
    item.textContent = windNames[player.physical_wind] + " " + player.name;
    item.setAttribute("aria-label", windNames[player.physical_wind] + "位 " + player.name);
  }
  compass.setAttribute("aria-label", "座位方向；目前輪到 " + (
    state.public.current_seat === null || state.public.current_seat === undefined
      ? "結算"
      : state.public.players[state.public.current_seat].name
  ));
}

function restoreFocus() {
  if (!restoreRequested) return;
  const target = lastFocus
    ? [...document.querySelectorAll("[data-focus-key]")].find(
      (element) => element.dataset.focusKey === lastFocus && !element.disabled,
    )
    : null;
  const fallback = document.querySelector("#human-hand button:not(:disabled)")
    || document.querySelector("#action-tray button:not(:disabled)");
  if (!target && !fallback) return;
  restoreRequested = false;
  (target || fallback).focus({ preventScroll: true });
}

function render() {
  if (!state) return;
  rememberFocus();
  const wasResult = resultRevision !== null;
  const resultNow = isResult();
  const resultChanged = resultNow && (!wasResult || resultRevision !== state.public.revision);
  if (resultNow) resultRevision = state.public.revision;
  else resultRevision = null;

  $("#game").hidden = false;
  renderTable();
  renderInfo();
  renderControls();
  renderMessages();
  renderCompass();

  if (resultChanged) {
    restoreRequested = false;
    requestAnimationFrame(() => {
      const title = $("#result-title");
      if (title) title.focus({ preventScroll: true });
      else $("#action-tray button")?.focus({ preventScroll: true });
    });
  } else if (restoreRequested) {
    requestAnimationFrame(restoreFocus);
  }
}

function pumpBots() {
  if (botTimer || !state || pendingAction || isResult() || state.private.needs_human) return;
  botTimer = window.setTimeout(() => {
    botTimer = 0;
    if (worker && state && !pendingAction && !isResult() && !state.private.needs_human) {
      pendingAction = true;
      worker.postMessage({ type: "bot" });
    }
  }, 80);
}

function isTextEntry(target) {
  return target?.closest?.("select, input, textarea, [contenteditable='true']");
}

function handleKeydown(event) {
  if (
    event.defaultPrevented
    || event.ctrlKey
    || event.metaKey
    || event.altKey
    || event.repeat
    || event.isComposing
    || !$("#setup").hidden
    || isTextEntry(event.target)
    || !state
    || pendingAction
    || isResult()
  ) return;
  const key = event.key.toUpperCase();
  if (key.length === 1 && handLetters.includes(key)) {
    const button = [...document.querySelectorAll("#human-hand button")][handLetters.indexOf(key)];
    if (button && !button.disabled) {
      event.preventDefault();
      button.focus({ preventScroll: true });
      button.click();
    }
    return;
  }
  if (event.key === "0") {
    const action = state.private.legal_actions.find((item) => item.kind.type === "pass");
    if (action) {
      event.preventDefault();
      document.querySelector("#action-tray button[data-action-type='pass']")?.focus({ preventScroll: true });
      sendAction(action);
    }
    return;
  }
  if (event.code === "Space" && event.target?.tagName !== "BUTTON") {
    const action = state.private.legal_actions.find((item) => item.kind.type === "draw");
    if (action) {
      event.preventDefault();
      document.querySelector("#action-tray button[data-action-type='draw']")?.focus({ preventScroll: true });
      sendAction(action);
    }
  }
}

$("#start").addEventListener("click", startRound);
$("#restart-top").addEventListener("click", restartSession);
document.addEventListener("keydown", handleKeydown);
setSetupEnabled(true, false);
