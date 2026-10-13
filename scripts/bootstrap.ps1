# SPDX-License-Identifier: MPL-2.0
# Fetches gpui-ce at the pinned commit into vendor/gpui-ce and applies the glass patch.
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$Revision = (Get-Content "$Root/patches/GPUI_CE_REVISION").Trim()
$Target = "$Root/vendor/gpui-ce"
if (-not (Test-Path "$Target/.git")) {
	New-Item -ItemType Directory -Force "$Root/vendor" | Out-Null
	git clone --filter=blob:none https://github.com/gpui-ce/gpui-ce.git $Target
}
git -C $Target fetch --quiet origin $Revision
git -C $Target checkout --quiet --force $Revision
git -C $Target clean --quiet -fd
Get-ChildItem "$Root/patches/*.patch" | ForEach-Object {
	git -C $Target apply --whitespace=nowarn $_.FullName
	Write-Host "applied $($_.Name)"
}
Write-Host "gpui-ce $Revision is ready in vendor/gpui-ce"
