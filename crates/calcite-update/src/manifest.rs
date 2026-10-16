// SPDX-License-Identifier: MPL-2.0

//! The feed and the file tree it describes, and the checks every path passes before a
//! single byte is written.

use std::{
	collections::{BTreeMap, BTreeSet},
	time::Duration,
};

use serde::{Deserialize, Serialize};

use crate::{Error, Result, Version};

/// The newest release of one product on one channel. It reaches clients only inside a
/// signed [`crate::Envelope`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Feed {
	/// The format of this feed. Clients reject formats they do not know.
	pub format: u32,
	/// The product identifier, for example `dev.example.client`. A feed for another product
	/// is rejected even when its signature is valid.
	pub product: String,
	pub channel: String,
	pub version: String,
	/// When the feed was signed, in seconds since the Unix epoch. A client rejects a feed that
	/// is older than its maximum age, so an attacker cannot freeze a client on an old feed.
	pub issued_at: u64,
	/// Rises with every signed feed. A client rejects a lower number than it has seen, so an
	/// attacker cannot replay an older feed.
	pub sequence: u64,
	/// The oldest version that may keep running. A client below it sees a required update.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub minimum_version: Option<String>,
	/// Release notes in Markdown.
	#[serde(default)]
	pub notes: String,
	/// The packages per target, keyed by [`target_id`].
	pub targets: BTreeMap<String, Target>,
}

/// The release for one operating system and architecture.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Target {
	/// The complete new tree.
	pub entries: Vec<Entry>,
	/// Every file of the tree in one archive.
	pub full: Asset,
	/// Smaller packages that turn one older release into this one.
	#[serde(default)]
	pub deltas: Vec<Delta>,
}

/// A delta package and the release it starts from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Delta {
	pub from: String,
	pub asset: Asset,
}

/// A file next to the feed. `name` is relative to the feed's URL.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Asset {
	pub name: String,
	pub size: u64,
	/// Lowercase hex SHA-256 of the whole file.
	pub sha256: String,
}

/// One item of the tree, with a path relative to the bundle root and `/` separators.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
	pub path: String,
	#[serde(flatten)]
	pub kind: Kind,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Kind {
	Directory,
	File {
		size: u64,
		sha256: String,
		#[serde(default)]
		executable: bool,
	},
	/// A relative link that stays inside the tree, as in macOS frameworks.
	Symlink {
		target: String,
	},
}

