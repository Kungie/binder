#!/bin/bash
# Builds build/Binder.app (and build/Binder.dmg when dmgbuild is installed, plus build/Binder.zip).
# Needs macOS 14+ and Swift 6 (Xcode or the Command Line Tools).
set -euo pipefail
cd "$(dirname "$0")/.."
make -s scratch

# Universal binaries: Apple silicon and Intel, macOS 14 and later.
mkdir -p build/bin
for arch in arm64 x86_64; do
  for product in BinderApp binder; do
    swift build -c release --triple "$arch-apple-macosx14.0" --product "$product" >/dev/null
  done
  bin="$(swift build -c release --triple "$arch-apple-macosx14.0" --show-bin-path)"
  cp "$bin/BinderApp" "build/bin/BinderApp-$arch"
  cp "$bin/binder" "build/bin/binder-$arch"
done
lipo -create build/bin/BinderApp-arm64 build/bin/BinderApp-x86_64 -output build/bin/BinderApp
lipo -create build/bin/binder-arm64 build/bin/binder-x86_64 -output build/bin/binder
BIN=build/bin/BinderApp
# Rendering (icon, DMG background) runs the per-arch build next to its resource bundle, not the lipo'd copy.
RENDER="$(swift build -c release --triple "$(uname -m)-apple-macosx14.0" --show-bin-path)/BinderApp"

APP=build/Binder.app
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Helpers" "$APP/Contents/Resources/Fonts"
cp "$BIN" "$APP/Contents/MacOS/Binder"
# The CLI the hooks call. Not next to the app binary: Binder and binder collide on a case-insensitive disk.
cp build/bin/binder "$APP/Contents/Helpers/binder"
cp Resources/Info.plist "$APP/Contents/Info.plist"
cp Sources/BinderUI/Resources/Fonts/*.woff2 Sources/BinderUI/Resources/Fonts/OFL-*.txt "$APP/Contents/Resources/Fonts/"

# App icon, drawn by Binder itself.
ICONSET=build/Binder.iconset
rm -rf "$ICONSET" && mkdir -p "$ICONSET"
"$RENDER" icon build/icon-1024.png
for s in 16 32 128 256 512; do
  sips -z $s $s build/icon-1024.png --out "$ICONSET/icon_${s}x${s}.png" >/dev/null
  sips -z $((s*2)) $((s*2)) build/icon-1024.png --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/Binder.icns"

# Ad-hoc signature, so macOS runs it locally. Not notarized: first launch needs Privacy & Security › Open Anyway.
# ~/Desktop syncs with iCloud, which stamps Finder attributes on files; codesign refuses those.
xattr -cr "$APP"
codesign --force --sign - "$APP/Contents/Helpers/binder" >/dev/null
codesign --force --sign - "$APP" >/dev/null

# Installer: a disk image with the drag-to-Applications window. Needs dmgbuild (pip install dmgbuild).
DMGBUILD="${DMGBUILD:-$(command -v dmgbuild || true)}"
[ -z "$DMGBUILD" ] && [ -x "$HOME/Library/Caches/Binder/venv/bin/dmgbuild" ] && DMGBUILD="$HOME/Library/Caches/Binder/venv/bin/dmgbuild"
if [ -n "$DMGBUILD" ]; then
  "$RENDER" dmg-background build/dmg-background.png
  rm -f build/Binder.dmg
  "$DMGBUILD" -s scripts/dmg-settings.py -D app="$APP" -D bg=build/dmg-background.png Binder build/Binder.dmg >/dev/null
  echo "Built build/Binder.dmg"
else
  echo "dmgbuild not found, skipping build/Binder.dmg (pip install dmgbuild)"
fi

# Zip for a GitHub release.
rm -f build/Binder.zip
ditto -c -k --keepParent "$APP" build/Binder.zip
echo "Built $APP and build/Binder.zip"
