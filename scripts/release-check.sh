#!/usr/bin/env bash
# Is v1 ready to release? Each row prints PASS or FAIL with what it found or
# what is missing; the exit code is the number of FAIL rows.
#
#   bash scripts/release-check.sh               every row, gates included
#   bash scripts/release-check.sh --skip-gates  skip the slow numerical gates
set -u
cd "$(dirname "$0")/.." || exit 2

SKIP_GATES=0
for arg in "$@"; do
  case "$arg" in
    --skip-gates) SKIP_GATES=1 ;;
    *) echo "usage: $0 [--skip-gates]" >&2; exit 2 ;;
  esac
done

FAILS=0
pass() { printf 'PASS  %s: %s\n' "$1" "$2"; }
fail() { printf 'FAIL  %s: %s\n' "$1" "$2"; FAILS=$((FAILS + 1)); }

# Prints the value at a property path of a JSON file, or nothing when absent.
json() { node -e 'let v=JSON.parse(require("fs").readFileSync(process.argv[1],"utf8"));for(const k of process.argv.slice(2)){if(v==null||typeof v!=="object"||!(k in v)){v=undefined;break}v=v[k]}if(v!==undefined)process.stdout.write(typeof v==="string"?v:JSON.stringify(v))' "$@"; }

# Fails <row>, naming <file> and every required string it lacks.
# Usage: require_strings <row> <file> <string>...
require_strings() {
  local row=$1 file=$2 s missing=()
  shift 2
  if [ ! -s "$file" ]; then fail "$row" "$file is missing"; return 1; fi
  for s in "$@"; do grep -qiF -- "$s" "$file" || missing+=("\"$s\""); done
  if [ ${#missing[@]} -gt 0 ]; then fail "$row" "$file does not mention ${missing[*]}"; return 1; fi
}

row_versions() {
  local cargo pkg lock lock_root tauri
  cargo=$(awk '/^\[workspace\.package\]/{p=1;next} /^\[/{p=0} p&&/^version *=/{gsub(/.*= *"|".*/,"");print;exit}' Cargo.toml)
  pkg=$(json app/package.json version)
  lock=$(json app/package-lock.json version)
  lock_root=$(json app/package-lock.json packages "" version)
  tauri=$(json app/src-tauri/tauri.conf.json version)
  if [ -z "$cargo" ]; then fail versions "Cargo.toml has no [workspace.package] version"; return; fi
  if ! [[ "$cargo" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    fail versions "Cargo.toml version $cargo is not MAJOR.MINOR.PATCH (MSI rejects suffixes)"; return
  fi
  local bad=()
  [ "$pkg" = "$cargo" ] || bad+=("app/package.json is ${pkg:-missing}")
  [ "$lock" = "$cargo" ] || bad+=("app/package-lock.json is ${lock:-missing}")
  [ "$lock_root" = "$cargo" ] || bad+=("app/package-lock.json packages[\"\"] is ${lock_root:-missing}")
  [ -z "$tauri" ] || bad+=("app/src-tauri/tauri.conf.json sets $tauri (remove it)")
  if [ ${#bad[@]} -gt 0 ]; then
    local list
    list=$(printf '%s; ' "${bad[@]}")
    fail versions "Cargo.toml is $cargo, but ${list%; }"
  else
    pass versions "$cargo in Cargo, package.json, package-lock.json; tauri.conf.json inherits"
  fi
}

# Every kind in the View union of app/src/state/model.ts, plus the time
# series (a plane view with time on an axis), needs a guide page. A kind
# missing from this table fails, so a new view cannot ship without one.
# (A case table, not an associative array: macOS still ships bash 3.2.)
view_page() {
  case "$1" in
    plane) echo phase-plane.md ;;
    timeseries) echo time-series.md ;;
    cobweb) echo cobweb.md ;;
    section) echo poincare-section.md ;;
    space) echo 3d-view.md ;;
    lyapunov) echo lyapunov.md ;;
    orbit) echo orbit-diagram.md ;;
    continuation) echo continuation.md ;;
  esac
}
GUIDE_TOPICS=(getting-started.md equations.md exports.md project-files.md projector-mode.md keyboard-shortcuts.md)

view_kinds() {
  local model=app/src/state/model.ts member
  awk '/^export type View =/{p=1;next} p{print} p&&/;$/{exit}' "$model" | sed -n 's/^ *| *//p' |
    while read -r member; do
      if [[ ! "$member" =~ kind ]]; then
        member=$(awk -v n="${member%;}" '$0 ~ "^export type " n " =" {p=1} p{print} p&&/;$/{exit}' "$model")
      fi
      grep -oE 'kind\??: "[a-z]+"' <<<"$member" | head -1 | sed -E 's/.*"([a-z]+)"/\1/'
    done
  echo timeseries
}

