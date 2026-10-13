set export
EMCC_CFLAGS :="-v -O3 -sUSE_GLFW=3 -sASSERTIONS=1 -sWASM=1 -sASYNCIFY -sGL_ENABLE_GET_PROC_ADDRESS=1"

default: run

run:
    cargo run --bin wrum_app

server:
    cargo run --bin wrum_server

web:
    BINDGEN_EXTRA_CLANG_ARGS="--sysroot=$EMSDK/upstream/emscripten/cache/sysroot" cargo build --target wasm32-unknown-emscripten --bin wrum_app

web-release:
    BINDGEN_EXTRA_CLANG_ARGS="--sysroot=$EMSDK/upstream/emscripten/cache/sysroot" cargo build --release --target wasm32-unknown-emscripten --bin wrum_app

serve:
    cp target/wasm32-unknown-emscripten/debug/wrum_app.{wasm,js} web/
    cp target/wasm32-unknown-emscripten/debug/deps/wrum_app.data web/
    python3 -m http.server 8000 -d web
