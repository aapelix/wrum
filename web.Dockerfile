ARG EMSDK_VERSION=6.0.10

FROM emscripten/emsdk:${EMSDK_VERSION} AS build

ARG WRUM_SERVER_URL
ENV WRUM_SERVER_URL=$WRUM_SERVER_URL

RUN apt-get update && apt-get install -y --no-install-recommends \
      curl build-essential cmake clang libclang-dev pkg-config \
    && rm -rf /var/lib/apt/lists/*

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --profile minimal \
      --target wasm32-unknown-emscripten
ENV PATH=/root/.cargo/bin:$PATH

ENV EMCC_CFLAGS="-O3 -sUSE_GLFW=3 -sASSERTIONS=1 -sWASM=1 -sASYNCIFY -sGL_ENABLE_GET_PROC_ADDRESS=1"
ENV BINDGEN_EXTRA_CLANG_ARGS="--sysroot=/emsdk/upstream/emscripten/cache/sysroot"

WORKDIR /src
COPY . .

RUN cargo build --release --target wasm32-unknown-emscripten --bin wrum_app -j 2

RUN mkdir /out \
 && cp -r web/. /out/ \
 && cp target/wasm32-unknown-emscripten/release/wrum_app.wasm \
       target/wasm32-unknown-emscripten/release/wrum_app.js \
       target/wasm32-unknown-emscripten/release/deps/wrum_app.data \
       /out/

FROM caddy:2-alpine
COPY Caddyfile.web /etc/caddy/Caddyfile
COPY --from=build /out /srv
