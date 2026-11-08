#!/usr/bin/env bash
#   bash scripts/smoke/linux-bundle.sh [<screenshot-dir>]
#
# Builds the Linux release bundles (.deb, .rpm, AppImage) as the Release
# workflow does, names them and writes SHA256SUMS.txt with the release
# scripts, installs and removes the .deb, lists the .rpm, then launches the
# AppImage under Xvfb and opens a project by drag-and-drop and by Open….
# Needs: Xvfb, openbox, xdotool, ImageMagick (import), python-xlib, dpkg,
# rpm, and root or sudo for dpkg. Deletes the bundles when it ends.
set -euo pipefail
export LC_ALL=C.UTF-8
root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$root"
shots=$(mkdir -p "${1:-$root/target/smoke-shots}" && cd "${1:-$root/target/smoke-shots}" && pwd)

step() { printf '\n== %s\n' "$*"; }
die() { printf 'FAIL  %s\n' "$*" >&2; exit 1; }
as_root() { if [ "$(id -u)" = 0 ]; then "$@"; else sudo "$@"; fi; }

free_gb=$(df -Pk "$root/target" | awk 'NR==2{print int($4/1048576)}')
[ "$free_gb" -ge 3 ] || die "only ${free_gb} GB free under target/; the bundle build needs 3 GB"
for tool in Xvfb openbox xdotool import dpkg rpm python3; do
  command -v "$tool" >/dev/null || die "$tool is not installed"
done
python3 -c 'import Xlib' 2>/dev/null || die "python-xlib is not installed"

work=$(mktemp -d)
deb_installed=0
pids=()
app_group=
cleanup() {
  if [ -n "$app_group" ]; then kill -- "-$app_group" 2>/dev/null || true; sleep 1; fi
  for pid in "${pids[@]}"; do kill "$pid" 2>/dev/null || true; done
  if [ "$deb_installed" = 1 ]; then as_root dpkg -r separatrix >/dev/null 2>&1 || true; fi
  if [ -f "$work/extracted-before" ]; then
    find "${TMPDIR:-/tmp}" /tmp -maxdepth 1 -name 'appimage_extracted_*' 2>/dev/null | LC_ALL=C sort -u |
      LC_ALL=C comm -13 "$work/extracted-before" - | while IFS= read -r d; do rm -rf "$d"; done
  fi
  rm -rf "$work" "$root/target/release/bundle"
}
trap cleanup EXIT
# The bundler's linuxdeploy tools and the AppImage below run with extract-and-run, which unpacks
# into the temp folder and can leave it there; cleanup removes what this run added.
find "${TMPDIR:-/tmp}" /tmp -maxdepth 1 -name 'appimage_extracted_*' 2>/dev/null | LC_ALL=C sort -u >"$work/extracted-before"

step "Build the bundles"
ver=$(bash scripts/release/assets.sh --version)
(cd app && npx tauri build --bundles deb,rpm,appimage)

step "Name the assets and check the checksums"
assets=$work/assets
bash scripts/release/assets.sh "$assets" linux x86_64
bash scripts/release/manifest.sh "$assets" "$ver" uncrayon/separatrix
(cd "$assets" && sha256sum -c SHA256SUMS.txt) || die "SHA256SUMS.txt does not match the assets"
deb=$assets/Separatrix_${ver}_linux_amd64.deb
rpm=$assets/Separatrix_${ver}_linux_x86_64.rpm
appimage=$assets/Separatrix_${ver}_linux_x86_64.AppImage
for f in "$deb" "$rpm" "$appimage"; do [ -s "$f" ] || die "$f is missing"; done

step "Install and remove the .deb"
as_root dpkg -i "$deb" >/dev/null
deb_installed=1
got=$(dpkg-query -W -f '${Version} ${Status}' separatrix)
[ "$got" = "$ver install ok installed" ] || die "dpkg reports '$got', want '$ver install ok installed'"
[ -x /usr/bin/separatrix ] || die "/usr/bin/separatrix is missing after dpkg -i"
as_root dpkg -r separatrix >/dev/null
deb_installed=0
echo "dpkg: $got, then removed"

