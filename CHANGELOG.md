# Changelog

All notable changes to Calcite are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Breaking

- `calcite-tint`: `Extractor::extract` takes an `ImageView` instead of a packed RGBA slice and dimensions. Use `ImageView::packed_rgba` for tightly packed RGBA; a malformed buffer returns an `ImageError`.
- `calcite-tint`: `Hsb` fields are private. Read them with `hue()`, `saturation()`, and `brightness()`, and build colors with `Hsb::new`, which wraps the hue and clamps the other values.
- `calcite-tint`: `Extractor` drops `hue_buckets` (the bucket count is fixed at 24). Its tuple settings are now `Bounds` and `BackgroundTint`.

### Added

- `calcite-tint`: `ImageView` accepts a row stride and BGRA pixels, so padded or BGRA buffers need no copy.

### Fixed

- `calcite-tint`: a red image no longer loses to a smaller blue area when its hue sits at the 0/1 seam. Hue buckets wrap, and the winning hue is a circular mean.
- `calcite-tint`: the artwork is averaged over a grid of cells instead of read at one pixel per cell, so the tint no longer depends on image size or a single stray pixel.

## [0.4.0] - 2026-10-16

### Added

- `artwork`: `artwork_layer` draws the launcher backdrop (a cover-fit image, or the fallback "A" and accent slash, with an optional blurred cross-fade) and `readability_field` draws the dark field behind the wordmark. `Assets::default` serves the slash.

## [0.3.0] - 2026-10-15

### Added

- Release workflow (`.github/workflows/release.yml`), started by hand with a version tag: it builds the developer preview as one standalone file per system (Windows `.exe`, Linux `.AppImage`, and a universal macOS `.dmg` for Apple silicon and Intel), creates the tag, and publishes a GitHub release. `dry_run` builds the files without a tag or release.
- `calcite-bundle`: the `exe` format copies the Windows executable alone, without an installer, as `<name>_<version>_<arch>.exe`. It needs a Windows host and no `resources`.
- `calcite-bundle`: the `standalone` format names the host's single file: `exe` on Windows, `dmg` on macOS, `appimage` on Linux.

### Performance

- Windows (Direct3D): backdrop blur and glass run under a scissor around each filter, like Metal and Vulkan. Each glass control used to blur the whole window three times per frame.
- All backends: a backdrop filter blurs only the part of its bounds its content mask shows, so glass half scrolled out of a list blurs half.
- All backends: the gaussian blur fetches two neighbouring taps with one bilinear sample, which halves the texture reads of the blur passes for radii up to about 21 px. The result is the same.
- Neutral flat controls at rest paint a plain fill instead of a one-pixel glass pass; only the pointer light on hover still uses glass.

### Fixed

- Release builds of `preview` and `home` no longer open a console window next to the app on Windows.

## [0.2.1] - 2026-10-14

### Added

- Size scale: `Style::with_scale`, `Style::scale`, `Theme::with_scale`, `Theme::scale`, `Theme::px`, `Theme::pt`, and `Theme::text_*`. It scales text, control heights, paddings, and icon sizes, and it crossfades with the theme. The default is 1.
- `Size::height_in(theme)`, the scaled control height.
- `Modal::max_dimensions`: a modal that fits its content up to a size, clamped to the window.

### Fixed

- Action rows of modals and `confirm_dialog` wrap instead of clipping when the font is large or the window is narrow. Buttons keep their width.

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
