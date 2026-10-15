// SPDX-License-Identifier: MPL-2.0

//! Packages a Calcite app for distribution, and gives it its icon.
//!
//! - [`bundle`] (feature `packager`, on by default, also the `cargo calcite bundle` command)
//!   builds the app and writes a macOS `.app` and DMG, a Windows NSIS or MSI installer or a
//!   standalone `.exe`, or a Linux deb, AppImage, or pacman package. It is a thin wrapper around
//!   [cargo-packager](https://github.com/crabnebula-dev/cargo-packager), the bundler of the
//!   Tauri team.
//! - [`build::windows_icon`] embeds the icon in the Windows executable from `build.rs`. Add
//!   the crate as a build dependency with `default-features = false` for that.
//!
//! The app describes itself in its `Cargo.toml`:
//!
//! ```toml
//! [package.metadata.calcite.bundle]
//! name = "Arknights Client"            # default: the package name
//! identifier = "dev.example.client"    # required, reverse DNS
//! icon = "assets/icon.png"             # one square PNG, at least 512 px; all sizes come from it
//! resources = ["assets/artwork/**/*"]  # globs, copied into the package
//! category = "Game"
//! copyright = "© 2026 Example"
//! formats = ["app", "dmg"]             # default: the platform's formats
//! minimum-macos = "13.0"
//! ```
//!
//! Version, description, authors, homepage, and license come from the package itself.
//!
//! | Platform | Default formats   | Other formats                   |
//! | -------- | ----------------- | ------------------------------- |
//! | macOS    | `app`, `dmg`      |                                 |
//! | Windows  | `nsis`            | `wix` (MSI), `exe` (standalone) |
//! | Linux    | `deb`, `appimage` | `pacman`                        |
//!
//! `exe` copies the executable alone, without an installer. It needs a Windows host and no
//! `resources`; the icon is in the executable when `build.rs` calls [`build::windows_icon`].

pub mod build;
#[cfg(feature = "packager")]
mod package;

#[cfg(feature = "packager")]
pub use package::{Options, Output, Result, bundle};
