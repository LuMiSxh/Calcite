# Agent Instructions

## Project

Calcite is a Rust UI library for gpui-ce: Liquid Glass looks, spring motion, controls, menus, panels, and HUD surfaces. The workspace adds app execution helpers, theme tinting, packaging, signed updates, and a showcase gallery.

## Workspace

- `crates/calcite`: the glass UI library (components, motion, glass surfaces).
  - `src/components/`: what apps call (controls, forms, menus, hud, layout, notice, overlay...). `src/primitives/`: shared building blocks (glass, interaction, motion, morph, surface, knob...). `theme`, `palette`, `style`, `tokens`, `fonts`, `icons` stay top-level; `lib.rs` re-exports every module at the crate root, so `calcite::controls` keeps working.
- `crates/calcite-exec`: execution layer for gpui views (task slots, activity leases, progress throttling, bounded parallelism).
- `crates/calcite-tint`: dominant-hue theme colors from artwork.
- `crates/calcite-bundle`: packages an app as .app/DMG, NSIS/MSI, deb, AppImage, or pacman (wraps cargo-packager).
- `crates/calcite-update`: signed, delta-capable app updates.
- `crates/calcite-showcase`: the gallery and home-screen demo binaries.

## Setup

- Run `just bootstrap` before building. It clones gpui-ce at the revision in `patches/GPUI_CE_REVISION` into `vendor/gpui-ce` (git-ignored) and applies `patches/gpui-ce-glass.patch`.
- `bootstrap` resets `vendor/gpui-ce` to the pinned revision. Uncommitted edits there are lost unless they are in the patch.
- Never edit the patch by hand. To change gpui-ce, edit `vendor/gpui-ce`, then run `just patch-regen`. It regenerates the patch and proves that the patch reproduces `vendor/gpui-ce`.

## Commands

- `just --list`: all recipes.
- `just run [BIN] [ARGS]`: run the gallery (default) or `home`.
- `just stats [BIN]`: run with frame timings.
- `just check`: format, clippy, and tests (`scripts/check.sh`).
- `just lint`: clippy over the workspace, warnings denied.
- `just test [ARGS]`: `cargo test --workspace`.
- `just format`: format each Calcite crate.
- `just bundle [FORMAT]`: package the gallery.
- `just patch-regen`: regenerate and verify `patches/gpui-ce-glass.patch`.

## Conventions

- Rust 2024 edition. Use tabs (width 4) and a 110-column limit (`rustfmt.toml`).
- Source and script files start with an `SPDX-License-Identifier: MPL-2.0` header.
- Format with `cargo fmt -p <crate>`, never `cargo fmt --all`: it also reformats `vendor/gpui-ce` through the path dependencies.
- Record user-visible changes in `CHANGELOG.md` under `## [Unreleased]`. Breaking changes are allowed before 1.0; list them under a Breaking heading.
- Write commit messages in Conventional Commits style (`feat:`, `fix:`), as in `git log`.

## Gotchas

- Frame timings need the `frame-stats` cargo feature and `CALCITE_FRAME_STATS=1`. `just stats` sets both.
- `window.refresh()` invalidates every cached view. Use it only when the whole window must repaint.
- Motion lives in `primitives/motion/`: take springs from `motion::tokens` (never raw numbers) and drive values with `motion::spring`, which keeps velocity on retarget; gpui `Animated::set` loses it.
- The cached-view fix lives in `patches/gpui-ce-glass.patch`. Do not build against an unpatched gpui-ce.
- Only macOS runs locally. Linux and Windows backends are checked only in CI.
