// SPDX-License-Identifier: MPL-2.0

//! `--feed`: a signed calcite-update feed, `update.json`, beside the bundle that calcite-update
//! installs. The tree hashing, the packages, and the signature come from the `publish` feature
//! of `calcite-update`.

use std::{fs, path::PathBuf};

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

/// The package that `--feed` describes, from the build and the manifest.
pub struct Release<'a> {
	/// The product identifier, for example `dev.example.client`.
	pub identifier: &'a str,
	/// Prefixes the package file names.
	pub name: &'a str,
	pub version: &'a str,
	/// The formats the build packages, or `None` for the platform defaults.
	pub formats: Option<&'a [String]>,
}

/// The kind of the installable output: a directory (`app`) or one file (`appimage`, `exe`).
/// A `--previous` path must have the same kind, because its delta is a diff of that tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shape {
	Directory,
	File,
}

impl Shape {
	/// The shape of the one installable output in `formats`. With no formats, the platform
	/// default decides. `None` when no single installable output is named; `write` reports that
	/// after the build.
	fn of(formats: Option<&[String]>) -> Option<Self> {
		let installable: Vec<&str> = match formats {
			Some(formats) => formats
				.iter()
				.map(String::as_str)
				.filter(|format| INSTALLABLE.contains(format))
				.collect(),
			None if cfg!(target_os = "macos") => vec!["app"],
			None if cfg!(target_os = "linux") => vec!["appimage"],
			None => Vec::new(),
		};
		match installable.as_slice() {
			["app"] => Some(Self::Directory),
			["appimage" | "exe"] => Some(Self::File),
			_ => None,
		}
	}

	fn noun(self) -> &'static str {
		match self {
			Self::Directory => "a directory",
			Self::File => "a file",
		}
	}
}

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
	/// The earlier releases to build deltas from.
	previous: Vec<(String, PathBuf)>,
}

impl Plan {
	/// Reads the update table, the sequence, the notes, the `--previous` releases, and the signing
	/// seed. `env` reads environment variables; tests pass a closure.
	pub fn resolve(
		request: &FeedOptions,
		settings: &Settings,
		manifest: &str,
		release: &Release,
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
		check_previous(&request.previous, release.version, Shape::of(release.formats))?;
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
			product: release.identifier.to_string(),
			channel: update.channel,
			name: release.name.to_string(),
			version: release.version.to_string(),
			sequence,
			notes,
			minimum_version: update.minimum_version,
			key_id,
			seed: seed.trim().to_string(),
			previous: request.previous.clone(),
		})
	}

	/// Packages the installable output as the calcite-update target, signs the feed, and stages
	/// `update.json` with the packages and deltas. The result joins the bundles in one commit.
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
		let target = publish::build_target(
			tree,
			&dir,
			&self.name,
			&self.version,
			&target_id(),
			&self.previous,
		)
		.map_err(|error| error.to_string())?;
		let full = dir.join(&target.full.name);
		let deltas: Vec<PathBuf> = target
			.deltas
			.iter()
			.map(|delta| dir.join(&delta.asset.name))
			.collect();
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
		let mut paths = vec![update, full];
		paths.extend(deltas);
		Ok(Output {
			format: "update".to_string(),
			paths,
		})
	}
}

