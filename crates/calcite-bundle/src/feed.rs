// SPDX-License-Identifier: MPL-2.0

//! `--feed`: a signed calcite-update feed, `update.json`, beside the bundle that calcite-update
//! installs. The tree hashing, the packages, and the signature come from the `publish` feature
//! of `calcite-update`.

use std::fs;

use calcite_update::{
	Version,
	publish::{self, FeedBuilder},
	target_id,
};

use crate::{
	config::{Settings, UPDATE_TABLE, UpdateSettings},
	package::{FeedOptions, Output, Result},
	stage::Staging,
};

/// The outputs that calcite-update installs: the `.app` tree, the AppImage, and the standalone
/// exe. Installers such as DMG, NSIS, MSI, deb, and pacman need their own install path.
const INSTALLABLE: [&str; 3] = ["app", "appimage", "exe"];

/// What `--feed` writes. It is resolved before the packagers run, so a missing setting fails early.
pub struct Plan {
	product: String,
	channel: String,
	name: String,
	version: String,
	sequence: u64,
	notes: String,
	minimum_version: Option<String>,
	key_id: String,
	/// The hex seed. It is never printed, and it is not in any error message.
	seed: String,
}

impl Plan {
	/// Reads the update table, the sequence, the notes, and the signing seed. `env` reads
	/// environment variables; tests pass a closure. `identifier` is the product, and `name`
	/// prefixes the package file names.
	pub fn resolve(
		request: &FeedOptions,
		settings: &Settings,
		manifest: &str,
		identifier: &str,
		name: &str,
		version: &str,
		env: impl Fn(&str) -> Option<String>,
	) -> Result<Self> {
		let update = UpdateSettings::from_settings(settings, manifest)?
			.ok_or_else(|| format!("{manifest}: --feed needs a {UPDATE_TABLE} table"))?;
		let base_url = update
			.base_url
			.as_deref()
			.ok_or_else(|| format!("{manifest}: {UPDATE_TABLE} needs `base-url` for --feed"))?;
		if !base_url.starts_with("https://") {
			return Err(format!(
				"{manifest}: {UPDATE_TABLE} `base-url` must be an https:// URL"
			));
		}
		let key_id = update
			.key_id
			.ok_or_else(|| format!("{manifest}: {UPDATE_TABLE} needs `key-id` for --feed"))?;
		let sequence = request.sequence.ok_or("--sequence is required with --feed")?;
		let seed = env("CALCITE_UPDATE_KEY")
			.ok_or("CALCITE_UPDATE_KEY is not set: it holds the calcite-update signing seed")?;
		publish::signing_key(seed.trim()).map_err(|error| format!("CALCITE_UPDATE_KEY: {error}"))?;
		let notes = match &request.notes {
			Some(path) => fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?,
			None => String::new(),
		};
		if let Some(minimum) = &update.minimum_version {
			minimum.parse::<Version>()?;
		}
		Ok(Self {
			product: identifier.to_string(),
			channel: update.channel,
			name: name.to_string(),
			version: version.to_string(),
			sequence,
			notes,
			minimum_version: update.minimum_version,
			key_id,
			seed: seed.trim().to_string(),
		})
	}

	/// Packages the installable output as the calcite-update target, signs the feed, and stages
	/// `update.json` with the packages. The result joins the bundles in one commit.
	pub fn write(&self, staging: &Staging, outputs: &[Output]) -> Result<Output> {
		let installable: Vec<&Output> = outputs
			.iter()
			.filter(|output| INSTALLABLE.contains(&output.format.as_str()))
			.collect();
		let [output] = installable.as_slice() else {
			return Err(format!(
				"--feed needs one output that calcite-update installs ({}), found {}",
				INSTALLABLE.join(", "),
				installable.len()
			));
		};
		let [tree] = output.paths.as_slice() else {
			return Err(format!(
				"the `{}` output has {} paths; --feed needs one",
				output.format,
				output.paths.len()
			));
		};
		let dir = staging.dir("feed")?;
		let target = publish::build_target(tree, &dir, &self.name, &self.version, &target_id(), &[])
			.map_err(|error| error.to_string())?;
		let full = dir.join(&target.full.name);
		let mut builder = FeedBuilder::new(
			&self.product,
			&self.channel,
			&self.version,
			self.sequence,
			publish::now(),
		)
		.notes(&self.notes);
		if let Some(minimum) = &self.minimum_version {
			builder = builder.minimum_version(minimum);
		}
		let feed = builder.target(target_id(), target).build();
		feed.minimum().map_err(|error| error.to_string())?;
		let key = publish::signing_key(&self.seed).map_err(|error| error.to_string())?;
		let bytes = publish::sign_feed(&feed, &self.key_id, &key).map_err(|error| error.to_string())?;
		let update = dir.join("update.json");
		fs::write(&update, bytes).map_err(|error| format!("{}: {error}", update.display()))?;
		Ok(Output {
			format: "update".to_string(),
			paths: vec![update, full],
		})
	}
}