step "List the .rpm"
rpm_ver=$(rpm -qp --qf '%{VERSION}' "$rpm" 2>/dev/null)
[ "$rpm_ver" = "$ver" ] || die "the .rpm is version '$rpm_ver', want $ver"
rpm -qpl "$rpm" 2>/dev/null | grep -qx /usr/bin/separatrix || die "the .rpm does not install /usr/bin/separatrix"
rpm -qpl "$rpm" 2>/dev/null | grep -q '/usr/share/mime/packages/.*\.xml$' || die "the .rpm has no MIME type for .sepx"
echo "rpm: $rpm_ver, installs /usr/bin/separatrix and the .sepx MIME type"

step "Launch the AppImage under Xvfb"
n=90
while [ -e "/tmp/.X$n-lock" ] || [ -e "/tmp/.X11-unix/X$n" ]; do n=$((n + 1)); done
export DISPLAY=:$n
Xvfb "$DISPLAY" -screen 0 1600x1000x24 -nolisten tcp >/dev/null 2>&1 &
pids+=($!)
for _ in $(seq 50); do xdotool getmouselocation >/dev/null 2>&1 && break; sleep 0.2; done
openbox >/dev/null 2>&1 &
pids+=($!)
chmod +x "$appimage"
mkdir -p "$work/config" "$work/data"
# Its own process group, so the kill reaches the extracted app too.
XDG_CONFIG_HOME=$work/config XDG_DATA_HOME=$work/data \
  WEBKIT_DISABLE_DMABUF_RENDERER=1 WEBKIT_DISABLE_COMPOSITING_MODE=1 LIBGL_ALWAYS_SOFTWARE=1 \
  setsid "$appimage" --appimage-extract-and-run >"$work/app.log" 2>&1 &
app_group=$!

window=
for _ in $(seq 60); do
  window=$(xdotool search --onlyvisible --name Separatrix 2>/dev/null | tail -1 || true)
  [ -n "$window" ] && break
  sleep 1
done
[ -n "$window" ] || { tail -20 "$work/app.log" >&2; die "no Separatrix window after 60 s"; }
sleep 4

wait_title() {
  local want=$1 title=
  for _ in $(seq 30); do
    title=$(xdotool getwindowname "$window" 2>/dev/null || true)
    [ "$title" = "$want" ] && return 0
    sleep 0.5
  done
  echo "window title is '$title', want '$want'" >&2
  return 1
}

open_with_dialog() {
  local file=$1 dialog=
  xdotool windowactivate --sync "$window" key ctrl+o
  for _ in $(seq 20); do
    dialog=$(xdotool search --onlyvisible --name '^Open File$' 2>/dev/null | head -1 || true)
    [ -n "$dialog" ] && break
    sleep 0.5
  done
  [ -n "$dialog" ] || { echo "Ctrl+O showed no Open File dialog" >&2; return 1; }
  sleep 2
  xdotool windowactivate --sync "$dialog" key ctrl+l
  sleep 0.5
  xdotool type --delay 20 "$file"
  sleep 1
  xdotool key Return
}

step "Drop a project on the window"
drop=$root/docs/guide/examples/continuation.sepx
if python3 scripts/smoke/xdnd_drop.py "$drop" "$window" && wait_title "continuation — Separatrix"; then
  echo "drag-and-drop opened continuation.sepx"
else
  echo "the drop did not open it; trying Open… instead" >&2
  open_with_dialog "$drop" && wait_title "continuation — Separatrix" || die "neither a drop nor Open… opened $drop"
fi
sleep 3
import -window root "$shots/M5-gate-appimage-drop.png"

step "Open a project with Open…"
opened=$root/docs/guide/examples/lyapunov.sepx
open_with_dialog "$opened" && wait_title "lyapunov — Separatrix" || die "Open… did not open $opened"
sleep 3
import -window root "$shots/M5-gate-appimage-open.png"

recent=$(find "$work/config" -name recent-projects.json | head -1)
[ -n "$recent" ] || die "no recent-projects.json was written"
grep -q 'continuation.sepx' "$recent" && grep -q 'lyapunov.sepx' "$recent" ||
  die "$recent does not list both projects"

printf '\nPASS  Linux bundles %s: .deb installs and removes, .rpm lists, AppImage opens a dropped project and one from Open…\n' "$ver"
echo "Screenshots: $shots/M5-gate-appimage-drop.png, $shots/M5-gate-appimage-open.png"
