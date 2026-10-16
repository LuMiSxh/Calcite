// SPDX-License-Identifier: MPL-2.0

//! Checking for, downloading, and staging an update. Every call blocks; run them off the
//! UI thread (for example in a `calcite_exec` task).

use std::{
	collections::BTreeMap,
	fmt, fs,
	io::{Read, Write},
	path::{Path, PathBuf},
	sync::Arc,
	time::{Duration, SystemTime, UNIX_EPOCH},
};

use ed25519_dalek::VerifyingKey;
use sha2::{Digest, Sha256};

use crate::{
	Asset, Entry, Envelope, Error, Result, Version, fsutil, install,
	manifest::{Feed, MAX_ASSET_SIZE},
	sign::encode_hex,
	stage, state,
};

/// The largest `update.json` a client reads.
const MAX_FEED: u64 = 8 << 20;
/// How long a client waits to connect.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// How long a request may take to answer, and the base time of a body.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
/// The slowest transfer a download tolerates. It sets the time a body may take beyond
/// [`REQUEST_TIMEOUT`].
const MIN_BYTES_PER_SECOND: u64 = 64 << 10;
/// A feed older than this is rejected by default.
pub const DEFAULT_MAX_FEED_AGE: Duration = Duration::from_secs(30 * 24 * 60 * 60);

/// The time source of an [`Updater`]. Tests give it a manual clock.
pub trait Clock: Send + Sync + fmt::Debug {
	fn now(&self) -> SystemTime;
}

/// The system clock.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
	fn now(&self) -> SystemTime {
		SystemTime::now()
	}
}

/// What the app knows about itself. Build it with [`Updater::builder`].
#[derive(Clone, Debug)]
pub struct Updater {
	feed_url: String,
	product: String,
	channel: String,
	current: Version,
	keys: BTreeMap<String, VerifyingKey>,
	root: PathBuf,
	state_dir: PathBuf,
	max_feed_age: Duration,
	clock: Arc<dyn Clock>,
	allow_file_urls: bool,
}

/// Collects what an [`Updater`] needs. `feed_url`, `product`, `channel`, `current`, `root`,
/// and `state_dir` are required.
#[derive(Debug)]
pub struct UpdaterBuilder {
	keys: BTreeMap<String, VerifyingKey>,
	feed_url: Option<String>,
	product: Option<String>,
	channel: Option<String>,
	current: Option<Version>,
	root: Option<PathBuf>,
	state_dir: Option<PathBuf>,
	max_feed_age: Duration,
	clock: Arc<dyn Clock>,
	allow_file_urls: bool,
}

impl UpdaterBuilder {
	/// `…/update.json`; assets resolve next to it. HTTPS only.
	pub fn feed_url(mut self, url: impl Into<String>) -> Self {
		self.feed_url = Some(url.into());
		self
	}

	pub fn product(mut self, product: impl Into<String>) -> Self {
		self.product = Some(product.into());
		self
	}

	pub fn channel(mut self, channel: impl Into<String>) -> Self {
		self.channel = Some(channel.into());
		self
	}

	/// The running version.
	pub fn current(mut self, version: Version) -> Self {
		self.current = Some(version);
		self
	}

	/// The installed tree, usually [`install::installed_root`].
	pub fn root(mut self, root: impl Into<PathBuf>) -> Self {
		self.root = Some(root.into());
		self
	}

	/// A folder the app owns for the update state (`update-state.json`), such as its
	/// per-user data folder. Do not put it inside the installed tree.
	pub fn state_dir(mut self, directory: impl Into<PathBuf>) -> Self {
		self.state_dir = Some(directory.into());
		self
	}

	/// How old a feed may be, measured from its signed `issued_at`. The default is 30 days.
	pub fn max_feed_age(mut self, age: Duration) -> Self {
		self.max_feed_age = age;
		self
	}

	pub fn clock(mut self, clock: impl Clock + 'static) -> Self {
		self.clock = Arc::new(clock);
		self
	}

	/// Accepts `file://` URLs for the feed and its assets.
	#[cfg(feature = "test-support")]
	pub fn allow_file_urls(mut self) -> Self {
		self.allow_file_urls = true;
		self
	}

	pub fn build(self) -> Result<Updater> {
		fn need<T>(value: Option<T>, name: &str) -> Result<T> {
			value.ok_or_else(|| Error::Feed(format!("the updater needs `{name}`")))
		}
		if self.keys.is_empty() {
			return Err(Error::Signature("the updater trusts no key".into()));
		}
		Ok(Updater {
			feed_url: need(self.feed_url, "feed_url")?,
			product: need(self.product, "product")?,
			channel: need(self.channel, "channel")?,
			current: need(self.current, "current")?,
			keys: self.keys,
			root: need(self.root, "root")?,
			state_dir: need(self.state_dir, "state_dir")?,
			max_feed_age: self.max_feed_age,
			clock: self.clock,
			allow_file_urls: self.allow_file_urls,
		})
	}
}

/// A newer release for this app.
#[derive(Clone, Debug)]
pub struct Available {
	pub version: String,
	pub notes: String,
	required: bool,
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

	/// Whether the running version is below the feed's `minimum_version`.
	pub fn is_required(&self) -> bool {
		self.required
	}
}

/// A verified tree, ready to replace the installed one.
#[derive(Debug)]
pub struct Staged {
	pub version: String,
	product: String,
	staging: PathBuf,
	entries: Vec<Entry>,
	root: PathBuf,
}

impl Staged {
	/// Swaps the new tree in. Call it when the app quits and no install or game runs. The
	/// staged files are hashed again first.
	pub fn install(self) -> Result<()> {
		install::install(
			&self.staging,
			&self.entries,
			&self.root,
			&self.product,
			&self.version,
		)
	}

