#!/usr/bin/env bash
#   bash scripts/release/assets.sh <out-dir> <os> <arch> [<variant>]
#   bash scripts/release/assets.sh --version     print the version cargo metadata reports
#
# Runs on Linux, macOS (bash 3.2) and Windows (Git Bash) from any directory.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)

table=$root/scripts/release/assets.txt

version() {
  (cd "$root" && cargo metadata --no-deps --format-version 1) |
    node -e 'let s="";process.stdin.on("data",d=>s+=d).on("end",()=>{const p=JSON.parse(s).packages.find(p=>p.name==="separatrix");if(!p)process.exit(1);process.stdout.write(p.version)})'
}

if [ "${1:-}" = "--version" ]; then version; echo; exit 0; fi
if [ $# -lt 3 ] || [ $# -gt 4 ]; then
  echo "usage: $0 <out-dir> <os> <arch> [<variant>] | --version" >&2
  exit 2
fi
out=$1 os=$2 arch=$3 variant=${4:--}
ver=$(version)
mkdir -p "$out"

copied=0
while read -r r_os r_arch r_variant r_target r_need r_path r_suffix _; do
  [ "$r_os $r_arch $r_variant" = "$os $arch $variant" ] || continue
  dir="$root/target/release/bundle"
  [ "$r_target" = "-" ] || dir="$root/target/$r_target/release/bundle"
  src="$dir/${r_path//@V@/$ver}"
  if [ ! -f "$src" ]; then
    [ "$r_need" = "optional" ] && continue
    echo "error: $src is missing; the bundle step did not produce it" >&2
    exit 1
  fi
  name="Separatrix_${ver}_${r_suffix}"
  cp "$src" "$out/$name"
  echo "$name"
  if [ -f "$src.sig" ]; then cp "$src.sig" "$out/$name.sig"; echo "$name.sig"; fi
  copied=$((copied + 1))
done < <(tail -n +2 "$table")

if [ "$copied" -eq 0 ]; then
  echo "error: no asset row matches os=$os arch=$arch variant=$variant" >&2
  exit 1
fi
