#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
# Formats, lints, and tests the Vitro crates. `cargo fmt --all` would also reformat the
# vendored gpui-ce through its path dependencies, so this formats each crate on its own.
set -euo pipefail
cd "$(dirname "$0")/.."
for crate in calcite vitro-exec vitro-showcase; do
	cargo fmt -p "$crate"
done
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
