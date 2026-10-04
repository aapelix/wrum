cmake_minimum_required(VERSION 3.28)

project(Wrum
    VERSION 0.1
    LANGUAGES C
)

set(CMAKE_C_STANDARD 23)
set(CMAKE_C_STANDARD_REQUIRED ON)

if(EMSCRIPTEN)
    set(CMAKE_C_EXTENSIONS ON)
elseif(UNIX)
    set(CMAKE_C_EXTENSIONS ON)
else()
    set(CMAKE_C_EXTENSIONS OFF)
endif()

set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

option(WRUM_SERVER_ONLY "Build only the server" OFF)

# cJSON
add_library(cJSON STATIC
    vendor/cJSON/cJSON.c
)

target_include_directories(cJSON PUBLIC
    vendor/cJSON
)

# =========================
# Client
# =========================

if(NOT WRUM_SERVER_ONLY)
    add_subdirectory(vendor/raylib EXCLUDE_FROM_ALL)

    set(TRANSPORT_SRC src/net/transport_ws.c)

    if(NOT EMSCRIPTEN)
        set(TRANSPORT_SRC src/net/transport_enet.c)
    endif()

    add_executable(wrum
        src/main.c
        src/stack.c
        src/car.c
        src/camera.c
        src/sim.c
        src/collision.c
        src/map.c
        src/level.c
        src/scene/scene.c
        src/scene/game_scene.c
        src/scene/menu_scene.c
        src/scene/general.c
        ${TRANSPORT_SRC}
    )

    target_link_libraries(wrum
        PRIVATE raylib cJSON
    )

    target_include_directories(wrum PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}/include
    )

    if(EMSCRIPTEN)
        target_link_options(wrum PRIVATE
            "--preload-file=${CMAKE_SOURCE_DIR}/assets@/assets"
            "--shell-file=${CMAKE_SOURCE_DIR}/assets/shell.html"
            "-lwebsocket.js"
        )

        set(CMAKE_EXE_LINKER_FLAGS
            "${CMAKE_EXE_LINKER_FLAGS} -s USE_GLFW=3 -s ASSERTIONS=1 -s WASM=1 -s ASYNCIFY -s GL_ENABLE_GET_PROC_ADDRESS=1"
        )

        set(CMAKE_EXECUTABLE_SUFFIX ".html")
    endif()

endif()

# =========================
# Server
# =========================

if(NOT EMSCRIPTEN)
    add_executable(server
        src/server_main.c
        src/sim.c
        src/net/transport_enet.c
        src/net/transport_ws_server.c
        src/collision.c
        src/level.c
    )

    add_library(mongoose STATIC
        vendor/mongoose/mongoose.c
    )

    target_include_directories(mongoose PUBLIC
        vendor/mongoose
    )

    target_link_libraries(server
        PRIVATE mongoose cJSON m
    )

    target_include_directories(server PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}/include
    )
endif()
