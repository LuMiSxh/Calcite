#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
# Checks format, lints, and tests the Calcite crates. `just format` fixes the format.
# `cargo fmt` formats workspace members only; third_party/gpui-ce is a separate workspace.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
