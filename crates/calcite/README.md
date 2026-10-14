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
   calcite = { git = "https://github.com/LuMiSxh/Calcite", rev = "CALCITE_COMMIT" }
   gpui = { package = "gpui-ce", git = "https://github.com/OWNER/gpui-ce", rev = "GPUI_COMMIT" }
   gpui_platform = { package = "gpui_ce_platform", git = "https://github.com/OWNER/gpui-ce", rev = "GPUI_COMMIT" }
   ```

3. Inside this workspace, Calcite uses `vendor/gpui-ce` instead. Run `scripts/bootstrap.sh`
   (macOS, Linux) or `scripts/bootstrap.ps1` (Windows) to create it.

Cargo features:

| Feature  | Default | Contents                                                         |
| -------- | ------- | ---------------------------------------------------------------- |
| `lucide` | yes     | The `Lucide` enum with every Lucide icon (about 0.9 MB of SVG). |
| `tint`   | yes     | `calcite::tint` and `Theme::set_artwork_tint` for Dynamic Theme. |

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
| Choices  | `choice_card` (radio, checkbox, or action), `choice_group`, `tag`, `chip`                         |
| Steps    | `step_rail`, `step` (done, current, upcoming)                                                    |
| Menus    | `menu_picker`, `action_menu`                                                                     |
| HUD      | `pill`, `control_bar`, `status`, `progress_outline`                                              |
| Layout   | `settings_page`, `settings_panel`, `page_header`, `action_row`, `danger_zone`, `hairline`        |
| Lists    | `nav_rail`, `nav_item`, `list_row`, `link`, `document_link`                                      |
| Overlays | `modal`, `modal_layer`, `floating_action_bar`, `confirm_dialog`                                  |
| Feedback | `badge`, `banner`, `spinner`, `progress_bar`, `empty_state`, `skeleton`, `.glass_tooltip(text)`  |
| Surfaces | `.glass(Glass)`, `.glass_hud(&theme)`, `.glass_tinted(&theme, tint)`, `glass_panel`              |

Sizes: `.compact()`, `.size(Size::Regular)` (default), and `.large()`. Tones: `.accent()`,
`.neutral()`, `.warning()`, `.danger()`, and `.tone(Tone::Color(rgba))`.

## Theme

The theme holds a palette, the accent, the glass style, and the accessibility settings. Every
component reads it with `cx.theme()`. This works in `App`, `Context<V>`, `AsyncApp`, and
`AsyncWindowContext`.

```rust
Theme::update(cx, |theme| theme.glass = GlassStyle::MacOS27(Transparency::Clear));
Theme::set_dynamic_accent(cx, Some(gpui::rgba(0xF59E38FF)));
```

Theme changes crossfade over the `Crossfade` curve (0.48 s). Under Reduce Motion they apply at
once. Use `Theme::set_immediately` to skip the crossfade.

| Field             | Effect                                                                    |
| ----------------- | ------------------------------------------------------------------------- |
| `palette`         | Every color the components draw with.                                     |
| `dynamic_accent`  | The accent from the artwork. `None` uses the palette accent.              |
| `glass`           | `MacOS26`, `MacOS27(Clear / Default / Tinted)`, or `ReducedTransparency`. |
| `reduce_motion`   | Springs settle at once. Scale and stretch effects stop.                   |
| `shows_borders`   | Glass draws its own edge (Increase Contrast).                             |

`Theme::decorative_motion(window)` is false while the window is inactive or Reduce Motion is on.
Continuous animation must check it.

### Palette

A `Palette` holds every color: the fallback accent, neutrals, semantic colors, text, panels, and
the HUD tint. Pick one at compile time with a `const`:

```rust
const EMBER: Palette = Palette {
	accent: tokens::rgba(0.96, 0.62, 0.30, 1.),
	panel_background: tokens::rgba(0.09, 0.07, 0.06, 1.),
	..Palette::ARKNIGHTS
};

