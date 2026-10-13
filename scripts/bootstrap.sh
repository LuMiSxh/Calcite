#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
# Fetches gpui-ce at the pinned commit into vendor/gpui-ce and applies the glass patch.
# Safe to run again: an existing checkout is reset to the pinned commit first.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REPOSITORY="https://github.com/gpui-ce/gpui-ce.git"
REVISION="$(cat "$ROOT/patches/GPUI_CE_REVISION")"
TARGET="$ROOT/vendor/gpui-ce"

if [ ! -d "$TARGET/.git" ]; then
	mkdir -p "$ROOT/vendor"
	git clone --filter=blob:none "$REPOSITORY" "$TARGET"
fi
git -C "$TARGET" fetch --quiet origin "$REVISION"
git -C "$TARGET" checkout --quiet --force "$REVISION"
git -C "$TARGET" clean --quiet -fd
for patch in "$ROOT"/patches/*.patch; do
	git -C "$TARGET" apply --whitespace=nowarn "$patch"
	echo "applied $(basename "$patch")"
done
echo "gpui-ce $REVISION is ready in vendor/gpui-ce"
