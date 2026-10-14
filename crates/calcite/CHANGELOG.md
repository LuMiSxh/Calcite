# Changelog

All notable changes to Calcite are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- Liquid Glass styles: macOS 26, macOS 27 (Clear, Default, Tinted), and Reduce Transparency.
- Spring motion curves with SwiftUI `spring(duration:bounce:)` semantics.
- Actions: `button`, `icon_button`, `halo`, `done_button`.
- Forms: `toggle` in the elongated macOS 26 shape with a glass lens on press, `slider`,
  `segmented`, `text_field`.
- Menus: `menu_picker`, `action_menu`.
- HUD: `pill`, `control_bar`, `status`, `progress_outline`.
- Layout: `settings_page`, `settings_panel`, `page_header`, `action_row`, `danger_zone`,
  `nav_rail`, `nav_item`, `list_row`, `link`, `document_link`.
- Overlays: `modal`, `modal_layer`, `floating_action_bar`.
- Feedback: `badge`, `skeleton`, glass tooltips.
- Icons: the `Icon` set, the full Lucide set behind the default `lucide` feature, and the
  `icons!` macro.
- The `keymap!` macro.
- `Palette`: every color in one value, const for compile-time looks and replaceable at runtime.
- Animated theme changes, `Theme::set_dynamic_accent`, and `Theme::set_artwork_tint` with the
  `calcite-tint` crate behind the default `tint` feature.
- Choices: `choice_card`, `choice_group`, `tag`, `chip`. Steps: `step_rail`, `step`.
- Feedback: `banner`, `spinner`, `progress_bar`, `empty_state`. Overlays: `confirm_dialog`.
- Motion curves `Page`, `Backdrop`, `Ripple`, `Settle`, `Disclosure`, `Curve::stagger`,
  `Curve::fade`, and `Materialize`.
- `AppIcon` and `app_icon!`: the Dock icon on macOS and the window icon on X11 outside a
  bundle.
- Motion: `.enter(id, index, reduce_motion)` fades and rises content into place with a
  stagger. Segmented controls slide one glass indicator between segments. Menus, sheets,
  tooltips, and panel disclosures animate in; choice indicators pop on selection; the
  navigation selection fades and its marker grows.
- Spacing scale `tokens::space` and more type sizes in `tokens::text`.
- Exit motion: `presence` keeps a view alive while it fades out. `modal_layer`,
  `confirm_dialog`, and `banner` take `.shown(bool)`; menus fade out when they close.
- `ContinuousMotion`: one shared clock per loop type for the spinner, the progress sweep,
  the outline breathing, and the skeleton pulse. A paused loop keeps its phase. Loops redraw
  at 30 fps and stop under Reduce Motion, in background windows, and under serious thermal
  pressure.
