default: run

run:
    cargo run --bin wrum

server:
    cargo run --bin server

web:
    cargo build --target wasm32-unknown-emscripten --bin wrum

web-release:
    cargo build --release --target wasm32-unknown-emscripten --bin wrum
    mkdir -p web/dist
    cp target/wasm32-unknown-emscripten/release/deps/wrum.{js,wasm,data} web/dist/
    cp web/index.html web/dist/
    cp web/config.js web/dist/

serve:
    cp target/wasm32-unknown-emscripten/debug/deps/wrum.{js,wasm,data} web/
    python3 -m http.server 8000 -d web