/// The current platform's key in [`Feed::targets`], for example `macos-aarch64`.
pub fn target_id() -> String {
	format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

impl Feed {
	pub const FORMAT: u32 = 2;

	/// How far in the future `issued_at` may lie before a client rejects the feed.
	pub const CLOCK_SKEW: Duration = Duration::from_secs(5 * 60);

	/// The parsed [`Feed::minimum_version`].
	pub fn minimum(&self) -> Result<Option<Version>> {
		let Some(text) = &self.minimum_version else {
			return Ok(None);
		};
		let minimum: Version = text.parse().map_err(Error::Feed)?;
		let version: Version = self.version.parse().map_err(Error::Feed)?;
		if minimum > version {
			return Err(Error::Feed(format!(
				"the minimum version {minimum} is newer than the release {version}"
			)));
		}
		Ok(Some(minimum))
	}

	/// Rejects a replayed feed (`sequence` below `highest_sequence`), a frozen one (older than
	/// `max_age` at `now`, in Unix seconds), and one dated in the future.
	pub fn check_fresh(&self, now: u64, max_age: Duration, highest_sequence: u64) -> Result<()> {
		if self.sequence < highest_sequence {
			return Err(Error::Stale(format!(
				"the feed has sequence {}, but sequence {highest_sequence} was seen",
				self.sequence
			)));
		}
		if self.issued_at > now.saturating_add(Self::CLOCK_SKEW.as_secs()) {
			return Err(Error::Stale("the feed is dated in the future".into()));
		}
		if now.saturating_sub(self.issued_at) > max_age.as_secs() {
			return Err(Error::Stale("the feed is older than the maximum age".into()));
		}
		Ok(())
	}

	/// Checks that the feed is for `product` on `channel`, newer than `current`, and that the
	/// current target's tree is safe to write.
	pub fn check(&self, product: &str, channel: &str, current: &Version) -> Result<Option<&Target>> {
		if self.format != Self::FORMAT {
			return Err(Error::Feed(format!("unknown feed format {}", self.format)));
		}
		if self.product != product {
			return Err(Error::Feed(format!(
				"the feed is for `{}`, not `{product}`",
				self.product
			)));
		}
		if self.channel != channel {
			return Err(Error::Feed(format!(
				"the feed is for channel `{}`, not `{channel}`",
				self.channel
			)));
		}
		self.minimum()?;
		let version: Version = self.version.parse().map_err(Error::Feed)?;
		if version <= *current {
			return Ok(None);
		}
		let Some(target) = self.targets.get(&target_id()) else {
			return Ok(None);
		};
		validate_entries(&target.entries)?;
		for asset in std::iter::once(&target.full).chain(target.deltas.iter().map(|delta| &delta.asset)) {
			validate_asset(asset)?;
		}
		Ok(Some(target))
	}
}

/// The largest package a client downloads.
pub(crate) const MAX_ASSET_SIZE: u64 = 8 << 30;

fn validate_asset(asset: &Asset) -> Result<()> {
	let name = &asset.name;
	if asset.size > MAX_ASSET_SIZE {
		return Err(Error::Feed(format!(
			"asset `{name}` is larger than {MAX_ASSET_SIZE} bytes"
		)));
	}
	let plain = !name.is_empty()
		&& name
			.chars()
			.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+'))
		&& !name.starts_with('.');
	if !plain {
		return Err(Error::Feed(format!(
			"asset name `{name}` is not a plain file name"
		)));
	}
	validate_sha(&asset.sha256)
}

pub(crate) fn validate_sha(sha: &str) -> Result<()> {
	if sha.len() == 64
		&& sha
			.bytes()
			.all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
	{
		Ok(())
	} else {
		Err(Error::Feed(format!("`{sha}` is not a lowercase SHA-256")))
	}
}

/// Rejects a tree that could write outside its root: absolute or parent paths, duplicate
/// or case-colliding paths, items below a symlink, links that leave the tree, and files
/// without a parent directory entry.
pub fn validate_entries(entries: &[Entry]) -> Result<()> {
	let mut seen = BTreeSet::new();
	let mut folded = BTreeSet::new();
	let mut directories = BTreeSet::new();
	let mut links = BTreeSet::new();
	for entry in entries {
		validate_path(&entry.path)?;
		if !seen.insert(entry.path.as_str()) {
			return Err(Error::Feed(format!("`{}` appears twice", entry.path)));
		}
		// macOS and Windows file systems are usually case-insensitive.
		if !folded.insert(entry.path.to_lowercase()) {
			return Err(Error::Feed(format!(
				"`{}` differs from another path only in case",
				entry.path
			)));
		}
		if let Some(parent) = parent(&entry.path) {
			if links.contains(parent) || ancestors(parent).any(|ancestor| links.contains(ancestor)) {
				return Err(Error::Feed(format!("`{}` lies below a symlink", entry.path)));
			}
			if !directories.contains(parent) {
				return Err(Error::Feed(format!(
					"`{}` comes before its directory `{parent}`",
					entry.path
				)));
			}
		}
		match &entry.kind {
			Kind::Directory => {
				directories.insert(entry.path.as_str());
			}
			Kind::File { sha256, .. } => validate_sha(sha256)?,
			Kind::Symlink { target } => {
				validate_link(&entry.path, target)?;
				links.insert(entry.path.as_str());
			}
		}
	}
	Ok(())
}

/// A relative path of normal components: no `.`, `..`, empty parts, backslashes, drive
/// letters, or control characters.
pub fn validate_path(path: &str) -> Result<()> {
	let bad = path.is_empty()
		|| path.starts_with('/')
		|| path.contains('\\')
		|| path.contains(':')
		|| path.chars().any(char::is_control)
		|| path
			.split('/')
			.any(|part| part.is_empty() || part == "." || part == "..");
	if bad {
		return Err(Error::Feed(format!("unsafe path `{path}`")));
	}
	Ok(())
}

/// A link target must be relative and, resolved lexically from the link's directory, stay
/// inside the tree.
fn validate_link(path: &str, target: &str) -> Result<()> {
	if target.is_empty() || target.starts_with('/') || target.contains('\\') || target.contains(':') {
		return Err(Error::Feed(format!("link `{path}` points to unsafe `{target}`")));
	}
	let mut depth = path.split('/').count() as i64 - 1;
	for part in target.split('/') {
		match part {
			"" | "." => {}
			".." => depth -= 1,
			_ => depth += 1,
		}
		if depth < 0 {
			return Err(Error::Feed(format!(
				"link `{path}` leaves the tree through `{target}`"
			)));
		}
	}
	Ok(())
}

pub(crate) fn parent(path: &str) -> Option<&str> {
	path.rsplit_once('/').map(|(parent, _)| parent)
}

fn ancestors(path: &str) -> impl Iterator<Item = &str> {
	std::iter::successors(parent(path), |path| parent(path))
}

#[cfg(test)]
mod tests {
	use super::*;

	fn file(path: &str) -> Entry {
		Entry {
			path: path.into(),
			kind: Kind::File {
				size: 0,
				sha256: "0".repeat(64),
				executable: false,
			},
		}
	}

	fn directory(path: &str) -> Entry {
		Entry {
			path: path.into(),
			kind: Kind::Directory,
		}
	}

	fn link(path: &str, target: &str) -> Entry {
		Entry {
			path: path.into(),
			kind: Kind::Symlink {
				target: target.into(),
			},
		}
	}

	#[test]
	fn unsafe_trees_are_rejected() {
		let unsafe_trees: Vec<Vec<Entry>> = vec![
			vec![file("../escape")],
			vec![file("/etc/passwd")],
			vec![file("a\\b")],
			vec![file("C:x")],
			vec![file("a//b")],
			vec![file("nested/file")],
			vec![file("same"), file("same")],
			vec![file("Readme"), file("README")],
			vec![
				directory("Frameworks"),
				link("Frameworks/Current", "../../outside"),
			],
			vec![link("Versions", "A"), file("Versions/escape")],
			vec![link("abs", "/usr/bin")],
		];
		for tree in unsafe_trees {
			assert!(validate_entries(&tree).is_err(), "{tree:?}");
		}
	}

	#[test]
	fn a_framework_layout_is_accepted() {
		let tree = vec![
			directory("Contents"),
			directory("Contents/Frameworks"),
			directory("Contents/Frameworks/A.framework"),
			directory("Contents/Frameworks/A.framework/Versions"),
			directory("Contents/Frameworks/A.framework/Versions/B"),
			file("Contents/Frameworks/A.framework/Versions/B/A"),
			link("Contents/Frameworks/A.framework/Versions/Current", "B"),
			link("Contents/Frameworks/A.framework/A", "Versions/Current/A"),
		];
		validate_entries(&tree).unwrap();
	}
}
