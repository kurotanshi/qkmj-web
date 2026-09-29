FROM rust:1.98.1-bookworm AS rust-build

WORKDIR /build
RUN rustup target add wasm32-unknown-unknown \
    && cargo install wasm-bindgen-cli --version 0.2.128 --locked
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY src ./src
RUN cargo build --release --features server --bin qkmj-server \
    && cargo build --release --target wasm32-unknown-unknown \
    && mkdir -p /build/pkg \
    && wasm-bindgen --target web --out-dir /build/pkg target/wasm32-unknown-unknown/release/qkmj_browser.wasm

FROM node:24-bookworm AS web-build

WORKDIR /build
COPY package.json package-lock.json ./
RUN npm ci
COPY vite.config.js ./
COPY web ./web
COPY --from=rust-build /build/pkg ./web/pkg
RUN npx vite build

FROM debian:bookworm-slim AS runtime

RUN useradd --system --create-home --home-dir /app qkmj
WORKDIR /app
COPY --from=rust-build /build/target/release/qkmj-server /usr/local/bin/qkmj-server
COPY --from=web-build /build/dist ./dist
RUN chown -R qkmj:qkmj /app
USER qkmj
ENV PORT=3000
EXPOSE 3000
CMD ["/usr/local/bin/qkmj-server"]