row_view_docs() {
  local kinds n kind page missing=() unmapped=()
  kinds=$(view_kinds)
  n=$(wc -w <<<"$kinds" | tr -d ' ')
  if [ "$n" -lt 2 ]; then fail view-docs "could not read the View union in app/src/state/model.ts"; return; fi
  for kind in $kinds; do
    page=$(view_page "$kind")
    if [ -z "$page" ]; then unmapped+=("$kind"); continue; fi
    [ -s "docs/guide/$page" ] || missing+=("docs/guide/$page")
  done
  for page in "${GUIDE_TOPICS[@]}"; do [ -s "docs/guide/$page" ] || missing+=("docs/guide/$page"); done
  if [ ${#unmapped[@]} -gt 0 ]; then
    fail view-docs "view kind(s) ${unmapped[*]} have no page in view_page() in scripts/release-check.sh"
  elif [ ${#missing[@]} -gt 0 ]; then
    fail view-docs "missing ${missing[*]}"
  else
    pass view-docs "$n views (${kinds//$'\n'/, }) and ${#GUIDE_TOPICS[@]} topics have pages in docs/guide"
  fi
}

row_install_guides() {
  require_strings install-guides docs/install/windows.md "More info" "Run anyway" "WebView2" "SHA-256" || return
  require_strings install-guides docs/install/macos.md "Open Anyway" "Privacy & Security" "xattr" "SHA-256" || return
  require_strings install-guides docs/install/linux.md "chmod +x" "AppImage" ".deb" ".rpm" "SHA-256" || return
  pass install-guides "windows.md, macos.md and linux.md cover first launch and SHA-256 checks"
}

row_changelog() {
  local v
  v=$(json app/package.json version)
  if grep -qE "^## \[${v//./\\.}\]" CHANGELOG.md; then
    pass changelog "CHANGELOG.md has a [$v] section"
  else
    fail changelog "CHANGELOG.md has no '## [$v]' section"
  fi
}

row_release_workflow() {
  require_strings release-workflow .github/workflows/release.yml \
    "sha256sum" "SHA256SUMS" "offlineInstaller" "ubuntu-22.04" "universal-apple-darwin" || return
  pass release-workflow "release.yml attaches SHA256SUMS and builds the WebView2 offline installer"
}

# The updater is wired, and it stays off until the owner configures a real
# minisign public key: no placeholder key ships, and no updater artifacts are
# requested (they need TAURI_SIGNING_PRIVATE_KEY) while there is no key.
row_updater_off_without_key() {
  local conf=app/src-tauri/tauri.conf.json pubkey artifacts decoded
  if ! grep -qE '^tauri-plugin-updater *=' app/src-tauri/Cargo.toml; then
    fail updater-off-without-key "tauri-plugin-updater is not a dependency in app/src-tauri/Cargo.toml"; return
  fi
  pubkey=$(json "$conf" plugins updater pubkey)
  artifacts=$(json "$conf" bundle createUpdaterArtifacts)
  if [ -z "$pubkey" ]; then
    if [ -n "$artifacts" ] && [ "$artifacts" != "false" ]; then
      fail updater-off-without-key "$conf sets bundle.createUpdaterArtifacts=$artifacts without a pubkey"; return
    fi
    pass updater-off-without-key "tauri-plugin-updater is wired; no pubkey, so the updater is off"
    return
  fi
  decoded=$(printf '%s' "$pubkey" | base64 -d 2>/dev/null | head -1)
  if [[ "$decoded" == "untrusted comment: minisign public key"* ]]; then
    pass updater-off-without-key "a minisign pubkey is configured, so the updater is on"
  else
    fail updater-off-without-key "$conf plugins.updater.pubkey is not a minisign public key (placeholder?)"
  fi
}

row_checklist() {
  require_strings checklist docs/release-checklist.md \
    "Release workflow" "TAURI_SIGNING_PRIVATE_KEY" "Mac" "Windows" "PyPI" "beta" || return
  pass checklist "docs/release-checklist.md lists the owner-only steps"
}

# Every relative link and image in the top-level docs and docs/ resolves, and
# the README links the install guides, the user guide and CONTRIBUTING.md.
row_docs_links() {
  local f target path broken=() unlinked=()
  for f in README.md CONTRIBUTING.md CHANGELOG.md docs/*.md docs/*/*.md; do
    [ -f "$f" ] || continue
    while IFS= read -r target; do
      target=${target%%#*}
      target=${target%% *}
      case "$target" in ''|http://*|https://*|mailto:*) continue ;; esac
      path="$(dirname "$f")/$target"
      [ -e "$path" ] || broken+=("$f -> $target")
    done < <(grep -oE '\]\([^)]+\)' "$f" | sed -E 's/^\]\(//; s/\)$//; s/^<//; s/>$//')
  done
  for target in docs/install/windows.md docs/install/macos.md docs/install/linux.md docs/guide/ CONTRIBUTING.md; do
    grep -qF "]($target" README.md 2>/dev/null || unlinked+=("$target")
  done
  if [ ${#broken[@]} -gt 0 ]; then
    fail docs-links "broken link(s): ${broken[*]}"
  elif [ ${#unlinked[@]} -gt 0 ]; then
    fail docs-links "README.md does not link ${unlinked[*]}"
  else
    pass docs-links "every relative link resolves; README links the install guides, the guide and CONTRIBUTING.md"
  fi
}

row_gates() {
  if [ "$SKIP_GATES" = 1 ]; then printf 'SKIP  gates: --skip-gates\n'; return; fi
  local gate failed=()
  for gate in strogatz chaos continuation; do
    bash "scripts/$gate-gate.sh" >/dev/null 2>&1 || failed+=("scripts/$gate-gate.sh")
  done
  if [ ${#failed[@]} -gt 0 ]; then
    fail gates "failed: ${failed[*]}"
  else
    pass gates "strogatz, chaos and continuation gates pass"
  fi
}

row_versions
row_view_docs
row_install_guides
row_changelog
row_release_workflow
row_updater_off_without_key
row_checklist
row_docs_links
row_gates

exit "$FAILS"
