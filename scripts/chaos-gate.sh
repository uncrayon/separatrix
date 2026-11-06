#!/usr/bin/env bash
set -u
cd "$(dirname "$0")/.."
cargo test --workspace --release --test chaos -- --nocapture --test-threads=1
