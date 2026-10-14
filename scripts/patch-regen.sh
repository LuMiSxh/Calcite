#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
# Regenerates patches/gpui-ce-glass.patch from the edits in vendor/gpui-ce, then proves that the
# patch reproduces that tree: it applies the patch to a clean archive of the pinned revision and
# compares every regular file with vendor/gpui-ce. The patch file changes only after the proof passes.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REVISION="$(cat "$ROOT/patches/GPUI_CE_REVISION")"
TARGET="$ROOT/vendor/gpui-ce"
PATCH="$ROOT/patches/gpui-ce-glass.patch"

if command -v sha256sum >/dev/null 2>&1; then
	HASH=(sha256sum)
else
	HASH=(shasum -a 256)
fi

# SHA-256 of every regular file under $1, sorted by path. Symlinks, .git and target/ are skipped.
manifest() {
	(cd "$1" && find . -type f -not -path './.git/*' -not -path './target/*' -print0 | sort -z | xargs -0 "${HASH[@]}")
}

if ! git -C "$TARGET" rev-parse --verify --quiet "$REVISION^{commit}" >/dev/null; then
	echo "error: $REVISION is not in vendor/gpui-ce; run scripts/bootstrap.sh first" >&2
	exit 1
fi

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

git -C "$TARGET" diff --no-color --no-ext-diff --src-prefix=a/ --dst-prefix=b/ "$REVISION" >"$WORK/glass.patch"
if [ ! -s "$WORK/glass.patch" ]; then
	echo "error: vendor/gpui-ce has no changes against $REVISION; nothing to regenerate" >&2
	exit 1
fi

# Proof: apply the new patch to a clean tree of the pinned revision, the way bootstrap applies it.
mkdir "$WORK/pristine"
git -C "$TARGET" archive "$REVISION" | tar -x -C "$WORK/pristine"
git -C "$WORK/pristine" apply --check --whitespace=nowarn "$WORK/glass.patch"
git -C "$WORK/pristine" apply --whitespace=nowarn "$WORK/glass.patch"

manifest "$WORK/pristine" >"$WORK/proof.sha256"
manifest "$TARGET" >"$WORK/vendor.sha256"
if ! cmp -s "$WORK/proof.sha256" "$WORK/vendor.sha256"; then
	echo "error: the regenerated patch does not reproduce vendor/gpui-ce; patches/ is unchanged" >&2
	diff "$WORK/vendor.sha256" "$WORK/proof.sha256" >&2 || true
	exit 1
fi

files="$(wc -l <"$WORK/proof.sha256" | tr -d ' ')"
if cmp -s "$WORK/glass.patch" "$PATCH"; then
	echo "patch unchanged: $files files match vendor/gpui-ce"
else
	cp "$WORK/glass.patch" "$PATCH"
	echo "patch updated: $(basename "$PATCH"), $files files match vendor/gpui-ce"
fi
