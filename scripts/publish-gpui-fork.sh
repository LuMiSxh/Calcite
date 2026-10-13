#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
# Pushes the patched gpui-ce to a fork, so Calcite works as a Git dependency.
#
# Usage: scripts/publish-gpui-fork.sh REMOTE_URL [BRANCH]
#   REMOTE_URL  An empty fork of gpui-ce, for example https://github.com/LuMiSxh/gpui-ce.git
#   BRANCH      The branch to push (default: calcite)
#
# The script commits patches/*.patch on top of the pinned revision in a temporary clone and
# pushes that commit. It prints the commit to pin in Cargo.toml.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REMOTE="${1:?usage: publish-gpui-fork.sh REMOTE_URL [BRANCH]}"
BRANCH="${2:-calcite}"
UPSTREAM="https://github.com/gpui-ce/gpui-ce.git"
REVISION="$(cat "$ROOT/patches/GPUI_CE_REVISION")"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

git clone --quiet --filter=blob:none "$UPSTREAM" "$WORK/gpui-ce"
git -C "$WORK/gpui-ce" checkout --quiet -b "$BRANCH" "$REVISION"
for patch in "$ROOT"/patches/*.patch; do
	git -C "$WORK/gpui-ce" apply --whitespace=nowarn "$patch"
done
git -C "$WORK/gpui-ce" add -A
git -C "$WORK/gpui-ce" commit --quiet -m "feat(render): add the Liquid Glass backdrop filter"
git -C "$WORK/gpui-ce" push "$REMOTE" "$BRANCH"
echo "Pin this commit in Cargo.toml: rev = \"$(git -C "$WORK/gpui-ce" rev-parse HEAD)\""
