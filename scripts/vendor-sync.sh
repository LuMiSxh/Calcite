#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
# Rebuilds third_party/gpui-ce from upstream gpui-ce: clones the pinned revision, applies
# patches/*.patch in order, and copies only the crates Calcite builds. Maintainers run it
# on macOS or Linux after they change the pin or a patch. Consumers never run it.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
UPSTREAM="https://github.com/gpui-ce/gpui-ce.git"
REVISION="$(tr -d '[:space:]' <"$ROOT/patches/GPUI_CE_REVISION")"
DEST="$ROOT/third_party/gpui-ce"
# Source directory of upstream to use instead of a network clone (the patch workflow sets it).
SOURCE="${GPUI_CE_SOURCE:-$UPSTREAM}"

# The crates in the dependency graph of calcite on every platform.
KEEP=(
	gpui gpui_apple gpui_ce_parley gpui_ce_util gpui_collections gpui_derive_refineable
	gpui_elements gpui_linux gpui_macos gpui_macros gpui_media gpui_platform gpui_refineable
	gpui_render gpui_scheduler gpui_shared_string gpui_sum_tree gpui_web gpui_wgpu gpui_windows
)
# Directories of a kept crate that no build of Calcite needs.
DROP=(examples tests benches docs target)

if command -v sha256sum >/dev/null 2>&1; then
	HASH=(sha256sum)
else
	HASH=(shasum -a 256)
fi

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

git init --quiet "$WORK/src"
git -C "$WORK/src" fetch --quiet --depth 1 "$SOURCE" "$REVISION"
git -C "$WORK/src" checkout --quiet --detach FETCH_HEAD
if [ "$(git -C "$WORK/src" rev-parse HEAD)" != "$REVISION" ]; then
	echo "error: fetched commit is not $REVISION" >&2
	exit 1
fi

patches=("$ROOT"/patches/*.patch)
for patch in "${patches[@]}"; do
	git -C "$WORK/src" apply --whitespace=nowarn "$patch"
done

OUT="$WORK/out"
mkdir -p "$OUT/crates"
cp "$WORK/src/Cargo.toml" "$OUT/Cargo.toml"
for license in "$WORK"/src/LICENSE* "$WORK"/src/NOTICE*; do
	[ -e "$license" ] && cp "$license" "$OUT/"
done
for crate in "${KEEP[@]}"; do
	cp -R "$WORK/src/crates/$crate" "$OUT/crates/$crate"
	for dir in "${DROP[@]}"; do
		rm -rf "${OUT:?}/crates/$crate/$dir"
	done
done

# Keep the manifests loadable without the dropped files: list only the kept crates as
# members, and remove example, bench, and test targets and path dependencies that point at
# dropped files or crates.
KEEP_LIST="${KEEP[*]}" python3 - "$OUT" <<'PY'
import os, re, sys

out = sys.argv[1]
keep = os.environ["KEEP_LIST"].split()

root = os.path.join(out, "Cargo.toml")
text = open(root).read()
members = "members = [\n" + "".join(f'  "./crates/{c}/",\n' for c in keep) + "]"
text, count = re.subn(r"members = \[.*?\]", members, text, count=1, flags=re.S)
assert count == 1, "members array not found"
text = re.sub(r"default-members = \[.*?\]", 'default-members = ["./crates/gpui/"]', text, count=1, flags=re.S)
open(root, "w").write(text)

TARGET_TABLE = re.compile(r"^\[\[(example|bench|test)\]\]\n(?:(?!\[).*\n?)*", re.M)
for crate in keep:
    manifest = os.path.join(out, "crates", crate, "Cargo.toml")
    base = os.path.dirname(manifest)
    text = open(manifest).read()

    def drop_missing(match):
        path = re.search(r'^path\s*=\s*"([^"]+)"', match.group(0), re.M)
        if path and not os.path.exists(os.path.join(base, path.group(1))):
            return ""
        return match.group(0)

    text = TARGET_TABLE.sub(drop_missing, text)

    def drop_dep(match):
        return "" if not os.path.exists(os.path.join(base, match.group(1))) else match.group(0)

    text = re.sub(r'^[^\n#]*\bpath\s*=\s*"(\.\./[^"]+)"[^\n]*\n', drop_dep, text, flags=re.M)
    text = re.sub(r"\n{3,}", "\n\n", text)
    open(manifest, "w").write(text)
PY

{
	echo "upstream $UPSTREAM"
	echo "revision $REVISION"
	for patch in "${patches[@]}"; do
		"${HASH[@]}" "$patch" | awk -v name="$(basename "$patch")" '{print "patch    " name " sha256 " $1}'
	done
} >"$OUT/REVISION"

rm -rf "$DEST"
mkdir -p "$(dirname "$DEST")"
mv "$OUT" "$DEST"
echo "third_party/gpui-ce is $REVISION plus ${#patches[@]} patches: $(find "$DEST" -type f | wc -l | tr -d ' ') files"
