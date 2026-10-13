# Calcite

Calcite is a glass UI library for [gpui-ce](https://github.com/gpui-ce/gpui-ce). It gives you:

- the Liquid Glass looks of macOS 26 and macOS 27 (Clear, Default, Tinted), and Reduce Transparency;
- spring motion with SwiftUI `spring(duration:bounce:)` semantics;
- controls, forms, menus, panels, modals, HUD surfaces, and a progress outline;
- 2100+ [Lucide](https://lucide.dev) icons and an `icons!` macro for your own icons.

Calcite draws every surface itself. The look is the same on macOS, Linux, and Windows.

## Requirements

- Rust 1.97 or later, edition 2024.
- gpui-ce with the Liquid Glass patch (`Filter::Glass`). The patch is in
  [`patches/gpui-ce-glass.patch`](../../patches/gpui-ce-glass.patch). Upstream gpui-ce does not
  have this filter yet.

## Add Calcite to a project

Calcite and your app must use the same patched gpui-ce.

1. Push the patched gpui-ce to a fork. Do this one time per gpui-ce revision:

   ```sh
   ./scripts/publish-gpui-fork.sh https://github.com/OWNER/gpui-ce.git calcite
   ```

   The script prints the commit to pin.

2. Add the dependencies to your `Cargo.toml`:

   ```toml
   [dependencies]
   calcite = { git = "https://github.com/LuMiSxh/calcite", rev = "CALCITE_COMMIT" }
   gpui = { package = "gpui-ce", git = "https://github.com/OWNER/gpui-ce", rev = "GPUI_COMMIT" }
   gpui_platform = { package = "gpui_ce_platform", git = "https://github.com/OWNER/gpui-ce", rev = "GPUI_COMMIT" }
   ```

3. Inside this workspace, Calcite uses `vendor/gpui-ce` instead. Run `scripts/bootstrap.sh`
   (macOS, Linux) or `scripts/bootstrap.ps1` (Windows) to create it.

Cargo features:

| Feature  | Default | Contents                                                         |
| -------- | ------- | ---------------------------------------------------------------- |
| `lucide` | yes     | The `Lucide` enum with every Lucide icon (about 0.9 MB of SVG). |

## Start

```rust
use calcite::prelude::*;

fn main() {
	gpui_platform::application().with_assets(Assets::default()).run(|cx| {
		calcite::init(cx);
		// Open a window. In its render, build components:
		// control_bar(status("Ready")).action(button("play", "Play").accent().large().icon(Icon::Play))
	});
}
```

`calcite::init` sets the theme and the text input key bindings. `Assets::default()` serves the
icons. Run the full example:

```sh
cargo run -p calcite --example minimal
```

## Rules of the API

- A component is a free function named after it: `button`, `toggle`, `modal`.
- You configure it with chained methods, like GPUI's `div()`.
- Components are stateless. The view owns the value and gives a change handler:
  `toggle("sync", self.sync).on_change(cx.listener(|this, on: &bool, _, cx| { … }))`.
- Every component reads the app theme with `cx.theme()`. Call `.theme(theme)` on a component to
  override it for that component only.
- Every interactive component has an id, an accessible role, and a label.

## Components

| Group    | Builders                                                                                         |
| -------- | ------------------------------------------------------------------------------------------------ |
| Actions  | `button`, `icon_button`, `halo`, `done_button`                                                   |
| Forms    | `toggle` (elongated macOS 26 shape, glass lens on press), `slider`, `segmented`, `text_field`   |
| Menus    | `menu_picker`, `action_menu`                                                                     |
| HUD      | `pill`, `control_bar`, `status`, `progress_outline`                                              |
| Layout   | `settings_page`, `settings_panel`, `page_header`, `action_row`, `danger_zone`, `hairline`        |
| Lists    | `nav_rail`, `nav_item`, `list_row`, `link`, `document_link`                                      |
| Overlays | `modal`, `modal_layer`, `floating_action_bar`                                                    |
| Feedback | `badge`, `skeleton`, `.glass_tooltip(text)`                                                      |
| Surfaces | `.glass(Glass)`, `.glass_hud(&theme)`, `.glass_tinted(&theme, tint)`, `glass_panel`              |

Sizes: `.compact()`, `.size(Size::Regular)` (default), and `.large()`. Tones: `.accent()`,
`.neutral()`, `.warning()`, `.danger()`, and `.tone(Tone::Color(rgba))`.

## Theme

```rust
Theme::update(cx, |theme| theme.glass = GlassStyle::MacOS27(Transparency::Clear));
Theme::update(cx, |theme| *theme = theme.with_accent(Some(gpui::rgba(0xF59E38FF))));
```

| Field          | Effect                                                                   |
| -------------- | ------------------------------------------------------------------------ |
| `accent`       | The tint of primary actions, selection, progress, and switches.          |
| `glass`        | `MacOS26`, `MacOS27(Clear / Default / Tinted)`, or `ReducedTransparency`. |
| `reduce_motion` | Springs settle at once. Scale and stretch effects stop.                 |
| `shows_borders` | Glass draws its own edge (Increase Contrast).                          |

`Theme::decorative_motion(window)` is false while the window is inactive or Reduce Motion is on.
Continuous animation must check it.

## Icons

```rust
icon(Icon::Gear).size(px(16.))
Lucide::Rocket.render(px(20.), tokens::PRIMARY_TEXT)
```

`Icon` holds the glyphs that the components use, with semantic names. `Lucide` holds every icon of
the vendored Lucide version. Declare your own set with `icons!`:

```rust
calcite::icons! {
	pub enum AppIcon in "assets/icons" {
		Logo => "logo.svg",
	}
}
// Register it: Assets::default().icons::<AppIcon>()
```

To vendor a new Lucide version, run `scripts/update-lucide.sh VERSION`. The script checks the npm
integrity hash.

## Key bindings

```rust
calcite::keymap!(cx, {
	"Home" => { "space" => PrimaryAction, "g" => CycleGlass },
	_ => { "cmd-q" => Quit, "ctrl-q" => Quit },
});
```

## Licenses

Calcite is licensed under the [MPL-2.0](LICENSE). The Lucide icons are licensed under the
[ISC license](assets/lucide/LICENSE). The glyphs in `assets/icons` are Lucide glyphs; the transport
glyphs (play, pause, stop, forward) are filled.
