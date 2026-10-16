#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
# Maintains patches/*.patch as commits in a working clone of gpui-ce (default .gpui-ce-work/,
# git-ignored; set GPUI_CE_WORK to move it). One commit is one numbered patch.
#
#   patch-regen.sh init    Clone upstream at the pinned revision and apply the patches as commits.
#   patch-regen.sh         Export the commits to patches/, prove that they reproduce the working
#                          tree, then run scripts/vendor-sync.sh.
#
# To change gpui-ce: run `init`, commit your edits in the clone (one commit per feature, or
# `git commit --fixup`), then run this script without arguments.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
UPSTREAM="https://github.com/gpui-ce/gpui-ce.git"
REVISION="$(tr -d '[:space:]' <"$ROOT/patches/GPUI_CE_REVISION")"
WORK="${GPUI_CE_WORK:-$ROOT/.gpui-ce-work}"

if [ "${1:-}" = "init" ]; then
	if [ -e "$WORK" ]; then
		echo "error: $WORK exists; remove it first" >&2
		exit 1
	fi
	git clone --quiet --filter=blob:none "$UPSTREAM" "$WORK"
	git -C "$WORK" checkout --quiet -b calcite "$REVISION"
	for patch in "$ROOT"/patches/*.patch; do
		git -C "$WORK" am --quiet "$patch"
		echo "applied $(basename "$patch")"
	done
	echo "gpui-ce $REVISION plus the patches is ready in $WORK"
	exit 0
fi

if [ ! -d "$WORK/.git" ]; then
	echo "error: $WORK is missing; run scripts/patch-regen.sh init first" >&2
	exit 1
fi
if [ -n "$(git -C "$WORK" status --porcelain)" ]; then
	echo "error: $WORK has uncommitted changes; commit them first" >&2
	exit 1
fi
if [ -z "$(git -C "$WORK" rev-list "$REVISION..HEAD")" ]; then
	echo "error: $WORK has no commits on top of $REVISION; nothing to export" >&2
	exit 1
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

git -C "$WORK" format-patch --quiet --zero-commit --no-signature --no-stat -o "$TMP/patches" "$REVISION..HEAD"

# Proof: the exported series applied to a clean archive of the pinned revision gives the
# tree of the working clone.
mkdir "$TMP/pristine"
git -C "$WORK" archive "$REVISION" | tar -x -C "$TMP/pristine"
git -C "$TMP/pristine" init --quiet
for patch in "$TMP"/patches/*.patch; do
	git -C "$TMP/pristine" apply --whitespace=nowarn "$patch"
done
git -C "$TMP/pristine" add -A -f
if [ "$(git -C "$TMP/pristine" write-tree)" != "$(git -C "$WORK" rev-parse 'HEAD^{tree}')" ]; then
	echo "error: the exported patches do not reproduce $WORK; patches/ is unchanged" >&2
	exit 1
fi

# A patch keeps the file name of its number; a new number takes the name of its subject.
mkdir "$TMP/named"
for patch in "$TMP"/patches/*.patch; do
	number="$(basename "$patch" | cut -d- -f1)"
	existing=("$ROOT"/patches/"$number"-*.patch)
	if [ -e "${existing[0]}" ]; then
		cp "$patch" "$TMP/named/$(basename "${existing[0]}")"
	else
		cp "$patch" "$TMP/named/"
	fi
done
rm -f "$ROOT"/patches/*.patch
cp "$TMP"/named/*.patch "$ROOT/patches/"
echo "patches updated: $(ls "$ROOT"/patches/*.patch | wc -l | tr -d ' ') files"
"$ROOT/scripts/vendor-sync.sh"
