#!/usr/bin/env bash
# Build the Linux installers (.deb, .rpm and, when Docker runs natively, the AppImage) in an
# Ubuntu 22.04 container, the same OS the Release workflow uses, from macOS or any machine with
# Docker.
#
#   bash scripts/build/linux-in-docker.sh            x86_64 packages, like the release
#   ARCH=arm64 bash scripts/build/linux-in-docker.sh  arm64 packages, native on Apple silicon
#
# The packages land in dist/linux/. The container works on a copy of the repository, so your
# own target/ and app/node_modules are never touched. They are unsigned for the updater,
# like any local build (see docs/building.md).
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
arch=${ARCH:-amd64}
out=$root/dist/linux
mkdir -p "$out"

# The AppImage tools are AppImages themselves and don't run under emulation (an x86_64 container
# on Apple silicon), so the AppImage is built only when the container runs natively.
case "$(docker info --format '{{.Architecture}}')" in
  x86_64 | amd64) host=amd64 ;;
  aarch64 | arm64) host=arm64 ;;
  *) host=unknown ;;
esac
bundles=deb,rpm
if [ "$host" = "$arch" ]; then
  bundles=deb,rpm,appimage
else
  echo "Building $arch under emulation on an $host host: .deb and .rpm only, no AppImage." >&2
fi

# Named volumes keep the toolchain, the crates and the build between runs, one set per arch.
docker run --rm --platform "linux/$arch" \
  -v "$root":/src:ro -v "$out":/out \
  -v "separatrix-linux-$arch-cargo":/root/.cargo -v "separatrix-linux-$arch-rustup":/root/.rustup \
  -v "separatrix-linux-$arch-target":/build/target \
  -e APPIMAGE_EXTRACT_AND_RUN=1 -e NO_STRIP=true -e BUNDLES="$bundles" \
  ubuntu:22.04 bash -euo pipefail -c '
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -qq
    apt-get install -y -qq curl ca-certificates build-essential file rpm patchelf xdg-utils \
      libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev libayatana-appindicator3-dev libssl-dev >/dev/null
    curl -fsSL https://deb.nodesource.com/setup_22.x | bash - >/dev/null
    apt-get install -y -qq nodejs >/dev/null
    [ -x "$HOME/.cargo/bin/cargo" ] || curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal >/dev/null
    . "$HOME/.cargo/env"
    mkdir -p /build
    tar -C /src --exclude=./target --exclude=./dist --exclude=./app/node_modules -cf - . | tar -C /build -xf -
    cd /build/app
    npm ci --no-audit --no-fund
    npx tauri build --bundles "$BUNDLES" --config "{\"bundle\":{\"createUpdaterArtifacts\":false}}"
    find /build/target/release/bundle \( -name "*.deb" -o -name "*.rpm" -o -name "*.AppImage" \) -exec cp {} /out/ \;
  '
ls -lh "$out"
