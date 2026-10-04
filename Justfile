default: run

run:
    cargo run

web:
    cargo build --target wasm32-unknown-emscripten

web-release:
    cargo build --release --target wasm32-unknown-emscripten

serve:
    cp target/wasm32-unknown-emscripten/debug/deps/wrum.{js,wasm,data} web/
    python3 -m http.server 8000 -d web
