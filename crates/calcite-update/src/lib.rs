// SPDX-License-Identifier: MPL-2.0

//! Signed, delta-capable app updates, in the spirit of Sparkle, for macOS, Windows, and
//! Linux.
//!
//! - **Feed:** `update.json` is an [`Envelope`]: the [`Feed`] text and its Ed25519
//!   signature. A client verifies the signature before it reads anything in the feed. The
//!   feed names the product, the channel, the version, and per target the complete file
//!   tree with a SHA-256 for every file.
//! - **Packages:** a full package holds every file. A delta holds only new and changed
//!   files, each compressed alone or as a zstd patch against the installed file. Unchanged
//!   files, such as a bundled runtime, cost nothing.
//! - **Staging:** the new tree is built next to the installed one and every file is checked
//!   against the signed tree. A delta that does not fit the installed copy falls back to
//!   the full package.
//! - **Install:** the swap is a rename, run when the app quits. See [`install`].
//!
//! ```no_run
//! # fn main() -> calcite_update::Result<()> {
//! use calcite_update::{Updater, installed_root, verifying_key};
//!
//! let Some(root) = installed_root() else { return Ok(()) }; // a package manager updates it
//! calcite_update::cleanup(&root);
//! let updater = Updater {
//!     feed_url: "https://github.com/OWNER/APP/releases/latest/download/update.json".into(),
//!     product: "dev.example.app".into(),
//!     channel: "stable".into(),
//!     current: env!("CARGO_PKG_VERSION").parse().map_err(calcite_update::Error::Feed)?,
//!     key: verifying_key("<64 hex characters of the public key>")?,
//!     root,
//! };
//! if let Some(update) = updater.check()? {
//!     let staged = updater.download(&update, |done, total| println!("{done}/{total}"))?;
//!     // Later, when the app quits and nothing runs:
//!     staged.install()?;
//! }
//! # Ok(())
//! # }
//! ```

#[cfg(feature = "client")]
mod client;
pub mod install;
mod manifest;
mod pack;
#[cfg(feature = "publish")]
pub mod publish;
mod sign;
pub mod stage;
mod version;

use std::{fmt, io, path::Path};

#[cfg(feature = "client")]
pub use client::{Available, Staged, Updater, asset_url};
pub use install::{cleanup, installed_root};
pub use manifest::{Asset, Delta, Entry, Feed, Kind, Target, target_id, validate_entries, validate_path};
pub use pack::{DeltaIndex, DeltaItem, Source};
pub use sign::{Envelope, verifying_key};
pub use version::Version;

/// Why an update did not happen.
#[derive(Debug)]
pub enum Error {
	Network(String),
	/// The signature is missing, malformed, or from another key.
	Signature(String),
	/// The feed is malformed, for another product or channel, or describes an unsafe tree.
	Feed(String),
	/// A download, a patch, or a written file does not match the signed feed.
	Integrity(String),
	Io(io::Error),
}

impl Error {
	pub(crate) fn io(path: &Path, error: io::Error) -> Self {
		Self::Io(io::Error::new(
			error.kind(),
			format!("{}: {error}", path.display()),
		))
	}
}

impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Network(reason) => write!(f, "network: {reason}"),
			Self::Signature(reason) => write!(f, "signature: {reason}"),
			Self::Feed(reason) => write!(f, "feed: {reason}"),
			Self::Integrity(reason) => write!(f, "integrity: {reason}"),
			Self::Io(error) => write!(f, "{error}"),
		}
	}
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;
