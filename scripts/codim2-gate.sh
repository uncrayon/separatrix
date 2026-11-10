#!/usr/bin/env bash
set -u
cd "$(dirname "$0")/.."
cargo test -p separatrix --release --no-fail-fast --test codim2 --test homoclinic -- --nocapture --test-threads=1
