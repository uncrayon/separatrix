#!/usr/bin/env bash
#   bash scripts/docs-screenshots.sh [path/to/separatrix]
#
# Build the app first with the bundled UI:
#   cd app && npm run build && cd .. && cargo build --release -p separatrix --features tauri/custom-protocol
# A later `cargo test --release` (the gates run one) rebuilds the binary without
# the bundled UI, so build again, or pass a copy of the binary.
# Needs Xvfb and ImageMagick (import, convert).
set -euo pipefail
cd "$(dirname "$0")/.."

BIN=${1:-target/release/separatrix}
WIDTH=1440
HEIGHT=900
SETTLE_SECONDS=3
TIMEOUT_SECONDS=90

for tool in Xvfb import convert; do
  command -v "$tool" >/dev/null || { echo "docs-screenshots: $tool is not installed" >&2; exit 2; }
done
[ -x "$BIN" ] || { echo "docs-screenshots: $BIN is missing; build it as the header says" >&2; exit 2; }
BIN=$(cd "$(dirname "$BIN")" && pwd)/$(basename "$BIN")

display=120
while [ -e "/tmp/.X11-unix/X$display" ] || [ -e "/tmp/.X$display-lock" ]; do display=$((display + 1)); done
export DISPLAY=:$display

work=$(mktemp -d)
xvfb_pid=
app_pid=
cleanup() {
  [ -z "$app_pid" ] || kill "$app_pid" 2>/dev/null || true
  [ -z "$xvfb_pid" ] || kill "$xvfb_pid" 2>/dev/null || true
  rm -rf "$work"
}
trap cleanup EXIT

Xvfb "$DISPLAY" -screen 0 "${WIDTH}x${HEIGHT}x24" -nolisten tcp >/dev/null 2>&1 &
xvfb_pid=$!
for _ in $(seq 50); do [ -e "/tmp/.X11-unix/X$display" ] && break; sleep 0.1; done

screen_hash() { import -window root rgb:- 2>/dev/null | md5sum | cut -d' ' -f1; }

capture_when_settled() {
  local out=$1 last='' now same=0 waited=0
  while [ "$waited" -lt "$TIMEOUT_SECONDS" ]; do
    sleep 1
    waited=$((waited + 1))
    now=$(screen_hash)
    if [ "$now" = "$last" ]; then same=$((same + 1)); else same=0; last=$now; fi
    if [ "$same" -ge "$SETTLE_SECONDS" ]; then
      import -window root "$work/shot.png"
      convert "$work/shot.png" -crop "${WIDTH}x${HEIGHT}+0+0" +repage -strip "$out"
      echo "docs-screenshots: $out after ${waited}s"
      return 0
    fi
  done
  echo "docs-screenshots: the screen never settled for $out within ${TIMEOUT_SECONDS}s" >&2
  return 1
}

shoot() {
  local name=$1 example=${2:-} home=$work/home-$1
  mkdir -p "$home"
  env -u DBUS_SESSION_BUS_ADDRESS HOME="$home" XDG_CONFIG_HOME="$home/config" \
    XDG_DATA_HOME="$home/data" XDG_CACHE_HOME="$home/cache" \
    WEBKIT_DISABLE_DMABUF_RENDERER=1 WEBKIT_DISABLE_COMPOSITING_MODE=1 LIBGL_ALWAYS_SOFTWARE=1 \
    "$BIN" ${example:+"$PWD/$example"} >"$work/$name.log" 2>&1 &
  app_pid=$!
  if ! capture_when_settled "docs/media/$name.png"; then
    cat "$work/$name.log" >&2
    return 1
  fi
  kill "$app_pid"
  wait "$app_pid" 2>/dev/null || true
  app_pid=
}

mkdir -p docs/media
shoot gallery
for example in docs/guide/examples/*.sepx; do
  shoot "$(basename "$example" .sepx)" "$example"
done
