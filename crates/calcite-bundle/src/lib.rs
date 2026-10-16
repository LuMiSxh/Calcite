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
//!
//! [package.metadata.calcite.bundle.macos]    # optional: signs the .app and DMG
//! signing-identity = "Developer ID Application: Example (TEAM123)"
//! entitlements = "packaging/entitlements.plist"
//!
//! [package.metadata.calcite.bundle.windows]  # optional: signs the binary and installers
//! certificate-thumbprint = "0123456789ABCDEF0123456789ABCDEF01234567"  # or CALCITE_SIGN_CERT
//! timestamp-url = "http://timestamp.digicert.com"
//!
//! [package.metadata.calcite.bundle.packager] # optional: merged verbatim into cargo-packager's config
//! deb = { depends = ["libgtk-3-0"] }
//! ```
//!
//! Unknown keys in these tables are errors that name the key and the manifest. Version,
//! description, authors, homepage, and license come from the package itself.
//!
//! Signing is off unless a signing key is set, so local builds need no certificate. Notarizing
//! a signed macOS build needs `APPLE_NOTARY_KEYCHAIN_PROFILE`, or `APPLE_ID`, `APPLE_TEAM_ID`,
//! and `APPLE_APP_PASSWORD`. A Windows certificate file needs `CALCITE_SIGN_CERT`, with
//! `CALCITE_SIGN_PASSWORD` if it is protected. Credentials come from the environment only.
//! cargo-packager signs the `.app` with the hardened runtime, and Calcite notarizes and staples
//! the DMG.
//!
//! | Platform | Default formats   | Other formats                   |
//! | -------- | ----------------- | ------------------------------- |
//! | macOS    | `app`, `dmg`      |                                 |
//! | Windows  | `nsis`            | `wix` (MSI), `exe` (standalone) |
//! | Linux    | `deb`, `appimage` | `pacman`                        |
//!
//! `exe` copies the executable alone, without an installer. It needs a Windows host and no
//! `resources`; the icon is in the executable when `build.rs` calls [`build::windows_icon`].
//!
//! `standalone` names the host's single-file format: `exe` on Windows, `dmg` on macOS, and
//! `appimage` on Linux.

pub mod build;
#[cfg(feature = "packager")]
mod config;
#[cfg(feature = "packager")]
mod icons;
#[cfg(feature = "packager")]
mod package;
#[cfg(feature = "packager")]
mod packager;
#[cfg(feature = "packager")]
mod sign;
#[cfg(feature = "packager")]
mod stage;

#[cfg(feature = "packager")]
pub use package::{Options, Output, Result, bundle};
