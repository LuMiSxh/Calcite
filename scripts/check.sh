#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
# Formats, lints, and tests the Calcite crates. `cargo fmt --all` would also reformat the
# vendored gpui-ce through its path dependencies, so this formats each crate on its own.
set -euo pipefail
cd "$(dirname "$0")/.."
for crate in calcite calcite-exec calcite-tint calcite-bundle calcite-update calcite-showcase; do
	cargo fmt -p "$crate"
done
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
