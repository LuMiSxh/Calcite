// SPDX-License-Identifier: MPL-2.0

//! The update security contracts: key rotation, rollback and freeze, a tampered staging
//! folder, and the ownership of the installed folder.

mod common;

use std::fs;

use calcite_update::{Error, Updater, install, publish, verifying_key};
use common::*;

#[test]
fn only_a_trusted_key_id_with_a_matching_signature_is_accepted() {
	let release = publish_two_releases("keys", "App.app");
	let (new_seed, new_public) = publish::generate_key().unwrap();
	let new_key = publish::signing_key(&new_seed).unwrap();
	let feed = release.feed(1, START);

	// An unknown key ID fails before the feed is read.
	release.publish_feed(&feed, "2027", &new_key);
	assert!(matches!(release.updater().check(), Err(Error::Signature(reason)) if reason.contains("2027")));

	// During a rotation the app trusts both keys, and the feed names the one that signed it.
	let rotating = Updater::builder([
		(KEY_ID.to_string(), verifying_key(&release.public_key).unwrap()),
		("2027".to_string(), verifying_key(&new_public).unwrap()),
	])
	.feed_url(format!("file://{}", release.feed_path().display()))
	.product("dev.example.app")
	.channel("stable")
	.current("1.0.0".parse().unwrap())
	.root(&release.installed)
	.state_dir(&release.state_dir)
	.clock(release.clock.clone())
	.allow_file_urls()
	.build()
	.unwrap();
	assert!(rotating.check().unwrap().is_some());

	// A trusted ID with the signature of another key.
	release.publish_feed(&feed, KEY_ID, &new_key);
	assert!(matches!(rotating.check(), Err(Error::Signature(_))));
}

#[test]
fn a_replayed_expired_or_future_feed_is_rejected() {
	let release = publish_two_releases("stale", "App.app");
	let key = &release.signing_key;
	let updater = release.updater();

	release.publish_feed(&release.feed(5, START), KEY_ID, key);
	assert!(updater.check().unwrap().is_some());

	// Replay: an older signed feed is genuine, but its sequence is lower.
	release.publish_feed(&release.feed(4, START), KEY_ID, key);
	assert!(matches!(updater.check(), Err(Error::Stale(_))));
	release.publish_feed(&release.feed(5, START), KEY_ID, key);
	assert!(
		updater.check().unwrap().is_some(),
		"the same sequence is not a replay"
	);

	// Freeze: the feed ages out, and a fresh one is accepted.
	release.clock.advance(31 * DAY);
	assert!(matches!(updater.check(), Err(Error::Stale(_))));
	release.publish_feed(&release.feed(6, START + 31 * DAY), KEY_ID, key);
	assert!(updater.check().unwrap().is_some());

	// A feed dated beyond the clock skew.
	release.publish_feed(&release.feed(7, START + 31 * DAY + 600), KEY_ID, key);
	assert!(matches!(updater.check(), Err(Error::Stale(_))));
}

#[test]
fn updates_load_over_https_only() {
	let release = publish_two_releases("https", "App.app");
	let strict = Updater::builder([(KEY_ID.to_string(), verifying_key(&release.public_key).unwrap())])
		.feed_url(format!("file://{}", release.feed_path().display()))
		.product("dev.example.app")
		.channel("stable")
		.current("1.0.0".parse().unwrap())
		.root(&release.installed)
		.state_dir(&release.state_dir)
		.build()
		.unwrap();
	assert!(matches!(strict.check(), Err(Error::Network(_))));
}

#[test]
fn a_staged_file_changed_after_staging_is_caught_before_the_install() {
	let release = publish_two_releases("tamper", "App.app");
	let updater = release.updater();
	let update = updater.check().unwrap().unwrap();
	let staged = updater.download(&update, |_, _| {}).unwrap();
	let staging = install::staging_path(&release.installed);
	fs::write(staging.join("Contents/Info.plist"), b"<version>evil</version>").unwrap();

	assert!(matches!(staged.install(), Err(Error::Integrity(_))));
	assert_eq!(
		fs::read_to_string(release.installed.join("Contents/Info.plist")).unwrap(),
		"<version>1</version>",
		"the installed copy stays"
	);
}

#[test]
fn an_update_keeps_files_it_does_not_own() {
	// A plain folder, not a macOS `.app` bundle, so the install cannot swap it whole.
	let release = publish_two_releases("unowned", "AppDir");
	fs::write(release.installed.join("notes.txt"), "mine").unwrap();
	fs::create_dir(release.installed.join("plugins")).unwrap();
	fs::write(release.installed.join("plugins/extra.dll"), "mine too").unwrap();

	let updater = release.updater();
	let update = updater.check().unwrap().unwrap();
	updater.download(&update, |_, _| {}).unwrap().install().unwrap();

	assert_eq!(
		fs::read_to_string(release.installed.join("notes.txt")).unwrap(),
		"mine"
	);
	assert_eq!(
		fs::read_to_string(release.installed.join("plugins/extra.dll")).unwrap(),
		"mine too"
	);
	assert_eq!(
		fs::read_to_string(release.installed.join("Contents/Info.plist")).unwrap(),
		"<version>2</version>"
	);
	assert!(release.installed.join(MARKER).exists());
	assert!(!release.installed.join(".calcite-install-journal.json").exists());
}

#[cfg(unix)]
#[test]
fn a_download_never_writes_through_a_link_at_the_target() {
	let release = publish_two_releases("link", "App.app");
	let updater = release.updater();
	let update = updater.check().unwrap().unwrap();
	let downloads = install::download_path(&release.installed);
	fs::create_dir_all(&downloads).unwrap();
	let victim = release.temp.0.join("victim.txt");
	fs::write(&victim, "precious").unwrap();
	let name = format!("App-1.0.0-1.1.0-{}.delta", calcite_update::target_id());
	std::os::unix::fs::symlink(&victim, downloads.join(name)).unwrap();

	assert!(matches!(
		updater.download(&update, |_, _| {}),
		Err(Error::Integrity(_))
	));
	assert_eq!(fs::read_to_string(&victim).unwrap(), "precious");
}