calcite::init_with(cx, Theme::new(EMBER));
```

Replace it at runtime. The dynamic accent stays:

```rust
Theme::update(cx, |theme| *theme = theme.with_palette(Palette::ARKNIGHTS));
```

### Dynamic Theme

Dynamic Theme changes two values only: the accent and the hue of the HUD tint. The palette
stays. With the default `tint` feature, `calcite::tint` finds them in artwork:

```rust
let tint = calcite::tint::Extractor::default().extract(&rgba_pixels, width, height);
Theme::set_artwork_tint(cx, tint); // `None` for artwork without a hue: back to the palette accent
```

Run the extraction off the UI thread. The `home` showcase shows the pattern.

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

## Window chrome

The launcher hides the system title bar and draws its controls over the artwork. `chrome` does
this on every platform:

```rust
let bounds = Bounds::centered(None, size(px(1040.), px(680.)), cx);
cx.open_window(
	WindowOptions {
		window_min_size: Some(size(px(880.), px(560.))),
		..chrome::window_options("Arknights Client", bounds)
	},
	|window, cx| cx.new(|cx| Home::new(window, cx)),
);

// In the root view's render:
window_frame(
	div()
		.size_full()
		.child(content)
		.child(title_bar("title-bar").child(icon_button("settings", Icon::Gear, "Settings"))),
)
```

| Platform | What the title bar does                                                                       |
| -------- | --------------------------------------------------------------------------------------------- |
| macOS    | The system draws the traffic lights. The app owns the drag, so clicks in the bar have no delay. |
| Windows  | Calcite draws the caption buttons. The system hit-tests them, so Snap Layouts work.            |
| Linux    | Calcite draws the shadow, the rounded frame, the resize edges, and the caption buttons.        |

Linux needs a compositor for client-side decorations. Without one, the window manager draws its
frame and the bar holds only the app's controls. GPUI clips to rectangles, so round full-bleed
images with `chrome::corner_radius(window)`.

On macOS and Windows, the system close runs `Window::on_window_should_close`. On Linux, give the
same check to `title_bar(..).on_close(..)`.

## App icon and bundling

GPUI shows the generic executable icon until the app sets its own. `AppIcon` covers `cargo run`:

```rust
static ICON: AppIcon = calcite::app_icon!("assets/icon.png"); // square PNG, 1024 px

application.run(|cx| {
	calcite::init(cx);
	ICON.install(cx); // macOS: Dock and app switcher
	cx.open_window(ICON.window_options(WindowOptions::default()), |_, cx| /* … */);
});
```

| Platform | Where the icon comes from                                                                 |
| -------- | ----------------------------------------------------------------------------------------- |
| macOS    | `AppIcon::install` without a bundle; the `.icns` of the `.app` in a bundle.               |
| Windows  | Icon resource 1 of the `.exe`. Embed it in `build.rs` (see below).                        |
| X11      | `AppIcon::window_options`.                                                                |
| Wayland  | The `.desktop` file. Set `WindowOptions::app_id` to the binary name, like the bundle does. |

Embed the Windows icon in `build.rs`:

```toml
[build-dependencies]
calcite-bundle = { git = "https://github.com/LuMiSxh/Calcite", default-features = false }
```

```rust
fn main() {
	calcite_bundle::build::windows_icon("assets/icon.png");
}
```

`calcite-bundle` also packages the app: a `.app` and DMG on macOS, an NSIS installer or MSI on
Windows, and a deb, AppImage, or pacman package on Linux. It wraps
[cargo-packager](https://github.com/crabnebula-dev/cargo-packager) and makes every icon size from
the one PNG:

```toml
[package.metadata.calcite.bundle]
name = "My App"
identifier = "dev.example.app"
icon = "assets/icon.png"
resources = ["assets/artwork/**/*"]
minimum-macos = "13.0"
```

```sh
cargo install --git https://github.com/LuMiSxh/Calcite calcite-bundle
cargo calcite bundle                      # the platform's default formats
cargo calcite bundle --format dmg --bin my-app
```

Packages land in `target/calcite-bundle/release`. Signing and notarization go through
cargo-packager's own settings; the wrapper does not set them yet.

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