#[cfg(test)]
mod tests {
	use std::{collections::BTreeMap, path::PathBuf};

	use calcite_update::{Envelope, verifying_key};
	use serde_json::{Value, json};

	use super::*;

	fn temp(name: &str) -> PathBuf {
		let dir = std::env::temp_dir().join(format!("calcite-feed-{}-{name}", std::process::id()));
		let _ = fs::remove_dir_all(&dir);
		fs::create_dir_all(&dir).unwrap();
		dir
	}

	fn settings(update: Option<Value>) -> Settings {
		let mut value = json!({ "identifier": "dev.example.app" });
		if let Some(update) = update {
			value["update"] = update;
		}
		serde_json::from_value(value).unwrap()
	}

	fn update_table() -> Value {
		json!({
			"base-url": "https://example.dev/app",
			"key-id": "2026",
			"minimum-version": "0.1.0",
		})
	}

	fn env(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
		let map: BTreeMap<String, String> = vars
			.iter()
			.map(|(key, value)| (key.to_string(), value.to_string()))
			.collect();
		move |key| map.get(key).cloned()
	}

	#[test]
	fn a_feed_verifies_with_the_key_that_signed_it() {
		let dir = temp("round-trip");
		let (seed, public_key) = publish::generate_key().unwrap();
		let app = dir.join("bundle/Preview.app/Contents/MacOS");
		fs::create_dir_all(&app).unwrap();
		fs::write(app.join("preview"), b"binary").unwrap();
		let plan = Plan::resolve(
			&FeedOptions {
				sequence: Some(7),
				notes: None,
			},
			&settings(Some(update_table())),
			"Cargo.toml",
			"dev.example.app",
			"preview",
			"0.1.0",
			env(&[("CALCITE_UPDATE_KEY", seed.as_str())]),
		)
		.unwrap();

		let staging = Staging::create(&dir.join("bundle")).unwrap();
		let staged = plan
			.write(
				&staging,
				&[Output {
					format: "app".to_string(),
					paths: vec![dir.join("bundle/Preview.app")],
				}],
			)
			.unwrap();
		let out = dir.join("release");
		staging.commit(&out, vec![staged]).unwrap();

		let envelope: Envelope = serde_json::from_slice(&fs::read(out.join("update.json")).unwrap()).unwrap();
		let keys = BTreeMap::from([("2026".to_string(), verifying_key(&public_key).unwrap())]);
		let feed = envelope.open(&keys).unwrap();
		assert_eq!(feed.product, "dev.example.app");
		assert_eq!(feed.channel, "stable");
		assert_eq!(feed.sequence, 7);
		assert_eq!(feed.minimum_version.as_deref(), Some("0.1.0"));
		let full = format!("preview-0.1.0-{}.full.zst", target_id());
		assert_eq!(feed.targets[&target_id()].full.name, full);
		assert!(out.join(&full).exists());

		let (_, other_key) = publish::generate_key().unwrap();
		let other = BTreeMap::from([("2026".to_string(), verifying_key(&other_key).unwrap())]);
		assert!(envelope.open(&other).is_err());
	}

	#[test]
	fn a_missing_feed_setting_is_named_in_the_error() {
		let (seed, _) = publish::generate_key().unwrap();
		let sequenced = FeedOptions {
			sequence: Some(7),
			notes: None,
		};
		let cases = [
			(
				"the sequence",
				FeedOptions::default(),
				settings(Some(update_table())),
				vec![("CALCITE_UPDATE_KEY", seed.as_str())],
				"--sequence",
			),
			(
				"the key",
				sequenced.clone(),
				settings(Some(update_table())),
				vec![],
				"CALCITE_UPDATE_KEY",
			),
			(
				"the update table",
				sequenced,
				settings(None),
				vec![("CALCITE_UPDATE_KEY", seed.as_str())],
				UPDATE_TABLE,
			),
		];
		for (case, request, config, vars, named) in cases {
			let error = Plan::resolve(
				&request,
				&config,
				"Cargo.toml",
				"dev.example.app",
				"preview",
				"0.1.0",
				env(&vars),
			)
			.err()
			.unwrap_or_else(|| panic!("{case} is missing, so resolve must fail"));
			assert!(error.contains(named), "{case}: {error}");
		}
	}
}
