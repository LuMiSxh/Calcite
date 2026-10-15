# SPDX-License-Identifier: MPL-2.0

set shell := ["bash", "-euo", "pipefail", "-c"]

# List the available commands.
default:
    @just --list

# Fetch gpui-ce and apply the glass patch; rerun after patches/ changes.
[group('Setup')]
bootstrap:
    ./scripts/bootstrap.sh

# Regenerate patches/gpui-ce-glass.patch from vendor/gpui-ce, then prove it reproduces that tree.
[group('Setup')]
patch-regen:
    ./scripts/patch-regen.sh

# Run the developer preview (default) or the home screen; extra arguments go to the binary, e.g. an artwork image for home.
[group('Development')]
run bin='preview' *args='':
    cargo run --release --bin {{ quote(bin) }} -- {{ args }}

# Run the developer preview (default) or the home screen with frame timings printed once a second.
[group('Development')]
stats bin='preview':
    CALCITE_FRAME_STATS=1 cargo run --release --features frame-stats --bin {{ quote(bin) }}

# Format, lint, and test the Calcite crates.
[group('Checks')]
check:
    ./scripts/check.sh

# Run the tests of the whole workspace.
[group('Checks')]
test *args='':
    cargo test --workspace {{ args }}

# Lint the whole workspace with warnings denied.
[group('Checks')]
lint:
    cargo clippy --workspace --all-targets -- -D warnings

# Format each Calcite crate on its own, so the vendored gpui-ce stays untouched.
[group('Checks')]
format:
    #!/usr/bin/env bash
    set -euo pipefail
    for crate in calcite calcite-exec calcite-tint calcite-bundle calcite-update calcite-showcase; do
        cargo fmt -p "$crate"
    done

# Package the developer preview ("Calcite - preview") as app, dmg, nsis, wix (MSI), exe, deb, appimage, or standalone (the host's single file); default: the host's native formats.
[group('Packaging')]
bundle format='' *args='':
    cargo run -p calcite-bundle -- calcite bundle -p calcite-showcase --bin preview {{ if format != '' { '--format ' + quote(format) } else { '' } }} {{ args }}
