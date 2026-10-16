# SPDX-License-Identifier: MPL-2.0

set shell := ["bash", "-euo", "pipefail", "-c"]

# List the available commands.
default:
    @just --list

# Rebuild third_party/gpui-ce from upstream gpui-ce and patches/ (maintainers; needs network).
[group('Setup')]
vendor-sync:
    ./scripts/vendor-sync.sh

# Edit the gpui-ce patches: `just patch-regen init` makes a working clone, plain `just patch-regen` exports its commits.
[group('Setup')]
patch-regen *args='':
    ./scripts/patch-regen.sh {{ args }}

# Run the developer preview (default) or the home screen; extra arguments go to the binary, e.g. an artwork image for home.
[group('Development')]
run bin='preview' *args='':
    cargo run --release --bin {{ quote(bin) }} -- {{ args }}

# Run the developer preview (default) or the home screen with frame timings printed once a second.
[group('Development')]
stats bin='preview':
    CALCITE_FRAME_STATS=1 cargo run --release --features frame-stats --bin {{ quote(bin) }}

# Check the format, lint, and test the Calcite crates.
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

# Format the Calcite crates (the vendored gpui-ce is a separate workspace and stays untouched).
[group('Checks')]
format:
    cargo fmt

# Package the developer preview ("Calcite - preview") as app, dmg, nsis, wix (MSI), exe, deb, appimage, or standalone (the host's single file); default: the host's native formats.
[group('Packaging')]
bundle format='' *args='':
    cargo run -p calcite-bundle -- calcite bundle -p calcite-showcase --bin preview {{ if format != '' { '--format ' + quote(format) } else { '' } }} {{ args }}
