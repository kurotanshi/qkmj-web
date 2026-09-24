# QKMJ browser game

## Play

Serve `browser/web/` from an HTTP origin after generating `web/pkg/`:

```sh
python3 -m http.server --directory browser/web 8080
```

Open `http://127.0.0.1:8080/`. Choose each computer's Weak, Medium, or Strong
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
The static UI is in `web/`; `worker.js` owns the Wasm game and bot steps.
There is no framework, server, remote AI, or new dependency beyond serde,
serde_json, and the pinned Wasm binding.

Verified toolchain and binding pins:

- Rust `1.98.1` via rustup
- `wasm-bindgen` crate and CLI `0.2.128`
- Wasm target `wasm32-unknown-unknown`

Bootstrap, when the pinned tools are not already available:

```sh
rustup toolchain install 1.98.1 --profile minimal --component rustfmt --component clippy
rustup target add wasm32-unknown-unknown --toolchain 1.98.1
rustup run 1.98.1 cargo install wasm-bindgen-cli --version 0.2.128 --locked --root browser/.tools
```

From the repository root, set explicit compiler paths because `rustup run`
alone can select another `rustc` from `PATH`, then fetch dependencies once:

```sh
export RUSTC="$(rustup which --toolchain 1.98.1 rustc)"
export RUSTDOC="$(rustup which --toolchain 1.98.1 rustdoc)"
rustup run 1.98.1 cargo fetch --manifest-path browser/Cargo.toml
rustup run 1.98.1 cargo fmt --manifest-path browser/Cargo.toml -- --check
rustup run 1.98.1 cargo test --offline --manifest-path browser/Cargo.toml
node --check browser/web/app.js
node --check browser/web/worker.js
rustup run 1.98.1 cargo build --release --target wasm32-unknown-unknown --manifest-path browser/Cargo.toml
browser/.tools/bin/wasm-bindgen --target web --out-dir browser/web/pkg browser/target/wasm32-unknown-unknown/release/qkmj_browser.wasm
PATH="$(dirname "$RUSTC"):$PATH" CARGO_TARGET_DIR=.acceptance/clippy-target cargo clippy --offline --all-targets --manifest-path browser/Cargo.toml -- -D warnings
```

Native tests cover seeded natural win and reserve-wall draw paths, tile
conservation, flower/kong lifecycle, legal claims and kongs, scoring and
settlement, dealer wrap, revision/API atomicity, hidden-state policy views,
and deterministic policy choices. The fixed native fixtures are seed `3` for
a natural win and seed `1` for a reserve-wall draw. For a browser smoke check,
serve the generated package over HTTP, verify that Wasm loads, then exercise
the button-only path: take Win when offered, otherwise take the first
available normal action. Check click, touch, keyboard/focus, difficulty
changes, next hand, restart, result rendering, and post-load network silence.
