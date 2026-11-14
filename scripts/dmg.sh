#!/bin/sh
# Makes Binder.dmg from a folder holding Binder.app and a link to /Applications: the window gets the
# background from app/dmg and its icons are placed through Finder. The first time, macOS asks whether
# the terminal may control Finder. Another volume named Binder may be mounted; the new one is found by its path.
# usage: scripts/dmg.sh <folder> <background.tiff> <out.dmg>
set -eu
src=$1 bg=$2 out=$3
rw="$out.rw.dmg"

mkdir -p "$src/.background"
cp "$bg" "$src/.background/background.tiff"
rm -f "$rw" "$out"
hdiutil create -quiet -volname Binder -srcfolder "$src" -fs HFS+ -format UDRW -ov "$rw"
attached=$(hdiutil attach -readwrite -noverify -noautoopen "$rw" 2>/dev/null | grep Apple_HFS)
dev=$(echo "$attached" | awk '{print $1}')
mnt=/Volumes/${attached#*/Volumes/}
trap 'hdiutil detach -quiet "$dev" 2>/dev/null || true' EXIT

osascript - "$mnt/.background/background.tiff" "${mnt#/Volumes/}" <<'EOF'
on run argv
tell application "Finder"
  tell disk (item 2 of argv)
    open
    set current view of container window to icon view
    set toolbar visible of container window to false
    set statusbar visible of container window to false
    set bounds of container window to {200, 120, 860, 552}
    set opts to the icon view options of container window
    set arrangement of opts to not arranged
    set icon size of opts to 112
    set text size of opts to 12
    set background picture of opts to (POSIX file (item 1 of argv) as alias)
    set position of item "Binder.app" of container window to {170, 250}
    set position of item "Applications" of container window to {490, 250}
    close
    open
    update without registering applications
    delay 1
    close
  end tell
end tell
end run
EOF

sync
hdiutil detach -quiet "$dev"
trap - EXIT
hdiutil convert -quiet "$rw" -format UDZO -o "$out"
rm -f "$rw"
