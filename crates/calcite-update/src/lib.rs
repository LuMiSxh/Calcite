// SPDX-License-Identifier: MPL-2.0

//! Signed, delta-capable app updates, in the spirit of Sparkle, for macOS, Windows, and
//! Linux.
//!
//! - **Feed:** `update.json` is an [`Envelope`]: the [`Feed`] text, its Ed25519 signature, and
//!   the ID of the signing key. A client verifies the signature before it reads anything in
//!   the feed. The feed names the product, the channel, the version, a sequence number, the
//!   signing time, and per target the complete file tree with a SHA-256 for every file. A
//!   client rejects a replayed or an expired feed.
//! - **Packages:** a full package holds every file. A delta holds only new and changed
//!   files, each compressed alone or as a zstd patch against the installed file. Unchanged
//!   files, such as a bundled runtime, cost nothing.
//! - **Staging:** the new tree is built next to the installed one and every file is checked
//!   against the signed tree. A delta that does not fit the installed copy falls back to
//!   the full package.
//! - **Install:** the swap is a rename, or a journaled move of the listed files, run when the
//!   app quits. See [`install`]. The security model is in the crate README.
//!
//! ```no_run
//! # fn main() -> calcite_update::Result<()> {
//! use calcite_update::{Updater, installed_root, verifying_key};
//!
//! let Some(root) = installed_root() else { return Ok(()) }; // a package manager updates it
//! calcite_update::cleanup(&root);
//! let updater = Updater::builder([("2026".to_string(), verifying_key("<64 hex characters of the public key>")?)])
//!     .feed_url("https://github.com/OWNER/APP/releases/latest/download/update.json")
//!     .product("dev.example.app")
//!     .channel("stable")
//!     .current(env!("CARGO_PKG_VERSION").parse().map_err(calcite_update::Error::Feed)?)
//!     .root(root)
//!     .state_dir("/path/to/the/app/data/folder")
//!     .build()?;
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
mod fsutil;
pub mod install;
mod journal;
mod manifest;
mod pack;
#[cfg(feature = "publish")]
pub mod publish;
mod sign;
pub mod stage;
#[cfg(feature = "client")]
mod state;
mod version;

use std::{fmt, io, path::Path};

#[cfg(feature = "client")]
pub use client::{
	Available, Clock, DEFAULT_MAX_FEED_AGE, Staged, SystemClock, Updater, UpdaterBuilder, asset_url,
};
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
	/// The feed is a replay, older than the maximum age, or dated in the future.
	Stale(String),
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
			Self::Stale(reason) => write!(f, "stale: {reason}"),
			Self::Io(error) => write!(f, "{error}"),
		}
	}
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;
