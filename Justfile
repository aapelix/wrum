native_debug := "build/native"
native_release := "build/native-release"
web_debug := "build/web"
web_release := "build/web-release"

default: run

build:
    cmake -S . -B {{native_debug}} -G Ninja -DCMAKE_BUILD_TYPE=Debug
    cmake --build {{native_debug}}

run: build
    PORT=7777 HOST=127.0.0.1 ./{{native_debug}}/wrum

release:
    cmake -S . -B {{native_release}} -G Ninja -DCMAKE_BUILD_TYPE=Release
    cmake --build {{native_release}}

run-release: release
    ./{{native_release}}/wrum

server: server-build
    PORT=8888 ./{{native_debug}}/server

server-build:
    cmake -S . -B {{native_debug}} -G Ninja -DCMAKE_BUILD_TYPE=Debug -DWRUM_SERVER_ONLY=ON
    cmake --build {{native_debug}} --target server

web:
    emcmake cmake -S . -B {{web_debug}} -G Ninja -DCMAKE_BUILD_TYPE=Debug
    cmake --build {{web_debug}}

web-release:
    emcmake cmake -S . -B {{web_release}} -G Ninja -DCMAKE_BUILD_TYPE=Release
    cmake --build {{web_release}}

clean:
    rm -rf build

serve:
    python3 -m http.server 8000 --directory build/web
