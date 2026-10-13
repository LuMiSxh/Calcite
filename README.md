# Calcite

Calcite is a glass UI library on [gpui-ce](https://github.com/gpui-ce/gpui-ce). This workspace
holds the library, its execution layer, and the showcase. It started as the Rust study
for [Arknights Client](https://github.com/LuMiSxh/Arknights-MacOS-Client) and does not depend on it.

| Crate            | Contents                                                                                     |
| ---------------- | -------------------------------------------------------------------------------------------- |
| `calcite`        | The glass UI library: Liquid Glass styles, spring motion, controls, forms, menus, panels, modals, HUD surfaces, Lucide icons. See [its README](crates/calcite/README.md) |
| `calcite-exec`     | Execution layer: `TaskSlot`, `ActivityStore` with leases, `Throttle`, `bounded`              |
| `calcite-showcase` | `home` (launcher home screen with a simulated install) and `gallery` (every component once) |
| `patches/`       | The gpui-ce patch that adds `Filter::Glass`, and the pinned gpui-ce commit                    |

## Set up and run

1. Install current stable Rust with `rustup` (tested with 1.97).
2. On macOS, install Xcode.
3. Fetch gpui-ce and apply the glass patch:

   macOS and Linux:

   ```sh
   ./scripts/bootstrap.sh
   ```

   Windows (PowerShell):

   ```powershell
   ./scripts/bootstrap.ps1
   ```

4. Start the gallery or the home screen:

   ```sh
   cargo run --release --bin gallery
   cargo run --release --bin home
   cargo run --release --bin home -- ~/Pictures/artwork.jpg
   ```

The first build compiles gpui-ce and takes several minutes. Run the bootstrap script again after
`patches/` changes.

### Keys

| Window  | Key       | Action                                              |
| ------- | --------- | --------------------------------------------------- |
| both    | G         | Next glass style                                    |
| both    | A         | Next accent                                         |
| both    | M         | Toggle Reduce Motion                                |
| home    | Space     | Primary action: Install, Pause, Resume, Play, Stop  |
| home    | 1, 2, 3   | Expand the music, version, or status pill           |
| home    | U         | Show the neutral "Install Update" button            |
| home    | O         | Toggle between your image and the procedural art    |
| home    | H         | Show or hide the controls panel                     |
| both    | Command-Q | Quit                                                |

## Liquid Glass

Liquid Glass is not a blur. The look comes from the bezel, a curved edge that:

- bends the backdrop along the shape normal (lensing),
- splits red and blue slightly (dispersion),
- catches a thin Fresnel rim all around,
- throws a specular highlight on the side that faces the light, and a fainter one opposite.

The flat face in the middle shows the softly blurred, more saturated backdrop through a fill. Larger
surfaces read thicker: more lensing and a deeper shadow.

The patch adds this as `Filter::Glass` to gpui-ce's existing backdrop pipeline (downsample, gaussian
ping/pong, composite). Only the composite pass changes. It uses one SDF evaluation per pixel (four
more for the normal) and three backdrop samples. There is no extra render pass. gpui-ce writes its
shaders once in Rust (`wgsl_rs`) and generates WGSL, MSL, and HLSL at build time. So the same glass
code runs on Metal, Direct3D 11, and wgpu.

`calcite/src/glass.rs` chooses the parameters per system look:

| Style               | Backplate (neutral / chromatic, minimum tint) | Blur | Refraction | Notes                                    |
| ------------------- | --------------------------------------------- | ---: | ---------: | ---------------------------------------- |
| macOS 26            | none; the tint colors the glass               |  9   |     11     | Clear glass, strongest lensing           |
| macOS 27 · Clear    | 0.38 / 0.26, 0.20                             | 10   |      9     | The slider's clear end                   |
| macOS 27 · Default  | 0.68 / 0.44, 0.24                             | 13   |      7     | The launcher's macOS 27 branch           |
| macOS 27 · Tinted   | 0.86 / 0.64, 0.34                             | 16   |      5     | The slider's opaque end                  |
| Reduce Transparency | opaque 0.92 and an edge                       |  —   |      —     | No backdrop at all                       |

The pointer light (`PointerSheen` in the Swift app) is part of the same shader. Standard and large
capsule actions pass `window.mouse_position()` while hovered. Secondary actions use a flat surface
with the same light and no lensing.

## Execution layer

`calcite-exec` adds no runtime of its own. Work runs as ordinary GPUI tasks owned by a view. Results
reach state only through `Entity::update`. Each type maps to one Swift type in Arknights Client:

| Rust                                       | Swift                                      |
| ------------------------------------------ | ------------------------------------------ |
| `TaskSlot::replace`, `owns`, `invalidate`, `publish` | `TaskSlot.replace`, `owns`, `invalidate` |
| `ActivityStore::begin`, `update`, `end`    | `LauncherLifecycleStore` and `ActivityLease` |
| `Throttle::progress()`                     | progress throttling to about 10 Hz         |
| `bounded(limit, items, work)`              | windowed task group with an `AppConstants` limit |

The `home` binary shows the pattern. `start_transfer` claims a lease and replaces the slot's task.
It throttles raw progress and publishes only while its token owns the slot. Pause invalidates the
slot before it ends the lease.

## Verification status

| Check                                         | Result                                                    |
| --------------------------------------------- | --------------------------------------------------------- |
| Linux build, clippy, tests                    | Pass                                                      |
| Linux render (Xvfb, software Vulkan)          | Gallery and home render; screenshots reviewed             |
| macOS Metal backend (`cargo check --target aarch64-apple-darwin -p gpui_ce_apple`) | Compiles; not run on a Mac yet |
| Windows Direct3D backend                      | Not compiled (needs MSVC); the change mirrors the Metal one |
| gpui-ce shader ABI tests (`gpui_ce_render`)   | Pass                                                      |

## Known gaps

- Under Reduce Motion, SwiftUI pins the pointer light to the center. Calcite hides it instead.
- The parameters are tuned against renders on Linux, not against macOS 26/27 screenshots. Tune them
  on a Mac next.
- Calcite needs the patched gpui-ce. Push it to a fork with `scripts/publish-gpui-fork.sh` before
  you use Calcite as a Git dependency.
