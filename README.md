# QKMJ Web

以 Rust／WebAssembly 重製的 QKMJ 十六張麻將，保留黑底 ANSI 彩色文字與 Telnet 終端機風格。

- 單機瀏覽器遊玩：1 位真人與 3 位 AI，各自可選弱／中／強。
- 線上四人房間：伺服器主持牌局、斷線由中 AI 接手，重連憑證保留座位。
- 字體隨視窗縮放，支援滑鼠、鍵盤與觸控操作。
- 單機引擎在 Web Worker 執行；線上牌局由 Rust 伺服器管理。前端使用原生 HTML、CSS 與 JavaScript。

單機結算後按「下一局」即可在原牌桌繼續，不再回到難度設定；AI 難度、累計分數與莊家進度會延續。想重設難度、清除累計分數時，使用右上角「重新開始」。線上結算則在牌桌按「準備下一局」，可看到目前準備人數，所有在線玩家準備後接著開局。

鍵盤操作：輪到出牌時，用 `←`／`→` 選擇手牌（首尾可循環），按空白鍵或 `Enter` 打出反白的牌；也可直接按 `A–Q` 出牌。`0` 表示「無」，輪到摸牌時可按空白鍵摸牌。單機與線上模式皆適用，輸入名稱等文字欄位時不會觸發遊戲快捷鍵。

## 快速開始

**執行 `npm run dev` 就會同時啟動 Vite 前端與 Rust 房間伺服器。** 單機與線上房間都使用同一個指令，只需保留一個終端機。

| 使用方式 | 需要保持運作的服務 | 瀏覽器網址 |
| --- | --- | --- |
| 本機單機／線上房間 | `npm run dev` 管理前後端 | <http://127.0.0.1:5173/> |
| 正式版本的本機測試 | 建置完成後，只啟動 Rust 服務 | <http://127.0.0.1:3000/> |

以下指令皆從專案根目錄執行，也就是包含 `Cargo.toml` 與 `package.json` 的目錄。開發流程不需要 Python；不要使用舊的 `http://127.0.0.1:8080/` 或直接以 `file://` 開啟網頁。

### 1. 第一次使用：準備工具