	/// Drops the staged tree, for example when the user skips this version.
	pub fn discard(self) {
		let _ = fs::remove_dir_all(&self.staging);
	}
}

impl Updater {
	/// Starts an updater that trusts these keys, named by the `key_id` of an envelope. List
	/// the old and the new key during a rotation.
	pub fn builder(trusted_keys: impl IntoIterator<Item = (String, VerifyingKey)>) -> UpdaterBuilder {
		UpdaterBuilder {
			keys: trusted_keys.into_iter().collect(),
			feed_url: None,
			product: None,
			channel: None,
			current: None,
			root: None,
			state_dir: None,
			max_feed_age: DEFAULT_MAX_FEED_AGE,
			clock: Arc::new(SystemClock),
			allow_file_urls: false,
		}
	}

	/// The newest release, if it is newer than this app. A feed that fails its signature,
	/// names another product or channel, is a replay or too old, or carries an unsafe tree is
	/// an error. A feed that passes records its sequence, even when it offers no update.
	pub fn check(&self) -> Result<Option<Available>> {
		let envelope: Envelope = {
			let mut text = String::new();
			self.get(&self.feed_url, MAX_FEED)?
				.take(MAX_FEED)
				.read_to_string(&mut text)
				.map_err(|error| Error::Network(error.to_string()))?;
			serde_json::from_str(&text).map_err(|error| Error::Feed(error.to_string()))?
		};
		let feed: Feed = envelope.open(&self.keys)?;
		let now = self
			.clock
			.now()
			.duration_since(UNIX_EPOCH)
			.map_or(0, |elapsed| elapsed.as_secs());
		let highest = state::highest_sequence(&self.state_dir)?;
		feed.check_fresh(now, self.max_feed_age, highest)?;
		let target = feed.check(&self.product, &self.channel, &self.current)?;
		state::record(&self.state_dir, feed.sequence)?;
		let Some(target) = target else {
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
		let required = feed.minimum()?.is_some_and(|minimum| self.current < minimum);
		Ok(Some(Available {
			version: feed.version.clone(),
			notes: feed.notes.clone(),
			required,
			entries: target.entries.clone(),
			full: target.full.clone(),
			delta,
		}))
	}

	/// Downloads and stages `update`. `progress` gets downloaded and total bytes. A delta
	/// that does not fit the installed copy falls back to the full package.
	pub fn download(&self, update: &Available, mut progress: impl FnMut(u64, u64)) -> Result<Staged> {
		let downloads = install::download_path(&self.root);
		fsutil::ensure_real_dir(&downloads).map_err(|error| Error::io(&downloads, error))?;
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
			product: self.product.clone(),
			staging,
			entries: update.entries.clone(),
			root: self.root.clone(),
		}
	}

	/// Downloads `asset` into `directory`, checking its size and hash from the signed feed.
	fn fetch(&self, asset: &Asset, directory: &Path, progress: &mut impl FnMut(u64, u64)) -> Result<PathBuf> {
		let url = asset_url(&self.feed_url, &asset.name);
		let path = directory.join(&asset.name);
		// A file from an earlier attempt goes; anything else in the way (a link, a folder) is
		// an error. The new file must not exist when it opens.
		match fs::symlink_metadata(&path) {
			Ok(metadata) if metadata.is_file() => {
				fs::remove_file(&path).map_err(|error| Error::io(&path, error))?;
			}
			Ok(_) => {
				return Err(Error::Integrity(format!(
					"`{}` in the download folder is not a plain file",
					asset.name
				)));
			}
			Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
			Err(error) => return Err(Error::io(&path, error)),
		}
		if asset.size > MAX_ASSET_SIZE {
			return Err(Error::Feed(format!(
				"`{}` is larger than a package may be",
				asset.name
			)));
		}
		let mut response = self.get(&url, asset.size)?.take(asset.size.saturating_add(1));
		let mut file = fs::OpenOptions::new()
			.write(true)
			.create_new(true)
			.open(&path)
			.map_err(|error| Error::io(&path, error))?;
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

	/// Opens `url`. `expected` is the body size; it sets how long the transfer may take.
	fn get(&self, url: &str, expected: u64) -> Result<Box<dyn Read>> {
		if let Some(path) = url.strip_prefix("file://") {
			if !self.allow_file_urls {
				return Err(Error::Network(format!("{url}: updates load over HTTPS only")));
			}
			return fs::File::open(path)
				.map(|file| Box::new(file) as Box<dyn Read>)
				.map_err(|error| Error::Network(format!("{url}: {error}")));
		}
		if !url.starts_with("https://") {
			return Err(Error::Network(format!("{url}: updates load over HTTPS only")));
		}
		let body_time = REQUEST_TIMEOUT + Duration::from_secs(expected / MIN_BYTES_PER_SECOND);
		let agent: ureq::Agent = ureq::Agent::config_builder()
			.https_only(true)
			.timeout_connect(Some(CONNECT_TIMEOUT))
			.timeout_recv_response(Some(REQUEST_TIMEOUT))
			.timeout_recv_body(Some(body_time))
			.build()
			.into();
		let response = agent
			.get(url)
			.call()
			.map_err(|error| Error::Network(format!("{url}: {error}")))?;
		Ok(Box::new(response.into_body().into_reader()))
	}
}

/// `name` next to the feed at `feed_url`.
pub fn asset_url(feed_url: &str, name: &str) -> String {
	match feed_url.rsplit_once('/') {
		Some((base, _)) => format!("{base}/{name}"),
		None => name.to_string(),
	}
}
