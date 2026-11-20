#!/bin/bash
# Installs rustup nightly + rust-src in WSL (once).
set -e
if ! command -v ~/.cargo/bin/rustup >/dev/null; then
  curl -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain nightly --profile minimal -c rust-src
fi
. ~/.cargo/env
rustup toolchain list
rustc --version
rustc --print target-list | grep -i vita || echo "no vita target in rustc"
