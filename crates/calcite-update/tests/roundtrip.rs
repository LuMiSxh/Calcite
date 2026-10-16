// SPDX-License-Identifier: MPL-2.0

//! Publishes two releases into a temporary folder and updates an installed copy from a
//! `file://` feed, as a client would over HTTPS.

mod common;

use std::fs;

use calcite_update::{Envelope, Error, Kind, publish, stage::read_delta_index, target_id};
use common::*;

#[test]
fn a_delta_update_skips_unchanged_files_and_installs_the_new_tree() {
	let release = publish_two_releases("delta", "App.app");
	let updater = release.updater();
	let update = updater.check().unwrap().expect("1.1.0 is newer");
	assert!(update.is_delta());
	assert!(!update.is_required());
	assert!(
		update.download_size() < 50_000,
		"the delta leaves the runtime out: {}",
		update.download_size()
	);

	let mut package =
		fs::File::open(release.out.join(format!("App-1.0.0-1.1.0-{}.delta", target_id()))).unwrap();
	let (index, _) = read_delta_index(&mut package).unwrap();
	assert!(index.items.iter().all(|item| !item.path.contains("runtime")));

	let staged = updater.download(&update, |_, _| {}).unwrap();
	staged.install().unwrap();
	assert_eq!(tree(&release.installed), tree(&release.new_root));
}

#[test]
fn a_modified_installed_copy_falls_back_to_the_full_package() {
	let release = publish_two_releases("fallback", "App.app");
	fs::write(release.installed.join("Contents/MacOS/app"), b"tampered").unwrap();
	let updater = release.updater();
	let update = updater.check().unwrap().unwrap();
	updater.download(&update, |_, _| {}).unwrap().install().unwrap();
	assert_eq!(tree(&release.installed), tree(&release.new_root));
}

#[test]
fn a_forged_feed_or_package_is_rejected() {
	let release = publish_two_releases("forged", "App.app");
	let original = fs::read_to_string(release.feed_path()).unwrap();

	// Another version in the signed text: the signature no longer matches.
	let mut envelope: Envelope = serde_json::from_str(&original).unwrap();
	envelope.feed = envelope.feed.replace("1.1.0", "9.9.9");
	fs::write(release.feed_path(), serde_json::to_string(&envelope).unwrap()).unwrap();
	assert!(matches!(release.updater().check(), Err(Error::Signature(_))));

	// A key that is not the app's.
	fs::write(release.feed_path(), &original).unwrap();
	let (_, other_key) = publish::generate_key().unwrap();
	let stranger = release.builder_trusting(&other_key).build().unwrap();
	assert!(matches!(stranger.check(), Err(Error::Signature(_))));

	// A feed for another product, and one that is not newer.
	let other = release.builder().product("dev.example.other").build().unwrap();
	assert!(matches!(other.check(), Err(Error::Feed(_))));
	let current = release
		.builder()
		.current("1.1.0".parse().unwrap())
		.build()
		.unwrap();
	assert!(current.check().unwrap().is_none());

	// A swapped package: the hash in the signed feed catches it.
	let updater = release.updater();
	let update = updater.check().unwrap().unwrap();
	for entry in fs::read_dir(&release.out).unwrap().flatten() {
		let name = entry.file_name().to_string_lossy().into_owned();
		if name.ends_with(".delta") {
			let mut bytes = fs::read(entry.path()).unwrap();
			let last = bytes.len() - 1;
			bytes[last] ^= 0xff;
			fs::write(entry.path(), bytes).unwrap();
		} else if name.ends_with(".full.zst") {
			fs::write(entry.path(), b"not the package").unwrap();
		}
	}
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
	let release = publish_two_releases("scan", "App.app");
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
