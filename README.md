# QKMJ Web

以 Rust／WebAssembly 重製的 QKMJ 十六張麻將，保留黑底 ANSI 彩色文字與 Telnet 終端機風格。

- 單機瀏覽器遊玩：1 位真人與 3 位 AI，各自可選弱／中／強。
- 線上四人房間：伺服器主持牌局、斷線由中 AI 接手，重連憑證保留座位。
- 字體隨視窗縮放，支援滑鼠、鍵盤與觸控操作。
- Rust 遊戲引擎在 Web Worker 執行；前端使用原生 HTML、CSS 與 JavaScript。

## 快速開始

以下指令皆從儲存庫根目錄執行。不要直接用 `file://` 開啟。

依照 [建置與驗證說明](#contributing-and-verification) 安裝固定版本的 Rust、wasm-bindgen 與 Node 24，然後：

```sh
npm ci
npm run dev
```

開啟 <http://127.0.0.1:5173/>。`npm run dev` 會只建置一次 Wasm，再交給 Vite；啟動線上房間時另開終端機執行 `cargo run --release --features server --bin qkmj-server`，Vite 會代理 `/ws`。

Production build and native service:

```sh
npm run build
PORT=3000 cargo run --release --features server --bin qkmj-server
```

服務會提供 `dist/`、`/ws` 與 `/health`，預設綁定 `0.0.0.0:3000`。房間與重連憑證只在記憶體中存在，服務重啟後所有房間消失；沒有公開觀眾、登入、資料庫或多實例協調。

After the release server is built, run the real Node 24 socket regression with:

```sh
node --test tests/ws-regression.mjs
```

The test chooses an available local port, observes child startup/exit, and covers four connected clients, AI takeover, reconnect watching, Result readiness, stale retries, malformed/unknown/oversize input, and idle unauthenticated admission.

## 專案結構

- `Cargo.toml`、`Cargo.lock`、`rust-toolchain.toml`：根目錄的 crate、依賴鎖定與固定工具鏈。
- `src/`：Rust 規則、遊戲狀態與 AI。
- `tests/`：引擎與回歸測試。
- `web/`：終端機風格介面及 Worker。
- `src/server.rs`、`src/bin/qkmj-server.rs`：feature-gated 原生房間、WebSocket 與靜態服務。
- `scripts/build-wasm.mjs`、`vite.config.js`：固定工具鏈的 Wasm 產出與 Vite bundle。
- `Dockerfile`、`render.yaml`：非 root 的單一 Render Free 服務部署設定。
- `web/pkg/`、`target/`、`.tools/`：產生的網頁套件、Rust 建置產物與本機工具，不納入版本控制。
- `AGENTS.md`：貢獻指南。

## Play

After starting the HTTP server above, choose each computer's Weak, Medium, or Strong
setting before every hand. The selectors lock when the hand starts and are
available again after a result. The normal game uses a random positive 32-bit
seed; `?seed=1` through `?seed=4294967295` is an optional reproducibility
switch.

You are physical East and the initial dealer. Use the hand buttons for
discards, the action tray for draw/claim/win/pass/kong choices, and keyboard
focus or touch exactly as for a mouse click. Claimed rivers are marked. A
result shows the winning tile, selected concealed and exposed decomposition,
tai, payment source, settlement, dealer change, and continuation count.

## QKMJ legacy profile

- 144 tiles: four each of 1–9, 11–19, 21–29, winds 31–34, dragons 41–43,
  and one each of flowers 51–58. In labels, 42 is 白 and 43 is 發.
- Each seat receives 16 tiles; the dealer receives one extra. The original
  16-tile packets are sorted before flower replacement. The dealer extra tile
  is replaced first, then the original positions in dealer order. Flowers and
  kong replacements draw from the live front wall.
- Sixteen wall tiles are reserved before every ordinary or replacement draw;
  if no draw is possible, the hand is a draw, the dealer stays, and the
  consecutive-dealer count increases once.
- A discard collects all replies. Priority is win, discard kong, pong, chow;
  simultaneous wins choose the first clockwise seat after the discarder.
  Chow is only the next live seat and offers every legal suited shape. Own
  concealed and added kongs draw a replacement. Robbing a kong is absent.
- A normal win is five melds plus one pair after exposed melds. Decompositions
  visit triplet, sequence, then pair in legacy order; equal maximum tai keeps
  the first decomposition.
- Tai values are: 1 tai for dealer, closed hand, self draw, no terminals or
  honors, one double sequence, kong-replacement win, last live self draw,
  last discard, each matching wind/dragon/flower; 2 for double wind, a full
  season set, a full plant set, all claimed, pinfu, mixed outside, three-color
  sequence, one through nine, two double sequences, three concealed triplets,
  three kongs, three-color triplets, and continuation at `2 × count`; 3 for
  closed self draw; 4 for all triplets, mixed one suit, pure outside, mixed
  terminals, and small three dragons; 6 for four concealed triplets and four
  kongs; 8 for big three dragons, small four winds, pure one suit, all honors,
  five concealed triplets, and pure terminals; 16 for big four winds and the
  first-round dealer/self-draw/other-seat wins.
- Exclusions follow the legacy scorer: closed self draw replaces closed plus
  self draw; complete season/plant sets remove one flower tai; pure outside
  replaces mixed outside; mixed terminals removes mixed outside and all
  triplets; big/small dragon and wind patterns remove their lower patterns;
  pure one suit removes mixed one suit; all honors removes all triplets; pure
  terminals removes all triplets, pure/mixed outside, and mixed terminals;
  five concealed triplets removes all triplets; big wind/dragon patterns
  remove their wind/dragon tai.
- The legacy stubs #8 搶槓, #44 七搶一, and #48 八仙過海 remain zero. Seven
  pairs, thirteen orphans, special flower wins, and a minimum-tai gate are
  not part of this profile. Approved safety corrections cover north double
  wind, actual exposed tiles, mixed-terminal classification, exact kong-win
  source, reserve enforcement, and validated input.
- Base value is 500 and one tai is 200. A self-draw charges every other seat
  500 + tai × 200, except a dealer payer who adds `1 + 2 × continuation`
  tai. A discard win charges only the discarder using the same dealer-payer
  surcharge. Totals are zero-sum. A dealer win continues; another winner
  advances the dealer and clears continuation, with the round wind wrapping
  after the fourth seat.

## Contributing and verification

The single Rust crate in `src/` is shared by native tests and WebAssembly.
The static UI is in `web/`; `worker.js` owns only the offline Wasm game and
bot steps. The optional `server` feature adds the native Axum/Tokio service;
the default crate remains the offline library and Wasm target.

Verified toolchain and binding pins:

- Rust `1.98.1` via rustup
- `wasm-bindgen` crate and CLI `0.2.128`
- Wasm target `wasm32-unknown-unknown`

Bootstrap, when the pinned tools are not already available:

```sh
rustup toolchain install 1.98.1 --profile minimal --component rustfmt --component clippy
rustup target add wasm32-unknown-unknown --toolchain 1.98.1
rustup run 1.98.1 cargo install wasm-bindgen-cli --version 0.2.128 --locked --root .tools
```

From the repository root, set explicit compiler paths because `rustup run`
alone can select another `rustc` from `PATH`, then fetch dependencies once:

```sh
export RUSTC="$(rustup which --toolchain 1.98.1 rustc)"
export RUSTDOC="$(rustup which --toolchain 1.98.1 rustdoc)"
rustup run 1.98.1 cargo fetch
rustup run 1.98.1 cargo fmt -- --check
rustup run 1.98.1 cargo test --offline
node --check web/app.js
node --check web/worker.js
rustup run 1.98.1 cargo build --release --target wasm32-unknown-unknown
./.tools/bin/wasm-bindgen --target web --out-dir web/pkg target/wasm32-unknown-unknown/release/qkmj_browser.wasm
PATH="$(dirname "$RUSTC"):$PATH" CARGO_TARGET_DIR=.acceptance/clippy-target cargo clippy --offline --all-targets -- -D warnings
npm ci
npm run build
rustup run 1.98.1 cargo test --offline --features server
PATH="$(dirname "$RUSTC"):$PATH" rustup run 1.98.1 cargo clippy --offline --all-targets --features server -- -D warnings
```

The server accepts strict JSON WebSocket messages: the first message is
`create {name}` or `join {room_code, name?, reconnect_token?}`; authenticated
commands are `ready {ready}`, `action {revision, kind}`, and `leave`. The
server derives the action seat from the authenticated connection. Room codes
are random hex IDs, tokens are random 256-bit hex values, and no credential is
placed in a URL, public roster, log, or error. Live state sends a public view
plus only the current controller's private hand/actions; a same-hand
reconnect is a read-only watcher until the next hand starts.

Render uses the single free Singapore Docker service in `render.yaml`. The
Free service can sleep, cold-start, and restart; this service keeps rooms and
reconnect credentials only in memory, so a restart ends active rooms. Set a
custom domain manually in the Render dashboard and add the DNS record at the
DNS provider; this repository does not perform deployment or DNS changes.

Native tests cover seeded natural win and reserve-wall draw paths, tile
conservation, flower/kong lifecycle, legal claims and kongs, scoring and
settlement, dealer wrap, revision/API atomicity, hidden-state policy views,
and deterministic policy choices. The fixed native fixtures are seed `3` for
a natural win and seed `1` for a reserve-wall draw. For a browser smoke check,
serve the generated package over HTTP, verify that Wasm loads, then exercise
the button-only path: take Win when offered, otherwise take the first
available normal action. Check click, touch, keyboard/focus, difficulty
changes, next hand, restart, result rendering, and post-load network silence.

## 來源與致謝

原版 QKMJ 由 sysu（吳先祐／Shian-Yow Wu）開發，TonyQ 維護 0.94 beta 分支，gjchen 提供 WebSocket 與 Docker 版本。
原版終端機 client／server 與歷史紀錄保留於來源儲存庫。
