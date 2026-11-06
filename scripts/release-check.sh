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

# Prints its arguments joined by "; ".
joined() { local s; s=$(printf '%s; ' "$@"); printf '%s' "${s%; }"; }

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

# Published asset names at <version>, one "<os> <need> <name>" per line, from
# the table in scripts/release/assets.sh. <need> is "always" or "updater".
asset_names() {
  awk -v v="$1" '/^ASSETS=/{p=1;next} p&&/^'"'"'/{exit}
    p&&NF==7&&$1!~/^#/{print $1, $5, "Separatrix_" v "_" $7}' scripts/release/assets.sh
}

# Strings each install guide must contain: its first-launch steps and its
# checksum command. A guide missing from this table fails.
guide_strings() {
  case "$1" in
    windows) printf '%s\n' "More info" "Run anyway" "offline" "WebView2" "SHA-256" "Get-FileHash" ;;
    macos) printf '%s\n' "Open Anyway" "Privacy & Security" "xattr -dr com.apple.quarantine" "SHA-256" "shasum -a 256" ;;
    linux) printf '%s\n' "chmod +x" "AppImage" "apt install ./" "dnf install ./" "SHA-256" "sha256sum -c" ;;
  esac
}
GUIDES=(windows macos linux)

# Screenshots only the owner can take, one "<guide>.md|<what it shows>"
# per line: each is a "> **Screenshot to add on a real <machine>:** ..." line.
screenshot_placeholders() {
  grep -HE '^ *> \*\*Screenshot to add on a real [^*]+:\*\* ' docs/install/*.md 2>/dev/null |
    sed -E 's#^docs/install/([a-z]+\.md): *> \*\*[^*]+\*\* #\1|#' | LC_ALL=C sort
}

row_install_guides() {
  local ver os guide names strings name stray=() absent=() n
  ver=$(json app/package.json version)
  names=$(asset_names "$ver")
  if [ -z "$names" ]; then fail install-guides "could not read the asset table in scripts/release/assets.sh"; return; fi
  for os in "${GUIDES[@]}"; do
    guide=docs/install/$os.md
    strings=()
    while IFS= read -r name; do strings+=("$name"); done < <(guide_strings "$os")
    require_strings install-guides "$guide" "${strings[@]}" || return
    while IFS= read -r name; do
      grep -qxF "$name" < <(cut -d' ' -f3 <<<"$names") || stray+=("$guide names $name")
    done < <(grep -oE 'Separatrix_[A-Za-z0-9._-]+' "$guide" | sed -E 's/\.+$//' | LC_ALL=C sort -u)
    while read -r _ _ name; do
      grep -qF "$name" "$guide" || absent+=("$guide does not name $name")
    done < <(grep "^$os always " <<<"$names")
  done
  if [ ${#stray[@]} -gt 0 ]; then
    fail install-guides "not an asset at $ver in scripts/release/assets.sh: $(joined "${stray[@]}")"
    return
  fi
  if [ ${#absent[@]} -gt 0 ]; then fail install-guides "$(joined "${absent[@]}")"; return; fi
  n=$(screenshot_placeholders | grep -c .)
  pass install-guides "${GUIDES[*]} guides name the $ver assets, cover first launch and a checksum command; $n screenshot placeholders"
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

# docs/release-checklist.md is an ordered list of owner steps, each with a
# "Done when" line, and it lists every screenshot placeholder still in the
# install guides as an open "- [ ] [<guide>.md](install/<guide>.md): <what>"
# item. Ticking an item and replacing its placeholder keep the two in step.
row_checklist() {
  local list=docs/release-checklist.md steps undone wanted listed missing extra
  require_strings checklist "$list" \
    "Release workflow" "TAURI_SIGNING_PRIVATE_KEY" "Mac" "Windows" "PyPI" "beta" || return
  read -r steps undone < <(awk '/^[0-9]+\. \[[ x]\] /{if(n&&!d)u++;n++;d=0;next} /Done when/{d=1}
    END{if(n&&!d)u++;print n+0, u+0}' "$list")
  if [ "$steps" -eq 0 ]; then fail checklist "$list has no '1. [ ] ...' steps"; return; fi
  if [ "$undone" -gt 0 ]; then fail checklist "$undone of $steps steps in $list have no 'Done when' line"; return; fi
  wanted=$(screenshot_placeholders)
  listed=$(sed -nE 's#^ *- \[ \] \[([a-z]+\.md)\]\(install/[a-z]+\.md\): #\1|#p' "$list" | LC_ALL=C sort)
  missing=$(LC_ALL=C comm -23 <(printf '%s\n' "$wanted") <(printf '%s\n' "$listed") | grep .)
  extra=$(LC_ALL=C comm -13 <(printf '%s\n' "$wanted") <(printf '%s\n' "$listed") | grep .)
  if [ -n "$missing" ]; then
    fail checklist "$list does not list these screenshot placeholders: $(tr '|\n' ' ;' <<<"$missing" | sed 's/;$//')"
  elif [ -n "$extra" ]; then
    fail checklist "$list lists placeholders the guides no longer have (tick them): $(tr '|\n' ' ;' <<<"$extra" | sed 's/;$//')"
  else
    pass checklist "$steps owner steps, each with 'Done when'; lists all $(grep -c . <<<"$wanted") screenshot placeholders"
  fi
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
  # A claim row ends in PASS or has PASS before an info column. Rows that
  # start "wrong:" are claims each gate requires to FAIL, so they don't count.
  local gate out rc passed claims counts=() failed=()
  for gate in strogatz chaos continuation; do
    out=$(bash "scripts/$gate-gate.sh" 2>&1)
    rc=$?
    passed=$(grep -E ' PASS( |$)' <<<"$out" | grep -vc 'wrong:')
    claims=$((passed + $(grep -E ' FAIL( |$)' <<<"$out" | grep -vc 'wrong:')))
    counts+=("$gate $passed/$claims")
    if [ "$rc" -ne 0 ] || [ "$claims" -eq 0 ] || [ "$passed" -ne "$claims" ]; then
      failed+=("scripts/$gate-gate.sh (exit $rc, $passed/$claims)")
    fi
  done
  if [ ${#failed[@]} -gt 0 ]; then
    fail gates "failed: $(joined "${failed[@]}")"
  else
    pass gates "$(joined "${counts[@]}" | sed 's/; /, /g')"
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
