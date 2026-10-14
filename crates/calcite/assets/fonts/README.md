# Bundled fonts

Calcite ships two families behind the default `fonts` feature. [`calcite::fonts::register`](../../src/fonts.rs)
adds them to the gpui text system, and `calcite::init` calls it. All are licensed under the
SIL Open Font License 1.1; each folder holds its `OFL.txt`.

| Family | Version | Files | Source | Licence |
|---|---|---|---|---|
| Plus Jakarta Sans (UI sans, Latin only) | 2.071 | `plus-jakarta-sans/PlusJakartaSans-Regular.ttf`, `PlusJakartaSans-Medium.ttf`, `PlusJakartaSans-SemiBold.ttf` (static files from `fonts/ttf`) | https://github.com/tokotype/PlusJakartaSans (`fonts/ttf` on master) | SIL OFL 1.1, `plus-jakarta-sans/OFL.txt` |
| Fira Code (mono) | 6.2 | `fira-code/FiraCode-Regular.ttf`, `FiraCode-Bold.ttf` | https://github.com/tonsky/FiraCode/releases/tag/6.2 | SIL OFL 1.1, `fira-code/OFL.txt` |

Total size is about 1 MB. The files are unmodified and not subset.

Use a family by its name: `gpui::font(calcite::fonts::Family::PlusJakartaSans.name())`.
