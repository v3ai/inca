#!/usr/bin/env bash
# Package a release build of Inca into dist/ (used by .github/workflows/release.yml; also works locally).
#   packaging/package.sh linux            -> dist/Inca-<v>-linux-x86_64.AppImage, dist/Inca-<v>-linux-x86_64.tar.gz
#   packaging/package.sh windows          -> dist/Inca-<v>-win-x64.zip, dist/Inca-<v>-win-x64-portable.exe
#   packaging/package.sh mac <target>     -> dist/Inca-<v>-mac-<arm64|x64>.dmg  (target: aarch64-apple-darwin | x86_64-apple-darwin)
# Expects `cargo build --release -p inca [--target <target>]` to have run.
set -euo pipefail
cd "$(dirname "$0")/.."
version=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
mkdir -p dist

case "${1:-}" in
  linux)
    bin=target/release/inca
    strip "$bin" || true
    # tarball
    stage=$(mktemp -d)/Inca-$version
    mkdir -p "$stage"
    cp "$bin" "$stage/inca"
    cp packaging/inca.desktop build/icon.png README.md "$stage/"
    tar -C "$(dirname "$stage")" -czf "dist/Inca-$version-linux-x86_64.tar.gz" "Inca-$version"
    # AppImage
    app=$(mktemp -d)/Inca.AppDir
    mkdir -p "$app/usr/bin"
    cp "$bin" "$app/usr/bin/inca"
    cp packaging/inca.desktop "$app/inca.desktop"
    cp build/icon.png "$app/inca.png"
    printf '#!/bin/sh\nHERE="$(dirname "$(readlink -f "$0")")"\nexec "$HERE/usr/bin/inca" "$@"\n' > "$app/AppRun"
    chmod +x "$app/AppRun"
    tool=${APPIMAGETOOL:-$(mktemp -d)/appimagetool}
    if [ ! -x "$tool" ]; then
      curl -fsSL -o "$tool" https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage
      chmod +x "$tool"
    fi
    ARCH=x86_64 "$tool" --appimage-extract-and-run "$app" "dist/Inca-$version-linux-x86_64.AppImage"
    ;;
  windows)
    bin=target/release/inca.exe
    stage=$(mktemp -d)/Inca-$version
    mkdir -p "$stage"
    cp "$bin" "$stage/Inca.exe"
    cp README.md "$stage/"
    out="$PWD/dist/Inca-$version-win-x64.zip"
    (cd "$(dirname "$stage")" && python -m zipfile -c "$out" "Inca-$version")
    cp "$bin" "dist/Inca-$version-win-x64-portable.exe"
    ;;
  mac)
    target=${2:?target triple}
    arch=$([ "$target" = aarch64-apple-darwin ] && echo arm64 || echo x64)
    bin=target/$target/release/inca
    work=$(mktemp -d)
    app="$work/Inca.app"
    mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
    cp "$bin" "$app/Contents/MacOS/inca"
    sed "s/VERSION/$version/g" packaging/Info.plist > "$app/Contents/Info.plist"
    # icon.icns from the 1024px icon
    iconset=$(mktemp -d)/icon.iconset
    mkdir -p "$iconset"
    for s in 16 32 128 256 512; do
      sips -z $s $s build/icon.png --out "$iconset/icon_${s}x${s}.png" >/dev/null
      sips -z $((s * 2)) $((s * 2)) build/icon.png --out "$iconset/icon_${s}x${s}@2x.png" >/dev/null
    done
    iconutil -c icns "$iconset" -o "$app/Contents/Resources/icon.icns"
    # unsigned apart from an ad-hoc signature (no Apple Developer account)
    codesign --force --deep -s - "$app"
    ln -s /Applications "$work/Applications"
    hdiutil create -volname "Inca $version" -srcfolder "$work" -ov -format UDZO "dist/Inca-$version-mac-$arch.dmg" -quiet 2>/dev/null || \
      hdiutil create -volname "Inca $version" -srcfolder "$work" -ov -format UDZO "dist/Inca-$version-mac-$arch.dmg"
    ;;
  *)
    echo "usage: $0 linux | windows | mac <target>" >&2
    exit 1
    ;;
esac
ls -l dist
