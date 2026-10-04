cmake_minimum_required(VERSION 4.4)

project(Wrum
    VERSION 0.1
    LANGUAGES C
)

set(CMAKE_C_STANDARD 23)
set(CMAKE_C_STANDARD_REQUIRED ON)
if(EMSCRIPTEN)
    set(CMAKE_C_EXTENSIONS ON)
else()
    set(CMAKE_C_EXTENSIONS OFF)
endif()
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

# sdl
set(SDL_TESTS OFF CACHE BOOL "" FORCE)
set(SDL_EXAMPLES OFF CACHE BOOL "" FORCE)

set(SDLIMAGE_PNG ON CACHE BOOL "" FORCE)
set(SDLIMAGE_ANI OFF CACHE BOOL "" FORCE)
set(SDLIMAGE_BMP OFF CACHE BOOL "" FORCE)
set(SDLIMAGE_GIF OFF CACHE BOOL "" FORCE)
set(SDLIMAGE_JPG OFF CACHE BOOL "" FORCE)
set(SDLIMAGE_JXL OFF CACHE BOOL "" FORCE)
set(SDLIMAGE_LBM OFF CACHE BOOL "" FORCE)
set(SDLIMAGE_PCX OFF CACHE BOOL "" FORCE)
set(SDLIMAGE_PNM OFF CACHE BOOL "" FORCE)
set(SDLIMAGE_QOI OFF CACHE BOOL "" FORCE)
set(SDLIMAGE_SVG OFF CACHE BOOL "" FORCE)
set(SDLIMAGE_TGA OFF CACHE BOOL "" FORCE)
set(SDLIMAGE_TIF OFF CACHE BOOL "" FORCE)
set(SDLIMAGE_WEBP OFF CACHE BOOL "" FORCE)
set(SDLIMAGE_XCF OFF CACHE BOOL "" FORCE)
set(SDLIMAGE_XPM OFF CACHE BOOL "" FORCE)
set(SDLIMAGE_XV OFF CACHE BOOL "" FORCE)
set(SDLIMAGE_AVIF OFF CACHE BOOL "" FORCE)

add_subdirectory(vendor/raylib EXCLUDE_FROM_ALL)

# game
add_library(cJSON STATIC
    vendor/cJSON/cJSON.c
)
target_include_directories(cJSON PUBLIC
    vendor/cJSON
)

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
    ${TRANSPORT_SRC}
)

target_link_libraries(wrum
    PRIVATE raylib cJSON
)

target_include_directories(wrum PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/include
)

if(NOT EMSCRIPTEN)
    # server
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
        PRIVATE raylib mongoose cJSON
    )

    target_include_directories(server PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}/include
    )
endif()

if(EMSCRIPTEN)
    target_link_options(wrum PRIVATE
        "--preload-file=${CMAKE_SOURCE_DIR}/assets@/assets"
        "--shell-file=${CMAKE_SOURCE_DIR}/assets/shell.html"
        "-lwebsocket.js"
    )

    set(CMAKE_EXE_LINKER_FLAGS "${CMAKE_EXE_LINKER_FLAGS} -s USE_GLFW=3 -s ASSERTIONS=1 -s WASM=1 -s ASYNCIFY -s GL_ENABLE_GET_PROC_ADDRESS=1")
    set(CMAKE_EXECUTABLE_SUFFIX ".html")
endif()
