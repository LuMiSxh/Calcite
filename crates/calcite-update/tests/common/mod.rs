// SPDX-License-Identifier: MPL-2.0

//! Shared fixtures: two published releases in a temporary folder, a manual clock, and an
//! installed copy of the older release.

#![allow(dead_code)]

use std::{
	fs,
	path::{Path, PathBuf},
	sync::{Arc, Mutex},
	time::{Duration, SystemTime, UNIX_EPOCH},
};

use calcite_update::{
	Clock, Entry, Feed, Updater, UpdaterBuilder, publish, publish::FeedBuilder, target_id, verifying_key,
};
use ed25519_dalek::SigningKey;

pub const KEY_ID: &str = "2026";
/// The manual clock starts here, in Unix seconds.
pub const START: u64 = 1_800_000_000;
pub const DAY: u64 = 24 * 60 * 60;
/// The install marker an install writes into a shared folder.
pub const MARKER: &str = ".calcite-install.json";

pub struct Temp(pub PathBuf);

impl Temp {
	pub fn new(name: &str) -> Self {
		let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
		let path = std::env::temp_dir().join(format!("calcite-update-{name}-{}-{nanos}", std::process::id()));
		fs::create_dir_all(&path).unwrap();
		Self(path)
	}
}

impl Drop for Temp {
	fn drop(&mut self) {
		let _ = fs::remove_dir_all(&self.0);
	}
}

/// A clock that only moves when a test says so.
#[derive(Clone, Debug)]
pub struct ManualClock(Arc<Mutex<u64>>);

impl ManualClock {
	pub fn new(seconds: u64) -> Self {
		Self(Arc::new(Mutex::new(seconds)))
	}

	pub fn advance(&self, seconds: u64) {
		*self.0.lock().unwrap() += seconds;
	}
}

impl Clock for ManualClock {
	fn now(&self) -> SystemTime {
		UNIX_EPOCH + Duration::from_secs(*self.0.lock().unwrap())
	}
}

/// Deterministic bytes that compress poorly, like a binary.
pub fn noise(seed: u64, length: usize) -> Vec<u8> {
	let mut state = seed;
	(0..length)
		.map(|_| {
			state = state
				.wrapping_mul(6364136223846793005)
				.wrapping_add(1442695040888963407);
			(state >> 33) as u8
		})
		.collect()
}

/// A bundle: an unchanged "runtime", a binary that changes a little, and a framework link.
pub fn write_release(root: &Path, version: u8) {
	fs::create_dir_all(root.join("Contents/MacOS")).unwrap();
	fs::create_dir_all(root.join("Contents/Resources/runtime")).unwrap();
	fs::write(
		root.join("Contents/Resources/runtime/wine.bin"),
		noise(7, 400_000),
	)
	.unwrap();
	let mut binary = noise(11, 120_000);
	binary[5_000..5_016].copy_from_slice(&[version; 16]);
	fs::write(root.join("Contents/MacOS/app"), binary).unwrap();
	fs::write(
		root.join("Contents/Info.plist"),
		format!("<version>{version}</version>"),
	)
	.unwrap();
	if version >= 2 {
		fs::write(root.join("Contents/Resources/new.txt"), "added in 2").unwrap();
	}
	#[cfg(unix)]
	{
		use std::os::unix::fs::PermissionsExt;
		fs::set_permissions(root.join("Contents/MacOS/app"), fs::Permissions::from_mode(0o755)).unwrap();
		std::os::unix::fs::symlink("MacOS/app", root.join("Contents/app-link")).unwrap();
	}
}

pub fn copy_tree(from: &Path, to: &Path) {
	fs::create_dir_all(to).unwrap();
	for item in fs::read_dir(from).unwrap().flatten() {
		let target = to.join(item.file_name());
		let kind = item.file_type().unwrap();
		if kind.is_symlink() {
			#[cfg(unix)]
			std::os::unix::fs::symlink(fs::read_link(item.path()).unwrap(), &target).unwrap();
		} else if kind.is_dir() {
			copy_tree(&item.path(), &target);
		} else {
			fs::copy(item.path(), &target).unwrap();
		}
	}
}

/// The scanned tree without the install marker, which an install adds.
pub fn tree(root: &Path) -> Vec<Entry> {
	let mut entries = publish::scan(root).unwrap();
	entries.retain(|entry| entry.path != MARKER);
	entries
}

pub struct Release {
	pub temp: Temp,
	pub clock: ManualClock,
	pub signing_key: SigningKey,
	pub public_key: String,
	/// Where the feed, the full package, and the delta are.
	pub out: PathBuf,
	pub new_root: PathBuf,
	pub installed: PathBuf,
	pub state_dir: PathBuf,
	target: calcite_update::Target,
}

impl Release {
	pub fn feed_path(&self) -> PathBuf {
		self.out.join("update.json")
	}

	/// A feed for 1.1.0 with these properties.
	pub fn feed(&self, sequence: u64, issued_at: u64) -> Feed {
		FeedBuilder::new("dev.example.app", "stable", "1.1.0", sequence, issued_at)
			.notes("Notes")
			.target(target_id(), self.target.clone())
			.build()
	}

	/// Signs `feed` and publishes it as `update.json`.
	pub fn publish_feed(&self, feed: &Feed, key_id: &str, key: &SigningKey) {
		fs::write(self.feed_path(), publish::sign_feed(feed, key_id, key).unwrap()).unwrap();
	}

	/// A builder for the app at 1.0.0 that trusts this release's key.
	pub fn builder(&self) -> UpdaterBuilder {
		self.builder_trusting(&self.public_key)
	}

	/// Like [`Release::builder`], but trusting `public_key` as key `KEY_ID`.
	pub fn builder_trusting(&self, public_key: &str) -> UpdaterBuilder {
		Updater::builder([(KEY_ID.to_string(), verifying_key(public_key).unwrap())])
			.feed_url(format!("file://{}", self.feed_path().display()))
			.product("dev.example.app")
			.channel("stable")
			.current("1.0.0".parse().unwrap())
			.root(&self.installed)
			.state_dir(&self.state_dir)
			.clock(self.clock.clone())
			.allow_file_urls()
	}

	pub fn updater(&self) -> Updater {
		self.builder().build().unwrap()
	}
}

/// Publishes 1.1.0 (a full package and a delta from 1.0.0), signs a feed with sequence 1, and
/// installs 1.0.0 at `installed/<installed_name>`.
pub fn publish_two_releases(name: &str, installed_name: &str) -> Release {
	let temp = Temp::new(name);
	let (old_root, new_root, out) = (
		temp.0.join("1.0.0/App.app"),
		temp.0.join("1.1.0/App.app"),
		temp.0.join("out"),
	);
	write_release(&old_root, 1);
	write_release(&new_root, 2);
	let target = publish::build_target(
		&new_root,
		&out,
		"App",
		"1.1.0",
		&target_id(),
		&[("1.0.0".to_string(), old_root.clone())],
	)
	.unwrap();

	let (seed, public_key) = publish::generate_key().unwrap();
	let installed = temp.0.join("installed").join(installed_name);
	copy_tree(&old_root, &installed);
	let release = Release {
		clock: ManualClock::new(START),
		signing_key: publish::signing_key(&seed).unwrap(),
		public_key,
		state_dir: temp.0.join("state"),
		out,
		new_root,
		installed,
		target,
		temp,
	};
	release.publish_feed(&release.feed(1, START), KEY_ID, &release.signing_key);
	release
}