需要 Node.js 24／npm、Rust `1.98.1`、Wasm target 與 wasm-bindgen CLI `0.2.128`。Rust 和 Wasm 工具的安裝指令見[建置與驗證說明](#contributing-and-verification)。

安裝前端依賴：

```sh
npm ci
```

### 2. 一個指令啟動前後端

```sh
npm run dev
```

指令會依序建置 Wasm、建置 Rust 後端，再同時啟動兩個服務。第一次編譯可能需要一些時間。看到 Vite 顯示 `Local: http://localhost:5173/` 後，瀏覽器開啟 <http://127.0.0.1:5173/>，即可選擇「單機」或「線上房間」。

保持這個終端機運作即可，**不需要另外啟動 Rust 服務**。Vite 會把 `/ws` 連線轉送至 Rust 的 `127.0.0.1:3000`；開發指令固定使用後端 3000 埠。若 Vite 的 5173 已被占用，以終端機實際顯示的 Local 網址為準。

按一次 `Ctrl+C` 會同時關閉前後端。若任一服務退出，啟動腳本也會關閉另一個，避免留下半套服務。停止或重啟 Rust 服務會清除目前所有房間。

修改前端 HTML、CSS、JavaScript 時由 Vite 更新；修改 Rust 原始碼後，請按 `Ctrl+C` 再執行 `npm run dev`，重新建置 Wasm 與後端。目前沒有設定 Rust 自動重新編譯。

若要確認後端是否正常，可用瀏覽器開啟 <http://127.0.0.1:3000/health>，或在另一個終端機執行：

```sh
curl http://127.0.0.1:3000/health
```

回傳 `ok` 代表後端已啟動。開發時仍從 Vite 的網頁建立房間。

### 3. 建立房間並開始遊戲

1. 開啟 Vite 網址，切換到「線上房間」，輸入名稱並按「建立」。
2. 將畫面上的房間代碼交給朋友；其他玩家在相同網站輸入名稱與代碼後按「加入」。
3. 四人都連線並按「準備」後開始第一局。若只有一至三人，桌主可按「提前開始／AI 補位」，立即讓在場玩家入局，其餘座位由 AI 代打，不必等待其他人的準備。四人到齊時仍需全員準備。
4. 若要在同一台電腦測試，手動開啟四個新分頁，各自輸入網址。不要直接複製已入房的分頁，以免複製到同一座位的重連資料。

同一區域網路的朋友可使用 Vite 終端機顯示的 Network 網址，例如 `http://192.168.1.10:5173/`；請使用你電腦實際的 IP，並確認防火牆允許連線。朋友電腦上的 `localhost` 指向朋友自己的電腦，不能用來連到你的服務。不同網路的朋友請使用部署後的網站網址。

線上牌局中，若出牌後你只有「無」可選，伺服器會自動略過，不必點擊；有吃、碰、槓或胡的選項時，仍由你決定。若其他玩家有可選動作，牌局會等待他們回覆。

建立房間的人是桌主，名單會標示「桌主」。桌主離線或離房時，由最早加入且仍在線的玩家接任；原桌主重連不會取回桌主身分。

提前開局後，新玩家仍可用房間代碼加入尚未有人認領的 AI 座位。本局先觀戰，AI 繼續代打，不會公開私人手牌。結算後，所有在線玩家（含觀戰者）按「準備」，下一局開始才接手，並沿用該座位的分數。已有玩家但暫時斷線的座位仍保留給原玩家重連，不供新玩家取代。四個座位都有人認領後即為滿房。

遊戲中斷線後，AI 會接手該局；在原分頁重連或重新整理，會先觀戰，下一局開始才恢復操作。重連資料保存在分頁的 `sessionStorage`，因此不要把關閉分頁後重新開啟視為可靠的重連方式。開局前按「離開房間」會釋放座位，讓其他人補位。

### 本機連線問題排查

| 狀況 | 檢查方式 |
| --- | --- |
| 建立新房間卻顯示「房間已結束，請重新加入」 | 這是舊版提示。更新後重新執行 `npm run dev`，並從 Vite 顯示的網址重新整理網頁。 |
| 顯示「無法連上房間伺服器」 | 確認 `npm run dev` 已完成建置且仍在運作，並檢查 `/health` 是否回傳 `ok`。若啟動失敗，查看同一個終端機的錯誤訊息。 |
| 顯示「找不到房間，可能已過期」 | 後端可能已重啟，或房間在所有人離線五分鐘後過期；需要重新建立房間。 |
| 房間已建立，但牌局沒有開始 | 四人到齊時需全員按「準備」；不足四人時，桌主可按「提前開始／AI 補位」。 |
| 後端顯示埠已被占用 | 先停止之前手動啟動的 Rust 後端，再執行 `npm run dev`。開發腳本固定使用 3000 埠，失敗時會關閉這次啟動的服務。 |

## 正式版本的本機測試

完成上述工具安裝與 `npm ci` 後，在專案根目錄執行：

```sh
npm run build
export RUSTC="$(rustup which --toolchain 1.98.1 rustc)"
export RUSTDOC="$(rustup which --toolchain 1.98.1 rustdoc)"
PORT=3000 rustup run 1.98.1 cargo run --release --features server --bin qkmj-server
```

若開發用的後端仍在執行，請先在原終端機按 `Ctrl+C`，再執行最後一行，以免占用相同的 3000 埠。

開啟 <http://127.0.0.1:3000/>。這個模式不需要 Vite 常駐：Rust 服務同時提供 `dist/` 網頁、`/ws` 與 `/health`，預設綁定 `0.0.0.0:3000`。修改網頁後要重新執行 `npm run build` 才會更新正式資源。

房間與重連資料只保存在伺服器記憶體中，服務重啟後所有房間都會消失；目前沒有登入、資料庫或多實例協調。

完成 release 後端建置後，可另外執行 WebSocket 回歸測試：

```sh
node --test tests/ws-regression.mjs
```

測試會自行選擇可用埠並啟動測試伺服器，涵蓋四人連線、AI 接手、重連觀戰、下一局準備、過期動作重試與輸入驗證。

開發啟動腳本另有程序管理回歸測試，可在 macOS／Linux 執行；使用替代建置與服務程序，檢查中斷、建置失敗及任一服務退出時的清理，不占用遊戲服務的埠：

```sh
node --test tests/dev-runner.mjs
```

## 專案結構

- `Cargo.toml`、`Cargo.lock`、`rust-toolchain.toml`：根目錄的 crate、依賴鎖定與固定工具鏈。
- `src/`：Rust 規則、遊戲狀態與 AI。
- `tests/`：引擎與回歸測試。
- `web/`：終端機風格介面及 Worker。
- `src/server.rs`、`src/bin/qkmj-server.rs`：feature-gated 原生房間、WebSocket 與靜態服務。
- `scripts/dev.mjs`：建置並管理本機前後端，同步處理關閉與失敗退出。
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

- Node.js `24` and npm
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
commands are `ready {ready}`, `start` (host-only early start with fewer than
four connected players), `action {revision, kind}`, and `leave`. The
server derives the action seat from the authenticated connection. Room codes
are random hex IDs, tokens are random 256-bit hex values, and no credential is
placed in a URL, public roster, log, or error. Live state sends a public view
plus only the current controller's private hand/actions; a same-hand
reconnect or a new player claiming an unowned AI seat is a read-only watcher
until the next hand starts. The public room view includes `host_seat`.

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