/// Checks each `--previous VERSION=PATH` before the build starts: the version parses, is lower
/// than `version`, and is given once; the path exists and has the kind of the installable output.
fn check_previous(previous: &[(String, PathBuf)], version: &str, shape: Option<Shape>) -> Result<()> {
	if previous.is_empty() {
		return Ok(());
	}
	let current = version
		.parse::<Version>()
		.map_err(|error| format!("the package version: {error}"))?;
	let mut seen: Vec<Version> = Vec::new();
	for (from, path) in previous {
		let flag = format!("--previous {from}={}", path.display());
		let parsed = from
			.parse::<Version>()
			.map_err(|error| format!("{flag}: {error}"))?;
		if parsed >= current {
			return Err(format!("{flag}: VERSION must be lower than {version}"));
		}
		if seen.contains(&parsed) {
			return Err(format!("{flag}: VERSION {from} is given more than once"));
		}
		seen.push(parsed);
		let metadata = fs::metadata(path).map_err(|error| format!("{flag}: {error}"))?;
		let actual = if metadata.is_dir() {
			Shape::Directory
		} else {
			Shape::File
		};
		if let Some(expected) = shape.filter(|expected| *expected != actual) {
			return Err(format!(
				"{flag}: the path is {}, but the new output is {}",
				actual.noun(),
				expected.noun()
			));
		}
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use std::{
		collections::BTreeMap,
		path::{Path, PathBuf},
	};

	use calcite_update::{Envelope, Kind, verifying_key};
	use serde_json::{Value, json};

	use super::*;

	fn temp(name: &str) -> PathBuf {
		let dir = std::env::temp_dir().join(format!("calcite-feed-{}-{name}", std::process::id()));
		let _ = fs::remove_dir_all(&dir);
		fs::create_dir_all(&dir).unwrap();
		dir
	}

	/// An `.app` tree with two files. Only the binary differs between releases.
	fn app_tree(root: &Path, binary: &[u8]) -> PathBuf {
		let app = root.join("Preview.app");
		fs::create_dir_all(app.join("Contents/MacOS")).unwrap();
		fs::create_dir_all(app.join("Contents/Resources")).unwrap();
		fs::write(app.join("Contents/MacOS/preview"), binary).unwrap();
		fs::write(app.join("Contents/Resources/notes.txt"), b"unchanged").unwrap();
		app
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
				previous: Vec::new(),
			},
			&settings(Some(update_table())),
			"Cargo.toml",
			&Release {
				identifier: "dev.example.app",
				name: "preview",
				version: "0.1.0",
				formats: None,
			},
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
			previous: Vec::new(),
		};
		let release = Release {
			identifier: "dev.example.app",
			name: "preview",
			version: "0.1.0",
			formats: None,
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
			let error = Plan::resolve(&request, &config, "Cargo.toml", &release, env(&vars))
				.err()
				.unwrap_or_else(|| panic!("{case} is missing, so resolve must fail"));
			assert!(error.contains(named), "{case}: {error}");
		}
	}

	#[test]
	fn a_delta_from_an_earlier_release_applies_to_its_tree() {
		let dir = temp("delta");
		let (seed, public_key) = publish::generate_key().unwrap();
		let old = app_tree(&dir.join("old"), b"old binary");
		let new = app_tree(&dir.join("new"), b"new binary");
		let formats = ["app".to_string()];
		let plan = Plan::resolve(
			&FeedOptions {
				sequence: Some(8),
				notes: None,
				previous: vec![("0.1.0".to_string(), old.clone())],
			},
			&settings(Some(update_table())),
			"Cargo.toml",
			&Release {
				identifier: "dev.example.app",
				name: "preview",
				version: "0.2.0",
				formats: Some(&formats),
			},
			env(&[("CALCITE_UPDATE_KEY", seed.as_str())]),
		)
		.unwrap();

		let staging = Staging::create(&dir.join("bundle")).unwrap();
		let staged = plan
			.write(
				&staging,
				&[Output {
					format: "app".to_string(),
					paths: vec![new.clone()],
				}],
			)
			.unwrap();
		let out = dir.join("release");
		staging.commit(&out, vec![staged]).unwrap();

		let envelope: Envelope = serde_json::from_slice(&fs::read(out.join("update.json")).unwrap()).unwrap();
		let keys = BTreeMap::from([("2026".to_string(), verifying_key(&public_key).unwrap())]);
		let feed = envelope.open(&keys).unwrap();
		let target = &feed.targets[&target_id()];
		let [delta] = target.deltas.as_slice() else {
			panic!("expected one delta, found {}", target.deltas.len());
		};
		assert_eq!(delta.from, "0.1.0");
		assert_eq!(
			delta.asset.name,
			format!("preview-0.1.0-0.2.0-{}.delta", target_id())
		);
		let package = out.join(&delta.asset.name);
		assert_eq!(delta.asset.size, fs::metadata(&package).unwrap().len());
		assert_eq!(delta.asset.sha256.len(), 64);

		let installed = dir.join("installed");
		calcite_update::stage::stage_delta(&package, &target.entries, &old, &installed).unwrap();
		for entry in target
			.entries
			.iter()
			.filter(|entry| matches!(entry.kind, Kind::File { .. }))
		{
			assert_eq!(
				fs::read(installed.join(&entry.path)).unwrap(),
				fs::read(new.join(&entry.path)).unwrap(),
				"{}",
				entry.path
			);
		}
	}

	#[test]
	fn a_previous_release_is_checked_before_the_build() {
		let dir = temp("previous-checks");
		let (seed, _) = publish::generate_key().unwrap();
		let tree = app_tree(&dir.join("old"), b"old binary");
		let file = dir.join("old-file");
		fs::write(&file, b"file").unwrap();
		let formats = ["app".to_string()];
		let release = Release {
			identifier: "dev.example.app",
			name: "preview",
			version: "0.2.0",
			formats: Some(&formats),
		};
		let cases: Vec<(&str, Vec<(&str, PathBuf)>)> = vec![
			("a missing path", vec![("0.1.0", dir.join("missing"))]),
			("a file for a directory output", vec![("0.1.0", file)]),
			("a version that is not lower", vec![("0.2.0", tree.clone())]),
			(
				"a repeated version",
				vec![("0.1.0", tree.clone()), ("0.1.0", tree.clone())],
			),
		];
		for (case, previous) in cases {
			let version = previous[0].0;
			let request = FeedOptions {
				sequence: Some(8),
				notes: None,
				previous: previous
					.into_iter()
					.map(|(from, path)| (from.to_string(), path))
					.collect(),
			};
			let error = Plan::resolve(
				&request,
				&settings(Some(update_table())),
				"Cargo.toml",
				&release,
				env(&[("CALCITE_UPDATE_KEY", seed.as_str())]),
			)
			.err()
			.unwrap_or_else(|| panic!("{case} must fail"));
			assert!(
				error.contains("--previous") && error.contains(version),
				"{case}: {error}"
			);
		}
	}
}
