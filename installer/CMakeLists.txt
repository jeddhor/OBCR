# OBCR Setup: a Qt 6 installer and uninstaller (Windows and Linux).
# SPDX-License-Identifier: GPL-3.0-or-later
#
#   cmake -S installer -B <build> -DOBCR_BINARY=<path to release obcr[.exe]> \
#         -DOBCR_VERSION=<x.y.z> -DCMAKE_PREFIX_PATH=<Qt 6 prefix>
#
# The packaging scripts (build-windows.ps1, build-linux.sh) do this, bundle
# the Qt runtime and wrap everything in a single self-extracting file.
cmake_minimum_required(VERSION 3.21)
project(obcr_setup LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)

find_package(Qt6 6.2 REQUIRED COMPONENTS Widgets)

set(OBCR_BINARY "" CACHE FILEPATH "Release build of the game to install")
set(OBCR_VERSION "0.0.0" CACHE STRING "OBCR version shown and recorded by Setup")
if(NOT EXISTS "${OBCR_BINARY}")
    message(FATAL_ERROR "Set OBCR_BINARY to a release build of obcr (got '${OBCR_BINARY}').")
endif()

qt_add_executable(OBCR-Setup WIN32
    src/main.cpp
    src/setup.cpp
    src/setup.h
    src/wizard.cpp
    src/wizard.h
)
target_compile_definitions(OBCR-Setup PRIVATE OBCR_VERSION="${OBCR_VERSION}" QT_NO_CAST_TO_ASCII)
target_link_libraries(OBCR-Setup PRIVATE Qt6::Widgets)
if(WIN32)
    set(game_name obcr.exe)
else()
    set(game_name obcr)
endif()

# The game and the licence travel inside Setup as resources.
set(license "${CMAKE_CURRENT_SOURCE_DIR}/../LICENSE")
set_source_files_properties("${OBCR_BINARY}" PROPERTIES QT_RESOURCE_ALIAS "${game_name}")
set_source_files_properties("${license}" PROPERTIES QT_RESOURCE_ALIAS "LICENSE")
qt_add_resources(OBCR-Setup payload
    PREFIX "/payload"
    BIG_RESOURCES
    FILES "${OBCR_BINARY}" "${license}"
)
