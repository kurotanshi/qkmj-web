# Repository Guidelines

## Project Structure & Module Organization

- `src/`: Rust 2021 crate; `engine.rs` manages game state, `rules.rs` handles scoring, `ai.rs` implements bots, and `lib.rs` exposes WebAssembly bindings.
- `tests/`: native integration and regression tests.
- `web/`: Vite-rooted HTML, CSS, JavaScript, and the offline Worker. `app.js` renders the UI; `worker.js` owns the Wasm game and bot steps.
- `src/server.rs` and `src/bin/qkmj-server.rs`: optional native HTTP/WebSocket service and room lifecycle.
- `scripts/build-wasm.mjs`, `vite.config.js`, `Dockerfile`, and `render.yaml`: pinned browser build and deployment packaging.
- `web/pkg/`: generated bindings and Wasm; ignored by Git.
- `README.md`: detailed game rules, toolchain setup, and verification instructions.

## Build, Test, and Development Commands

Run from the repository root. Follow `README.md` to install pinned Rust `1.98.1`, the Wasm target, and wasm-bindgen CLI `0.2.128`. Set compiler paths before Cargo commands:

```sh
export RUSTC="$(rustup which --toolchain 1.98.1 rustc)"
export RUSTDOC="$(rustup which --toolchain 1.98.1 rustdoc)"
```

- Fetch dependencies: `rustup run 1.98.1 cargo fetch`.
- Run native tests: `rustup run 1.98.1 cargo test --offline`.
- Check formatting: `rustup run 1.98.1 cargo fmt -- --check`.
- Check JavaScript syntax: `node --check web/app.js` and `node --check web/worker.js`.
- Run the real Node 24 WebSocket regression after building the release server: `node --test tests/ws-regression.mjs`.
- Check development process startup/failure/shutdown: `node --test tests/dev-runner.mjs` (POSIX fixtures).

Build and generate browser assets:

```sh
rustup run 1.98.1 cargo build --release --target wasm32-unknown-unknown
./.tools/bin/wasm-bindgen --target web --out-dir web/pkg target/wasm32-unknown-unknown/release/qkmj_browser.wasm
npm run build
rustup run 1.98.1 cargo build --release --features server --bin qkmj-server
PORT=3000 ./target/release/qkmj-server
```

Use `npm run dev` to build Wasm/native code and start both Vite and the Rust server (port 3000); Ctrl+C stops both. Vite proxies `/ws` to Rust. For a production build, open the native service at `http://127.0.0.1:3000/`. Render Free may sleep, cold-start, and restart the single in-memory service; rooms and reconnect credentials are lost on restart. Configure a custom domain manually in Render and at the DNS provider. Run the README's Clippy command with warnings denied before submitting Rust changes.

## Coding Style & Naming Conventions

Use rustfmt's four-space Rust indentation, `snake_case` functions/modules, and `PascalCase` types. Match existing two-space JavaScript/CSS indentation, camelCase JavaScript names, double quotes, and semicolons. Keep gameplay logic in Rust and browser presentation in the static UI. Preserve Traditional Chinese labels and keyboard accessibility.

## Testing Guidelines

Use Rust's built-in `#[test]` framework with descriptive `snake_case` names in `tests/`. Add regression coverage for changed rules, state transitions, and input validation. No numeric coverage threshold is configured. Existing deterministic fixtures use seed `3` for a win and `1` for a draw.

For UI changes, rebuild Wasm and manually check loading, mouse/touch/keyboard controls, difficulty selection, restart, next hand, and results.

## Commit & Pull Request Guidelines

History uses short imperative subjects, such as `Ignore local verification build artifacts`; follow that style. No PR template is present. Describe the change, list verification performed, link relevant issues, and include screenshots for visual changes. Keep generated packages, tool installations, and build artifacts out of commits.
