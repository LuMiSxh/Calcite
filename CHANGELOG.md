# Changelog

All notable changes to Calcite are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

## [0.2.0] - 2026-10-14

### Breaking

- The `gallery` binary is renamed `preview`. Run it with `just run` or `cargo run --bin preview`.
- `BannerTone` and `PanelTone` are merged into `Tone`; `Tone::Info` is now `Tone::Accent`.
- `nav_rail(&theme)` is now `nav_rail(id)`, a builder.
- `page_header`, `hairline`, and `glass_panel` no longer take a theme argument.
- Progress outline options are builder calls: `progress_outline(mode).glint(true).animated(true)`.
- `Spinner::size` is now `Spinner::diameter`; `Skeleton::size` is now `Skeleton::dimensions(width, height)`; `Ticker::size` is now `Ticker::font_size`.
- `MODAL_DIM` is removed; use `Theme::scrim()`. `Modal::size` is now `Modal::dimensions`.
- The `BOUNCY`, `GENTLE`, and `MORPH_FOLD` spring tokens and `squash` are removed. See `motion::tokens` for the remaining springs.
- `floating_action_bar()` takes no theme argument.

### Added

- Developer Preview: the `preview` binary replaces `gallery`. It has an Appearance toolbar (Dark or Light, Reduce Motion, Reduce Transparency, accent), component matrices per section, Source lines that print each section's Rust calls, and a test that fails when a prelude builder has no section.
- `Palette::LIGHT`, a light palette with a pale HUD tint, and `Theme::follow_system` / `follow_system_with` to follow the OS appearance. Reduce Transparency and Increase Contrast are not exposed by gpui-ce.
- `Theme::sync_reduce_motion` copies the app's Reduce Motion flag into the theme.
- `Button::prominent`: a solid accent-filled button with a legible label. `Button::success`. `default_action` and the confirm button of `confirm_dialog` use `prominent`.
- `Theme::scrim`, `Chromatic::disabled_with`, and `tokens::{luminance, contrast_ratio, on_color}`.
- Sliders: capsule thumb, glide animation, and Home, End, PageUp, and PageDown keys.
- Menus: arrow, Home, End, and type-ahead navigation. Picker rows are radio menu items.
- Modals: Tab stays inside the modal, focus returns to the trigger on close, and the modal body is glass.
- Keyboard presses give the same press feedback as pointer presses.
- `Segmented::label`, `TextField::label`, and `Spinner::label`.
- `enabled` and `disabled` on interactive components.
- Nav rail: a roving tab stop and `label`. `tab_step` moves focus between tab stops.
- Attached surfaces stay inside the window: they shift at the window edges (`body_shift`).
- Preview flags: `--section`, `--palette`, `--reduce-motion`, `--reduce-transparency`, and `--glass`.
- Preview Source lines are `demo!` expressions: the snippet is the code that renders, and a Copy button puts it on the clipboard.

### Changed

- `calcite-bundle`: macOS `.app` and `.dmg` builds no longer fail with "No matching IconType"; the 64 and 1024 px icons are written as `@2x` entries.
- Disabled success and warning buttons keep a legible label in the light palette.
- All component text follows `Style.fonts` (Plus Jakarta Sans by default).
- The preview's Dark segment brings back the last dark palette (Arknights or Ember) instead of always Arknights.
- The preview's app bundle and window are named "Calcite - preview".
- The close morph rebounds outward: `MORPH_CLOSE` is 0.34 / 0.38, with a bump of up to 4 px.
- Motion curves are defined on `motion::tokens`; selection moves use `SELECT`.
- The focus ring is softer.
- The modal dim is lighter on a light palette (`Theme::scrim()`).
- The segmented label uses the 13 pt primary color.
- Reduce Motion gates all decorative frame requests.
- Window chrome no longer calls `window.refresh()`.
- `control::OVERLAY_DIM` is 0.30 (was 0.55).
- The disabled chromatic fill, the progress sweep, the choice press wash, and the selected chip follow the palette ink, so they stay visible on a light palette.

### Fixed

- A press could stick after the pointer was dragged off a control.
- Segmented hover could stick.
- Escape inside text fields.
- Tab could leave modals and menus.
- The dialog bottom-left corner bled.
- Slider value labels overlapped.
- Text fields did not repaint when their state was swapped.
- Menu rows stayed enabled when disabled while the menu was open.
- Hard-coded dark layers in light mode now follow the palette.

## [0.1.0] - 2026-10-14

Initial release.
