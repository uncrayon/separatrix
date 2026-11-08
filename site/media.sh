#!/usr/bin/env bash
# Regenerates the web-sized media the site commits, from their sources:
#   docs/media/<view>.png   -> site/media/views/<view>.webp (960 px wide)
#   site/media/separatrix-intro.mp4 -> site/media/separatrix-intro-poster.webp
# and records each source's SHA-256 in site/media/manifest.json. build.mjs reads the
# manifest: when a screenshot in docs/media changes and this script has not been run
# again, the build uses the full-size PNG for that card and prints a warning.
#
#   bash site/media.sh            needs ffmpeg with libwebp
set -euo pipefail
cd "$(dirname "$0")/.."

VIEWS=(phase-plane time-series cobweb 3d-view poincare-section lyapunov orbit-diagram continuation)
OUT=site/media/views
mkdir -p "$OUT"

entries=()
for v in "${VIEWS[@]}"; do
  src=docs/media/$v.png
  ffmpeg -nostdin -y -v error -i "$src" -vf "scale=960:-2:flags=lanczos" \
    -c:v libwebp -quality 82 -compression_level 6 "$OUT/$v.webp"
  sum=$(sha256sum "$src" | cut -d' ' -f1)
  dims=$(ffprobe -v error -select_streams v:0 -show_entries stream=width,height -of csv=p=0 "$OUT/$v.webp")
  entries+=("\"$v\": {\"source\": \"$src\", \"sha256\": \"$sum\", \"file\": \"views/$v.webp\", \"width\": ${dims%,*}, \"height\": ${dims#*,}}")
done

video=site/media/separatrix-intro.mp4
poster=site/media/separatrix-intro-poster.webp
# 30 s in: the phase plane with six trajectories, one dragged across the separatrix.
ffmpeg -nostdin -y -v error -ss 30 -i "$video" -frames:v 1 -vf "scale=1280:-2:flags=lanczos" \
  -c:v libwebp -quality 80 "$poster"
dims=$(ffprobe -v error -select_streams v:0 -show_entries stream=width,height -of csv=p=0 "$video")
entries+=("\"video\": {\"source\": \"$video\", \"sha256\": \"$(sha256sum "$video" | cut -d' ' -f1)\", \"file\": \"separatrix-intro.mp4\", \"poster\": \"separatrix-intro-poster.webp\", \"width\": ${dims%,*}, \"height\": ${dims#*,}}")

{
  echo "{"
  for i in "${!entries[@]}"; do
    sep=","; [ "$i" -eq $((${#entries[@]} - 1)) ] && sep=""
    echo "  ${entries[$i]}$sep"
  done
  echo "}"
} > site/media/manifest.json
echo "wrote $OUT/*.webp, $poster and site/media/manifest.json"
