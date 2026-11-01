FROM emscripten/emsdk:latest AS base

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    pkg-config \
    curl \
    ca-certificates \
    libssl-dev \
    libasound2-dev \
    libudev-dev \
    libclang-dev \
    && rm -rf /var/lib/apt/lists/*

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --profile minimal

ENV PATH="/root/.cargo/bin:${PATH}"

RUN cargo install cargo-chef --locked \
    && rustup target add wasm32-unknown-emscripten

WORKDIR /src

FROM base AS planner

COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ---- Build ----
FROM base AS build

ARG WRUM_SERVER_URL
ENV WRUM_SERVER_URL=${WRUM_SERVER_URL}

ENV EMCC_CFLAGS="-O3 -sUSE_GLFW=3 -sASSERTIONS=0 -sWASM=1 -sASYNCIFY -sGL_ENABLE_GET_PROC_ADDRESS=1 -sALLOW_MEMORY_GROWTH=1"
ENV BINDGEN_EXTRA_CLANG_ARGS="--sysroot=/emsdk/upstream/emscripten/cache/sysroot"

COPY --from=planner /src/recipe.json recipe.json
RUN cargo chef cook --release --bin wrum_server \
    --recipe-path recipe.json -j 2

COPY . .

RUN cargo build --release \
    --target wasm32-unknown-emscripten \
    --bin wrum_app \
    --features debug-renderer -j 2

RUN cp target/wasm32-unknown-emscripten/release/wrum_app.wasm web/ \
    && cp target/wasm32-unknown-emscripten/release/wrum_app.js web/ \
    && cp target/wasm32-unknown-emscripten/release/deps/wrum_app.data web/

RUN cargo build --release --bin wrum_server -j 2

# ---- Runtime ----
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=build /src/target/release/wrum_server /app/server
COPY --from=build /src/web /app/web
COPY --from=build /src/assets /app/assets

ENV WEB=/app/web
ENV LEVEL=/app/assets/maps/untitled.json
ENV ADDR=0.0.0.0:8080

CMD ["/app/server"]
