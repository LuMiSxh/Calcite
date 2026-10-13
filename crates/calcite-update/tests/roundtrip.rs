// SPDX-License-Identifier: MPL-2.0

//! Publishes two releases into a temporary folder and updates an installed copy from a
//! `file://` feed, as a client would over HTTPS.

use std::{
	collections::BTreeMap,
	fs,
	path::{Path, PathBuf},
	time::{SystemTime, UNIX_EPOCH},
};

use calcite_update::{
	Delta, Envelope, Error, Feed, Kind, Target, Updater, publish, stage::read_delta_index, target_id,
	verifying_key,
};

struct Temp(PathBuf);

impl Temp {
	fn new(name: &str) -> Self {
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

/// Deterministic bytes that compress poorly, like a binary.
fn noise(seed: u64, length: usize) -> Vec<u8> {
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
fn write_release(root: &Path, version: u8) {
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

fn copy_tree(from: &Path, to: &Path) {
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

struct Release {
	_temp: Temp,
	feed_url: String,
	public_key: String,
	installed: PathBuf,
	out: PathBuf,
}

fn publish_two_releases(name: &str) -> Release {
	let temp = Temp::new(name);
	let (old_root, new_root, out) = (
		temp.0.join("1.0.0/App.app"),
		temp.0.join("1.1.0/App.app"),
		temp.0.join("out"),
	);
	write_release(&old_root, 1);
	write_release(&new_root, 2);
	fs::create_dir_all(&out).unwrap();

	let old_entries = publish::scan(&old_root).unwrap();
	let new_entries = publish::scan(&new_root).unwrap();
	let full = publish::write_full(&new_root, &new_entries, &out.join("App-1.1.0.full.zst")).unwrap();
	let delta = publish::write_delta(
		&old_root,
		&old_entries,
		&new_root,
		&new_entries,
		"1.0.0",
		"1.1.0",
		&out.join("App-1.0.0-1.1.0.delta"),
	)
	.unwrap();
	let feed = Feed {
		format: Feed::FORMAT,
		product: "dev.example.app".into(),
		channel: "stable".into(),
		version: "1.1.0".into(),
		notes: "Notes".into(),
		targets: BTreeMap::from([(
			target_id(),
			Target {
				entries: new_entries,
				full,
				deltas: vec![Delta {
					from: "1.0.0".into(),
					asset: delta,
				}],
			},
		)]),
	};
	let (seed, public_key) = publish::generate_key().unwrap();
	let envelope = publish::sign(&feed, &publish::signing_key(&seed).unwrap()).unwrap();
	fs::write(out.join("update.json"), serde_json::to_string(&envelope).unwrap()).unwrap();

	let installed = temp.0.join("installed/App.app");
	copy_tree(&old_root, &installed);
	Release {
		feed_url: format!("file://{}", out.join("update.json").display()),
		public_key,
		installed,
		out,
		_temp: temp,
	}
}

fn updater(release: &Release) -> Updater {
	Updater {
		feed_url: release.feed_url.clone(),
		product: "dev.example.app".into(),
		channel: "stable".into(),
		current: "1.0.0".parse().unwrap(),
		key: verifying_key(&release.public_key).unwrap(),
		root: release.installed.clone(),
	}
}

fn tree_without_hashes_differing(root: &Path) -> Vec<calcite_update::Entry> {
	publish::scan(root).unwrap()
}

#[test]
fn a_delta_update_skips_unchanged_files_and_installs_the_new_tree() {
	let release = publish_two_releases("delta");
	let updater = updater(&release);
	let update = updater.check().unwrap().expect("1.1.0 is newer");
	assert!(update.is_delta());
	assert!(
		update.download_size() < 50_000,
		"the delta leaves the runtime out: {}",
		update.download_size()
	);

	let mut package = fs::File::open(release.out.join("App-1.0.0-1.1.0.delta")).unwrap();
	let (index, _) = read_delta_index(&mut package).unwrap();
	assert!(index.items.iter().all(|item| !item.path.contains("runtime")));

	let staged = updater.download(&update, |_, _| {}).unwrap();
	staged.install().unwrap();
	let expected = publish::scan(&release.out.join("../1.1.0/App.app")).unwrap();
	assert_eq!(tree_without_hashes_differing(&release.installed), expected);
}

#[test]
fn a_modified_installed_copy_falls_back_to_the_full_package() {
	let release = publish_two_releases("fallback");
	fs::write(release.installed.join("Contents/MacOS/app"), b"tampered").unwrap();
	let updater = updater(&release);
	let update = updater.check().unwrap().unwrap();
	updater.download(&update, |_, _| {}).unwrap().install().unwrap();
	let expected = publish::scan(&release.out.join("../1.1.0/App.app")).unwrap();
	assert_eq!(publish::scan(&release.installed).unwrap(), expected);
}

#[test]
fn a_forged_feed_or_package_is_rejected() {
	let release = publish_two_releases("forged");
	let feed_path = release.out.join("update.json");
	let original = fs::read_to_string(&feed_path).unwrap();

	// Another version in the signed text: the signature no longer matches.
	let mut envelope: Envelope = serde_json::from_str(&original).unwrap();
	envelope.feed = envelope.feed.replace("1.1.0", "9.9.9");
	fs::write(&feed_path, serde_json::to_string(&envelope).unwrap()).unwrap();
	assert!(matches!(updater(&release).check(), Err(Error::Signature(_))));

	// A key that is not the app's.
	fs::write(&feed_path, &original).unwrap();
	let (_, other_key) = publish::generate_key().unwrap();
	let mut stranger = updater(&release);
	stranger.key = verifying_key(&other_key).unwrap();
	assert!(matches!(stranger.check(), Err(Error::Signature(_))));

	// A feed for another product, and one that is not newer.
	let mut other = updater(&release);
	other.product = "dev.example.other".into();
	assert!(matches!(other.check(), Err(Error::Feed(_))));
	let mut current = updater(&release);
	current.current = "1.1.0".parse().unwrap();
	assert!(current.check().unwrap().is_none());

	// A swapped package: the hash in the signed feed catches it.
	let updater = updater(&release);
	let update = updater.check().unwrap().unwrap();
	let delta = release.out.join("App-1.0.0-1.1.0.delta");
	let mut bytes = fs::read(&delta).unwrap();
	let last = bytes.len() - 1;
	bytes[last] ^= 0xff;
	fs::write(&delta, bytes).unwrap();
	let full = release.out.join("App-1.1.0.full.zst");
	fs::write(&full, b"not the package").unwrap();
	assert!(matches!(
		updater.download(&update, |_, _| {}),
		Err(Error::Integrity(_))
	));
	assert!(
		release.installed.join("Contents/Info.plist").exists(),
		"the installed tree stays"
	);
}

#[test]
fn the_scanned_tree_keeps_links_and_executables() {
	let release = publish_two_releases("scan");
	let entries = publish::scan(&release.installed).unwrap();
	#[cfg(unix)]
	{
		assert!(
			entries
				.iter()
				.any(|entry| matches!(&entry.kind, Kind::Symlink { target } if target == "MacOS/app"))
		);
		assert!(entries.iter().any(|entry| entry.path == "Contents/MacOS/app"
			&& matches!(entry.kind, Kind::File { executable: true, .. })));
	}
	assert!(
		entries
			.iter()
			.any(|entry| entry.path == "Contents" && entry.kind == Kind::Directory)
	);
}
