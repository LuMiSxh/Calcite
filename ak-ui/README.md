# ak-ui prototype

This crate is a prototype of the launcher UI kit on [gpui-ce](https://github.com/gpui-ce/gpui-ce).
It is part of the [Rust rewrite study](../../studies/rust-gpui-rewrite/README.md). It contains only
UI components and a showcase. It contains no launcher logic. It downloads, installs, and starts
nothing.

## Run on a Mac

1. Install current stable Rust with `rustup` (tested with 1.97).
2. Install Xcode. GPUI compiles its Metal shaders with it.
3. Open a terminal in this folder.
4. Run the showcase:

   ```sh
   cargo run --release
   ```

5. Optional: show the glass over your own image:

   ```sh
   cargo run --release -- ~/Pictures/artwork.jpg
   ```

The first build downloads and compiles gpui-ce. It takes several minutes.

## Showcase controls

| Key       | Action                                               |
| --------- | ---------------------------------------------------- |
| Space     | Primary action: Install, Pause, Resume, Play, Stop   |
| A         | Next accent (fallback cyan, then four theme accents) |
| 1, 2, 3   | Expand the music, version, or status pill            |
| U         | Show the neutral "Install Update" button             |
| T         | Toggle Reduce Transparency                           |
| M         | Toggle Reduce Motion                                 |
| O         | Toggle between your image and the procedural art     |
| H         | Show or hide the controls panel                      |
| Command-Q | Quit                                                 |

The download is simulated. It ends with a short verification, then the Play state with the accent
halo.

## Contents

| Module        | Ports from                                                          |
| ------------- | ------------------------------------------------------------------- |
| `tokens.rs`   | `LauncherVisuals`, `AppConstants.HUD`, `contrastTint`, HUD tint     |
| `glass.rs`    | The macOS 27 branch of `AdaptiveGlass.swift`                        |
| `motion.rs`   | `LauncherMotion` curves (SwiftUI springs mapped to stiffness/damping) |
| `controls.rs` | `CapsuleActionButton`, `IconActionButton`, `AccentHalo`             |
| `hud.rs`      | `ExpandableHUDPill`, `HUDPillSurface`, the control bar, status text |
| `progress.rs` | `CapsuleProgressOutline` with glow, sweep, and indeterminate glint  |
| `artwork.rs`  | Artwork layer, `LauncherReadabilityField`, fallback wordmark        |
| `icons.rs`    | Project-owned SVG icons (no SF Symbols)                             |

## How the glass works

SwiftUI on macOS 27 stacks Liquid Glass, a backplate (black 0.68, or the tint over black 0.44), and
the content. The kit draws the same stack on one element:

1. `backdrop_blur(30 px)` blurs everything behind the surface.
2. One translucent fill replaces the glass lift and the backplate. `tokens::stack` composites the
   layers with the same opacities as `AdaptiveGlass.swift`.
3. Inset box shadows draw the specular rim and the edge glow. An outer shadow lifts the surface.
4. The caller adds the semantic border, as in SwiftUI.

## Known differences from the SwiftUI app

- gpui-ce has no element scale. Press feedback dims the control instead of scaling it to 0.95.
  Hover shows a fill change, not a 1.025 lift.
- There is no lens refraction or interactive glass response. Option B of the study adds them with a
  `custom-gpu` shader.
- The wordmark is a text fallback. The real app shows the official logo from the publisher's
  configuration.
- Accessibility labels (AccessKit) and the motion visibility gate are not wired yet.
- The music, version, and region data are sample values.
