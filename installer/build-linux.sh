#!/usr/bin/env bash
# Builds target/dist/OBCR-Setup-<version>-linux-x86_64: one self-extracting
# file holding OBCR Setup (Qt 6), the Qt libraries it needs and the release
# game.
#
#   installer/build-linux.sh
#
# Needs Rust (rustup), CMake, Ninja, patchelf, the Qt 6 base and Wayland
# development packages and SDL3's build dependencies (X11, Wayland, ALSA,
# PulseAudio, PipeWire, udev, D-Bus headers). Build on an older distribution
# (Ubuntu 22.04) so the result runs on newer ones.
# SPDX-License-Identifier: GPL-3.0-or-later
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"

version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
build=target/installer/linux
deploy=target/installer/linux-deploy
dist=target/dist
out="$dist/OBCR-Setup-$version-linux-x86_64"
qmake=$(command -v qmake6 || command -v qmake)

echo "== cargo build"
cargo build --release -p obcr -p obcr-setup
strip target/release/obcr target/release/obcr-setup

echo "== build Setup"
cmake -S installer -B "$build" -G Ninja -DCMAKE_BUILD_TYPE=Release \
    -DOBCR_BINARY="$root/target/release/obcr" -DOBCR_VERSION="$version"
cmake --build "$build"

echo "== deploy the Qt runtime"
rm -rf "$deploy"
mkdir -p "$deploy/lib" "$deploy/plugins" "$dist"
cp "$build/OBCR-Setup" "$deploy/"
strip "$deploy/OBCR-Setup"
plugins=$("$qmake" -query QT_INSTALL_PLUGINS)
for p in platforms/libqxcb.so platforms/libqwayland-generic.so platforms/libqwayland-egl.so \
         platforms/libqoffscreen.so xcbglintegrations/libqxcb-glx-integration.so \
         xcbglintegrations/libqxcb-egl-integration.so wayland-shell-integration/libxdg-shell.so \
         wayland-decoration-client/libbradient.so platforminputcontexts/libcomposeplatforminputcontextplugin.so \
         platformthemes/libqxdgdesktopportal.so; do
    if [ -f "$plugins/$p" ]; then
        mkdir -p "$deploy/plugins/$(dirname "$p")"
        cp "$plugins/$p" "$deploy/plugins/$p"
    fi
done
printf '[Paths]\nPrefix = .\nPlugins = plugins\n' > "$deploy/qt.conf"

# Libraries every desktop provides (C runtime, graphics drivers, X11 and
# Wayland core, fonts, GLib, D-Bus): taken from the system, as AppImages do.
system='^(ld-linux|libc\.so|libm\.so|libdl\.so|libpthread|librt\.so|libresolv|libutil|libgcc_s|libstdc\+\+|libGL|libEGL|libOpenGL|libGLES|libGLX|libGLdispatch|libdrm|libgbm|libvulkan|libX11|libXau|libXdmcp|libXext|libXrender|libXi\.|libXfixes|libxcb\.so|libxcb-(glx|dri2|dri3|present|sync|xfixes|shm|randr|render\.|shape)|libxshmfence|libwayland|libfontconfig|libfreetype|libexpat|libz\.so|libpng16|libharfbuzz|libgraphite2|libbrotli|libglib-2|libgobject|libgthread|libgio|libgmodule|libpcre\.|libpcre2-8|libdbus-1|libsystemd|libudev|libselinux|libmount|libblkid|libuuid|libffi|libgcrypt|libgpg-error|liblzma|liblz4|libzstd|libcap|libbsd|libmd\.so|libbz2)'
collect() {
    ldd "$1" | awk '/=> \//{print $3}' | while read -r lib; do
        name=$(basename "$lib")
        if ! [[ $name =~ $system ]] && [ ! -f "$deploy/lib/$name" ]; then
            cp -L "$lib" "$deploy/lib/$name"
        fi
    done
}
collect "$deploy/OBCR-Setup"
find "$deploy/plugins" -name '*.so' | while read -r p; do collect "$p"; done
patchelf --set-rpath '$ORIGIN/lib' "$deploy/OBCR-Setup"
for l in "$deploy"/lib/*.so*; do patchelf --set-rpath '$ORIGIN' "$l"; done
find "$deploy/plugins" -name '*.so' -exec patchelf --set-rpath '$ORIGIN/../../lib' {} \;

# The uninstaller copy needs exactly these files.
{
    echo "# Runtime files of OBCR Setup, copied with the uninstaller."
    (cd "$deploy" && find . -type f ! -name OBCR-Setup | sed 's|^\./||' | sort)
    echo "setup-runtime.txt"
} > "$deploy/setup-runtime.txt"

echo "== pack"
cargo xtask pack-installer target/release/obcr-setup "$deploy" "$out"
echo "== done: $out ($(du -h "$out" | cut -f1))"
