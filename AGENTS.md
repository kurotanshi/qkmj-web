# Repository Guidelines

## Project Structure & Module Organization

- `src/`: Rust 2021 crate; `engine.rs` manages game state, `rules.rs` handles scoring, `ai.rs` implements bots, and `lib.rs` exposes WebAssembly bindings.
- `tests/`: native integration and regression tests.
- `web/`: static HTML, CSS, and JavaScript assets. `app.js` renders the UI; `worker.js` owns the Wasm game and bot steps.
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

Build and generate browser assets:

```sh
rustup run 1.98.1 cargo build --release --target wasm32-unknown-unknown
./.tools/bin/wasm-bindgen --target web --out-dir web/pkg target/wasm32-unknown-unknown/release/qkmj_browser.wasm
python3 -m http.server --directory web 8080
```

Open `http://127.0.0.1:8080/`. Run the README's Clippy command with warnings denied before submitting Rust changes.

## Coding Style & Naming Conventions

Use rustfmt's four-space Rust indentation, `snake_case` functions/modules, and `PascalCase` types. Match existing two-space JavaScript/CSS indentation, camelCase JavaScript names, double quotes, and semicolons. Keep gameplay logic in Rust and browser presentation in the static UI. Preserve Traditional Chinese labels and keyboard accessibility.

## Testing Guidelines

Use Rust's built-in `#[test]` framework with descriptive `snake_case` names in `tests/`. Add regression coverage for changed rules, state transitions, and input validation. No numeric coverage threshold is configured. Existing deterministic fixtures use seed `3` for a win and `1` for a draw.

For UI changes, rebuild Wasm and manually check loading, mouse/touch/keyboard controls, difficulty selection, restart, next hand, and results.

## Commit & Pull Request Guidelines

History uses short imperative subjects, such as `Ignore local verification build artifacts`; follow that style. No PR template is present. Describe the change, list verification performed, link relevant issues, and include screenshots for visual changes. Keep generated packages, tool installations, and build artifacts out of commits.
