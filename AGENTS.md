# Agent Instructions

## Project

Calcite is a Rust UI library for gpui-ce: Liquid Glass looks, spring motion, controls, menus, panels, and HUD surfaces. The workspace adds app execution helpers, theme tinting, packaging, signed updates, and a showcase and developer preview.

## Workspace

- `crates/calcite`: the glass UI library (components, motion, glass surfaces).
  - `src/components/`: what apps call (controls, forms, menus, hud, layout, notice, overlay...). `src/primitives/`: shared building blocks (glass, interaction, motion, morph, surface, knob...). `theme`, `palette`, `style`, `tokens`, `fonts`, `icons` stay top-level; `lib.rs` re-exports every module at the crate root, so `calcite::controls` keeps working.
- `crates/calcite-exec`: execution layer for gpui views (task slots, activity leases, progress throttling, bounded parallelism).
- `crates/calcite-tint`: dominant-hue theme colors from artwork.
- `crates/calcite-bundle`: packages an app as .app/DMG, NSIS/MSI, deb, AppImage, or pacman (wraps cargo-packager).
- `crates/calcite-update`: signed, delta-capable app updates.
- `crates/calcite-showcase`: the developer preview and home-screen demo binaries.

## Setup

- No bootstrap step. Calcite works as a plain git dependency: `calcite = { git = "...", tag = "..." }` plus `use calcite::gpui;` where a gpui derive or `actions!` is used. A consumer adds no gpui dependency; `calcite` re-exports `gpui` and `platform` (gpui_platform).
- `third_party/gpui-ce` is upstream gpui-ce at `patches/GPUI_CE_REVISION` with `patches/*.patch` applied, reduced to the crates Calcite builds. `third_party/gpui-ce/REVISION` lists the pin and the patch hashes. Never edit `third_party/` by hand: CI rebuilds it with `scripts/vendor-sync.sh` and fails on any difference.
- Never edit a patch by hand. To change gpui-ce, run `just patch-regen init`, commit in the clone `.gpui-ce-work/` (one commit is one numbered patch), then run `just patch-regen`. It exports the patches, proves that they reproduce the clone, and runs `vendor-sync`. `patches/NEW_API.md` has one section per patch.
- `vendor/` is the old checkout, git-ignored. Nothing builds against it.

## Revising the gpui-ce pin

1. Change the commit in `patches/GPUI_CE_REVISION`.
2. Run `just patch-regen init` in a fresh clone (remove `.gpui-ce-work/` first). If a patch does not apply cleanly, resolve it in the clone, then run `just patch-regen`.
3. Run `just vendor-sync`.
4. Run `just check`.
5. Run `just run` on macOS and let the preview run for 8 s.
6. Watch the Linux and Windows CI jobs.
7. On a 120 Hz (ProMotion) display, run `just stats` and check the frame timings.

## Commands

- `just --list`: all recipes.
- `just run [BIN] [ARGS]`: run the developer preview (default) or `home`.
- `just stats [BIN]`: run with frame timings.
- `just check`: format check, clippy, and tests (`scripts/check.sh`).
- `just vendor-sync`: rebuild `third_party/gpui-ce`.
- `just lint`: clippy over the workspace, warnings denied.
- `just test [ARGS]`: `cargo test --workspace`.
- `just format`: `cargo fmt`.
- `just bundle [FORMAT]`: package the developer preview.
- `just patch-regen [init]`: export the gpui-ce patches from the working clone and verify them.

## Conventions

- Rust 2024 edition. Use tabs (width 4) and a 110-column limit (`rustfmt.toml`).
- Source and script files start with an `SPDX-License-Identifier: MPL-2.0` header.
- Format with `cargo fmt` (or `just format`), never `cargo fmt --all`: it also reformats `third_party/gpui-ce` through the path dependencies.
- `rust-version` is in `[workspace.package]`; CI uses that toolchain.
- Record user-visible changes in `CHANGELOG.md` under `## [Unreleased]`. Breaking changes are allowed before 1.0; list them under a Breaking heading.
- Write commit messages in Conventional Commits style (`feat:`, `fix:`), as in `git log`.

## Gotchas

- Frame timings need the `frame-stats` cargo feature and `CALCITE_FRAME_STATS=1`. `just stats` sets both.
- `window.refresh()` invalidates every cached view. Use it only when the whole window must repaint.
- `theme.rs` calls `window.refresh()` on every frame of the 0.48 s theme crossfade on purpose: all colors change in all views, cached ones included.
- Motion lives in `primitives/motion/`: take springs from `motion::tokens` (never raw numbers) and drive values with `motion::spring`, which keeps velocity on retarget; gpui `Animated::set` loses it.
- The cached-view fix lives in `patches/0003-cached-view-fix.patch`. Do not build against an unpatched gpui-ce.
- Only macOS runs locally. Linux and Windows backends are checked only in CI.
