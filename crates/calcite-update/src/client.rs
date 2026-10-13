// SPDX-License-Identifier: MPL-2.0

//! Checking for, downloading, and staging an update. Every call blocks; run them off the
//! UI thread (for example in a `calcite_exec` task).

use std::{
	fs,
	io::{Read, Write},
	path::{Path, PathBuf},
};

use ed25519_dalek::VerifyingKey;
use sha2::{Digest, Sha256};

use crate::{
	Asset, Entry, Envelope, Error, Result, Version, install, manifest::Feed, sign::encode_hex, stage,
};

/// The largest `update.json` a client reads.
const MAX_FEED: u64 = 8 << 20;

/// What the app knows about itself.
#[derive(Clone, Debug)]
pub struct Updater {
	/// `…/update.json`; assets resolve next to it.
	pub feed_url: String,
	pub product: String,
	pub channel: String,
	pub current: Version,
	pub key: VerifyingKey,
	/// The installed tree, usually [`install::installed_root`].
	pub root: PathBuf,
}

/// A newer release for this app.
#[derive(Clone, Debug)]
pub struct Available {
	pub version: String,
	pub notes: String,
	entries: Vec<Entry>,
	full: Asset,
	delta: Option<Asset>,
}

impl Available {
	/// The bytes the update downloads: the delta when one starts from this version.
	pub fn download_size(&self) -> u64 {
		self.delta.as_ref().unwrap_or(&self.full).size
	}

	pub fn is_delta(&self) -> bool {
		self.delta.is_some()
	}
}

/// A verified tree, ready to replace the installed one.
#[derive(Debug)]
pub struct Staged {
	pub version: String,
	staging: PathBuf,
	entries: Vec<Entry>,
	root: PathBuf,
}

impl Staged {
	/// Swaps the new tree in. Call it when the app quits and no install or game runs.
	pub fn install(self) -> Result<()> {
		install::install(&self.staging, &self.entries, &self.root)
	}

	/// Drops the staged tree, for example when the user skips this version.
	pub fn discard(self) {
		let _ = fs::remove_dir_all(&self.staging);
	}
}

impl Updater {
	/// The newest release, if it is newer than this app. A feed that fails its signature,
	/// names another product or channel, or carries an unsafe tree is an error.
	pub fn check(&self) -> Result<Option<Available>> {
		let envelope: Envelope = {
			let mut text = String::new();
			get(&self.feed_url)?
				.take(MAX_FEED)
				.read_to_string(&mut text)
				.map_err(|error| Error::Network(error.to_string()))?;
			serde_json::from_str(&text).map_err(|error| Error::Feed(error.to_string()))?
		};
		let feed: Feed = envelope.open(&self.key)?;
		let Some(target) = feed.check(&self.product, &self.channel, &self.current)? else {
			return Ok(None);
		};
		let current = self.current.to_string();
		let delta = target
			.deltas
			.iter()
			.find(|delta| {
				delta
					.from
					.parse::<Version>()
					.is_ok_and(|from| from == self.current)
					|| delta.from == current
			})
			.filter(|delta| delta.asset.size < target.full.size)
			.map(|delta| delta.asset.clone());
		Ok(Some(Available {
			version: feed.version.clone(),
			notes: feed.notes.clone(),
			entries: target.entries.clone(),
			full: target.full.clone(),
			delta,
		}))
	}

	/// Downloads and stages `update`. `progress` gets downloaded and total bytes. A delta
	/// that does not fit the installed copy falls back to the full package.
	pub fn download(&self, update: &Available, mut progress: impl FnMut(u64, u64)) -> Result<Staged> {
		let downloads = install::download_path(&self.root);
		fs::create_dir_all(&downloads).map_err(|error| Error::io(&downloads, error))?;
		let staging = install::staging_path(&self.root);
		let installed_tree = self.root.is_dir();

		if let Some(delta) = update.delta.as_ref().filter(|_| installed_tree) {
			let package = self.fetch(delta, &downloads, &mut progress)?;
			match stage::stage_delta(&package, &update.entries, &self.root, &staging) {
				Ok(()) => return Ok(self.staged(update, staging)),
				Err(Error::Integrity(reason)) => {
					eprintln!("calcite-update: the delta does not apply ({reason}); using the full package");
				}
				Err(error) => return Err(error),
			}
		}
		let package = self.fetch(&update.full, &downloads, &mut progress)?;
		stage::stage_full(&package, &update.entries, &staging)?;
		Ok(self.staged(update, staging))
	}

	fn staged(&self, update: &Available, staging: PathBuf) -> Staged {
		Staged {
			version: update.version.clone(),
			staging,
			entries: update.entries.clone(),
			root: self.root.clone(),
		}
	}

	/// Downloads `asset` into `directory`, checking its size and hash from the signed feed.
	fn fetch(&self, asset: &Asset, directory: &Path, progress: &mut impl FnMut(u64, u64)) -> Result<PathBuf> {
		let url = asset_url(&self.feed_url, &asset.name);
		let path = directory.join(&asset.name);
		let mut response = get(&url)?.take(asset.size + 1);
		let mut file = fs::File::create(&path).map_err(|error| Error::io(&path, error))?;
		let mut hasher = Sha256::new();
		let mut buffer = vec![0; 1 << 16];
		let mut received = 0u64;
		loop {
			let count = response
				.read(&mut buffer)
				.map_err(|error| Error::Network(error.to_string()))?;
			if count == 0 {
				break;
			}
			received += count as u64;
			if received > asset.size {
				return Err(Error::Integrity(format!(
					"`{}` is larger than the feed says",
					asset.name
				)));
			}
			hasher.update(&buffer[..count]);
			file.write_all(&buffer[..count])
				.map_err(|error| Error::io(&path, error))?;
			progress(received, asset.size);
		}
		file.flush().map_err(|error| Error::io(&path, error))?;
		if received != asset.size || encode_hex(&hasher.finalize()) != asset.sha256 {
			let _ = fs::remove_file(&path);
			return Err(Error::Integrity(format!(
				"`{}` does not match the signed feed",
				asset.name
			)));
		}
		Ok(path)
	}
}

/// `name` next to the feed at `feed_url`.
pub fn asset_url(feed_url: &str, name: &str) -> String {
	match feed_url.rsplit_once('/') {
		Some((base, _)) => format!("{base}/{name}"),
		None => name.to_string(),
	}
}

fn get(url: &str) -> Result<impl Read> {
	// Local feeds make release dry runs and tests possible without a server.
	if let Some(path) = url.strip_prefix("file://") {
		return fs::File::open(path)
			.map(|file| Box::new(file) as Box<dyn Read>)
			.map_err(|error| Error::Network(format!("{url}: {error}")));
	}
	if !url.starts_with("https://") {
		return Err(Error::Network(format!("{url}: updates load over HTTPS only")));
	}
	let response = ureq::get(url)
		.call()
		.map_err(|error| Error::Network(format!("{url}: {error}")))?;
	Ok(Box::new(response.into_body().into_reader()) as Box<dyn Read>)
}
