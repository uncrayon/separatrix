#!/usr/bin/env bash
#   bash scripts/release/manifest.sh <dir> <version> <owner/repo>
set -euo pipefail
export LC_ALL=C

if [ $# -ne 3 ]; then
  echo "usage: $0 <dir> <version> <owner/repo>" >&2
  exit 2
fi
dir=$1 ver=$2 repo=$3
here=$(cd "$(dirname "$0")" && pwd)
cd "$dir"
rm -f SHA256SUMS.txt latest.json.tmp SHA256SUMS.txt.tmp

PLATFORMS='
linux-x86_64    linux_x86_64.AppImage
windows-x86_64  windows_x64-setup.exe
darwin-x86_64   macos_universal.app.tar.gz
darwin-aarch64  macos_universal.app.tar.gz
'

sigs=$(find . -maxdepth 1 -name '*.sig' | wc -l | tr -d ' ')
if [ "$sigs" -eq 0 ]; then
  rm -f latest.json
else
  epoch=${SOURCE_DATE_EPOCH:-$(git -C "$here" log -1 --format=%ct)}
  pub_date=$(date -u -d "@$epoch" +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || date -u -r "$epoch" +%Y-%m-%dT%H:%M:%SZ)
  notes="Separatrix $ver. See https://github.com/$repo/releases/tag/v$ver for what changed."
  {
    printf '{\n  "version": "%s",\n  "notes": "%s",\n  "pub_date": "%s",\n  "platforms": {' "$ver" "$notes" "$pub_date"
    sep=''
    while read -r platform suffix; do
      [ -n "$platform" ] || continue
      name="Separatrix_${ver}_${suffix}"
      if [ ! -f "$name" ] || [ ! -f "$name.sig" ]; then
        echo "error: signing is on, but $name or $name.sig is missing for $platform" >&2
        exit 1
      fi
      signature=$(tr -d '\r\n' <"$name.sig")
      case "$signature" in
        ''|*[!A-Za-z0-9+/=]*) echo "error: $name.sig is not a base64 signature" >&2; exit 1 ;;
      esac
      printf '%s\n    "%s": {\n      "signature": "%s",\n      "url": "https://github.com/%s/releases/download/v%s/%s"\n    }' \
        "$sep" "$platform" "$signature" "$repo" "$ver" "$name"
      sep=','
    done <<EOF
$PLATFORMS
EOF
    printf '\n  }\n}\n'
  } >latest.json.tmp
  mv latest.json.tmp latest.json
fi

files=()
for f in *; do
  if [ -f "$f" ] && [ "$f" != SHA256SUMS.txt ]; then files+=("$f"); fi
done
if [ ${#files[@]} -eq 0 ]; then
  echo "error: $dir holds no assets" >&2
  exit 1
fi
sha256sum -- "${files[@]}" >SHA256SUMS.txt.tmp
mv SHA256SUMS.txt.tmp SHA256SUMS.txt
