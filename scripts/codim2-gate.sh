#!/usr/bin/env bash
set -u
cd "$(dirname "$0")/.."
cargo test -p separatrix --release --test codim2 -- --nocapture --test-threads=1
