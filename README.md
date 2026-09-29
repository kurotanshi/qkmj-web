# QKMJ Web

以 Rust／WebAssembly 重製的 QKMJ 十六張麻將，保留黑底 ANSI 彩色文字與 Telnet 終端機風格。

- 單機瀏覽器遊玩：1 位真人與 3 位 AI，各自可選弱／中／強。
- 字體隨視窗縮放，支援滑鼠、鍵盤與觸控操作。
- Rust 遊戲引擎在 Web Worker 執行；前端使用原生 HTML、CSS 與 JavaScript。

## 建置與遊玩

依照 [建置與驗證說明](browser/README.md#contributing-and-verification) 安裝固定版本的 Rust 與 wasm-bindgen，產生 `browser/web/pkg/` 後執行：

```sh
python3 -m http.server --directory browser/web 8080
```

開啟 <http://127.0.0.1:8080/>。完整操作、牌型與計分規則見 [遊戲說明](browser/README.md)。

## 專案結構

- `browser/src/`：Rust 規則、遊戲狀態與 AI。
- `browser/tests/`：引擎與回歸測試。
- `browser/web/`：終端機風格介面及 Worker。
- `browser/web/pkg/`：本機建置產物，不納入版本控制。

## 來源與致謝

原版 QKMJ 由 sysu（吳先祐／Shian-Yow Wu）開發，TonyQ 維護 0.94 beta 分支，gjchen 提供 WebSocket 與 Docker 版本。
原版終端機 client／server 與歷史紀錄保留於來源儲存庫。
