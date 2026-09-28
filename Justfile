native_debug := "build/native"
native_release := "build/native-release"
web_debug := "build/web"
web_release := "build/web-release"

default: run

build:
    cmake -S . -B {{native_debug}} -G Ninja -DCMAKE_BUILD_TYPE=Debug
    cmake --build {{native_debug}}

run: build
    ./{{native_debug}}/wrum

release:
    cmake -S . -B {{native_release}} -G Ninja -DCMAKE_BUILD_TYPE=Release
    cmake --build {{native_release}}

run-release: release
    ./{{native_release}}/wrum

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
