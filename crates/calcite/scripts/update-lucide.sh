#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
#
# Vendors the Lucide icon set (ISC) into assets/lucide.
# Usage: scripts/update-lucide.sh [VERSION]
# Without VERSION, it uses the version in assets/lucide/VERSION.
set -euo pipefail

crate_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
target="$crate_dir/assets/lucide"
version="${1:-$(cat "$target/VERSION")}"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

metadata="$(curl -fsSL "https://registry.npmjs.org/lucide-static/$version")"
tarball="$(printf '%s' "$metadata" | python3 -c 'import json,sys; print(json.load(sys.stdin)["dist"]["tarball"])')"
integrity="$(printf '%s' "$metadata" | python3 -c 'import json,sys; print(json.load(sys.stdin)["dist"]["integrity"])')"

curl -fsSL "$tarball" -o "$work/lucide.tgz"
actual="sha512-$(openssl dgst -sha512 -binary "$work/lucide.tgz" | base64 | tr -d '\n')"
if [[ "$actual" != "$integrity" ]]; then
	echo "Integrity check failed for lucide-static $version" >&2
	exit 1
fi

mkdir "$work/extract"
tar -xzf "$work/lucide.tgz" -C "$work/extract"
rm -rf "$target"
mkdir -p "$target"
cp "$work/extract/package/LICENSE" "$target/LICENSE"
printf '%s\n' "$version" > "$target/VERSION"

# Collapse whitespace; keep the license comment of each file.
python3 - "$work/extract/package/icons" "$target" <<'PY'
import pathlib, re, sys
source, target = map(pathlib.Path, sys.argv[1:])
for icon in sorted(source.glob("*.svg")):
	text = re.sub(r"\s+", " ", icon.read_text()).replace("> <", "><").replace(" />", "/>").strip()
	text = re.sub(r' class="[^"]*"', "", text)
	(target / icon.name).write_text(text + "\n")
PY

# The built-in Icon set: semantic names over Lucide glyphs. Transport glyphs are
# filled, like the SF Symbols the launcher uses.
icons="$crate_dir/assets/icons"
rm -rf "$icons"
mkdir -p "$icons"
while read -r name lucide filled; do
	if [[ "$filled" == "filled" ]]; then
		sed 's/fill="none"/fill="currentColor"/' "$target/$lucide.svg" > "$icons/$name.svg"
	else
		cp "$target/$lucide.svg" "$icons/$name.svg"
	fi
done <<'MAP'
arrow-clockwise rotate-cw
arrow-down arrow-down
checkmark check
chevron-down chevron-down
chevron-right chevron-right
chevron-up chevron-up
chevron-up-down chevrons-up-down
document file-text
folder folder
forward fast-forward filled
gear settings
globe globe
info info
magnifier search
music music
pause pause filled
play play filled
sparkles sparkles
stop square filled
tag tag
warning triangle-alert
xmark x
MAP
echo "Vendored $(ls "$target"/*.svg | wc -l) Lucide $version icons into $target"
