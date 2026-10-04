# =========================
# Build WASM
# =========================
FROM emscripten/emsdk:latest AS web

RUN apt-get update && apt-get install -y ninja-build \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /src
COPY . .

RUN emcmake cmake \
    -S . \
    -B build/web \
    -G Ninja \
    -DCMAKE_BUILD_TYPE=Release \
    -DPLATFORM_WEB

RUN cmake --build build/web


# =========================
# Build server
# =========================
FROM ubuntu:24.04 AS server

RUN apt-get update && apt-get install -y \
    build-essential \
    cmake \
    ninja-build \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /src

COPY . .

RUN cmake \
    -S . \
    -B build/server \
    -G Ninja \
    -DCMAKE_BUILD_TYPE=Release \
    -DWRUM_SERVER_ONLY=ON

RUN cmake --build build/server --target server


# =========================
# Runtime
# =========================
FROM ubuntu:24.04

WORKDIR /app

COPY --from=server /src/build/server/server ./server
COPY --from=server /src/assets ./assets
COPY --from=web /src/build/web ./web

ENV WRUM_WEB_ROOT=/app/web

CMD ["./server"]
