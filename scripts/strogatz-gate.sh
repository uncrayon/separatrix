#!/usr/bin/env bash
# Prints the Strogatz chapters 2–8 validation table and exits with the test's status.
set -u
cd "$(dirname "$0")/.."
cargo test -p separatrix --release --test strogatz -- --nocapture --test-threads=1
