import init, { WasmGame } from "./pkg/qkmj_browser.js";

let game = null;
let ready = false;

function sendState(json) {
  self.postMessage({ type: "state", state: JSON.parse(json) });
}

function sendError(error) {
  self.postMessage({ type: "error", message: String(error?.message || error) });
}

self.onmessage = async ({ data }) => {
  try {
    if (data.type === "start") {
      await init();
      const seed = Number(data.seed);
      if (!Number.isSafeInteger(seed) || seed < 1 || seed > 4294967295) {
        throw new Error("seed 必須是 1 到 4294967295");
      }
      game = new WasmGame(seed);
      for (const [seat, difficulty] of Object.entries(data.difficulties || {})) {
        game.configure_ai(Number(seat), difficulty);
      }
      ready = true;
      sendState(game.start());
      return;
    }
    if (!ready || !game) {
      throw new Error("引擎尚未準備好");
    }
    if (data.type === "action") {
      sendState(game.action_json(JSON.stringify(data.action)));
    } else if (data.type === "bot") {
      sendState(game.bot_step_json());
    } else if (data.type === "next") {
      for (const [seat, difficulty] of Object.entries(data.difficulties || {})) {
        game.configure_ai(Number(seat), difficulty);
      }
      sendState(game.next_hand_json());
    } else {
      throw new Error("未知的 Worker 訊息");
    }
  } catch (error) {
    sendError(error);
  }
};
