// SPDX-License-Identifier: MPL-2.0

//! The cargo-packager run: the configuration Calcite builds, and the [`Packager`] that runs it.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use crate::{config::Settings, package::Result, sign::MacosSigning};

/// One packaged file or directory, as cargo-packager reports it.
#[derive(Clone, Debug)]
pub struct Packaged {
	pub format: String,
	pub paths: Vec<PathBuf>,
}

/// Runs cargo-packager. Tests inject a fake that writes files and fails on demand.
pub trait Packager {
	fn package(&self, config: &cargo_packager::Config) -> Result<Vec<Packaged>>;
}

/// Runs the real cargo-packager.
pub struct CargoPackager;

impl Packager for CargoPackager {
	fn package(&self, config: &cargo_packager::Config) -> Result<Vec<Packaged>> {
		let packaged = cargo_packager::package(config).map_err(|error| error.to_string())?;
		Ok(packaged
			.into_iter()
			.map(|output| Packaged {
				format: format!("{:?}", output.format).to_lowercase(),
				paths: output.paths,
			})
			.collect())
	}
}

/// What one packager run builds, with the paths it needs.
pub struct Run<'a> {
	pub product_name: &'a str,
	pub version: &'a str,
	pub identifier: &'a str,
	/// The cargo metadata of the package: description, authors, homepage, and license file.
	pub package: &'a Value,
	pub settings: &'a Settings,
	pub macos: Option<&'a MacosSigning>,
	pub bin: &'a str,
	pub binaries_dir: &'a Path,
	/// The run's staging directory. cargo-packager writes into it.
	pub out_dir: &'a Path,
	pub manifest_dir: &'a Path,
	pub icons: &'a [PathBuf],
	pub formats: Option<&'a [String]>,
}

/// Builds the cargo-packager configuration: Calcite's keys, then the `packager` table merged
/// over them. The merge is deep, and the packager table wins on every leaf.
pub fn config(run: &Run) -> Result<cargo_packager::Config> {
	let mut json = base(run);
	if let Some(overrides) = &run.settings.packager {
		if let Some(key) = managed_key(overrides) {
			return Err(format!(
				"[package.metadata.calcite.bundle.packager]: `{key}` is set by Calcite: it owns the output directory, signing, and notarization"
			));
		}
		merge(&mut json, &Value::Object(overrides.clone()));
	}
	let mut config: cargo_packager::Config = serde_json::from_value(json)
		.map_err(|error| format!("[package.metadata.calcite.bundle]: packager settings: {error}"))?;
	if let Some(macos) = run.macos {
		config
			.macos
			.get_or_insert_with(Default::default)
			.notarization_credentials = Some(macos.notary.to_packager());
	}
	Ok(config)
}

fn base(run: &Run) -> Value {
	let settings = run.settings;
	let package = run.package;
	let mut config = json!({
		"productName": run.product_name,
		"version": run.version,
		"identifier": run.identifier,
		"binaries": [{ "path": run.bin, "main": true }],
		"binariesDir": run.binaries_dir,
		"outDir": run.out_dir,
	});
	let object = config.as_object_mut().expect("the literal is an object");
	for (key, value) in [
		("description", &package["description"]),
		("homepage", &package["homepage"]),
		("authors", &package["authors"]),
	] {
		if !value.is_null() {
			object.insert(key.to_string(), value.clone());
		}
	}
	insert(object, "publisher", settings.publisher.as_ref());
	insert(object, "copyright", settings.copyright.as_ref());
	insert(object, "category", settings.category.as_ref());
	if let Some(file) = package["license_file"].as_str() {
		object.insert("licenseFile".into(), json!(run.manifest_dir.join(file)));
	}
	if let Some(globs) = &settings.resources {
		// Globs resolve against the app's manifest directory, not the shell's.
		let resources: Vec<String> = globs
			.iter()
			.map(|glob| run.manifest_dir.join(glob).to_string_lossy().into_owned())
			.collect();
		object.insert("resources".into(), json!(resources));
	}
	if !run.icons.is_empty() {
		object.insert("icons".into(), json!(run.icons));
	}
	if let Some(formats) = run.formats {
		object.insert("formats".into(), json!(formats));
	}
	let mut macos = Map::new();
	if let Some(version) = &settings.minimum_macos {
		macos.insert("minimumSystemVersion".into(), json!(version));
	}
	if let Some(signing) = run.macos {
		macos.insert("signingIdentity".into(), json!(signing.identity));
		if let Some(entitlements) = &signing.entitlements {
			macos.insert("entitlements".into(), json!(entitlements));
		}
	}
	if !macos.is_empty() {
		object.insert("macos".into(), Value::Object(macos));
	}
	config
}

fn insert(object: &mut Map<String, Value>, key: &str, value: Option<&String>) {
	if let Some(value) = value {
		object.insert(key.to_string(), json!(value));
	}
}

/// Calcite owns the output directory, and macOS and Windows signing and notarization, so no
/// packager setting may name them. Returns the offending key as written, dotted for nested ones.
fn managed_key(overrides: &Map<String, Value>) -> Option<String> {
	const MANAGED: [(&str, &[&str]); 3] = [
		("", &["outdir"]),
		(
			"macos",
			&["signingidentity", "entitlements", "notarizationcredentials"],
		),
		(
			"windows",
			&[
				"certificatethumbprint",
				"signcommand",
				"timestampurl",
				"tsp",
				"digestalgorithm",
			],
		),
	];
	for (key, value) in overrides {
		let key_name = normalize(key);
		for (section, names) in MANAGED {
			if section.is_empty() && names.contains(&key_name.as_str()) {
				return Some(key.clone());
			}
			if section == key_name
				&& let Value::Object(inner) = value
				&& let Some(found) = inner.keys().find(|k| names.contains(&normalize(k).as_str()))
			{
				return Some(format!("{key}.{found}"));
			}
		}
	}
	None
}

fn normalize(key: &str) -> String {
	key.chars()
		.filter(|c| *c != '-' && *c != '_')
		.map(|c| c.to_ascii_lowercase())
		.collect()
}

fn merge(base: &mut Value, overlay: &Value) {
	match (base, overlay) {
		(Value::Object(base), Value::Object(overlay)) => {
			for (key, value) in overlay {
				merge(base.entry(key.clone()).or_insert(Value::Null), value);
			}
		}
		(base, overlay) => *base = overlay.clone(),
	}
}

#[cfg(test)]
mod tests {
	use serde_json::json;

	use super::*;

	fn settings(value: Value) -> Settings {
		serde_json::from_value(value).unwrap()
	}

	fn run<'a>(settings: &'a Settings, package: &'a Value, icons: &'a [PathBuf]) -> Run<'a> {
		Run {
			product_name: "Calcite Developer Preview",
			version: "0.1.0",
			identifier: "dev.example.preview",
			package,
			settings,
			macos: None,
			bin: "preview",
			binaries_dir: Path::new("/target/release"),
			out_dir: Path::new("/target/calcite-bundle/.staging/run-0"),
			manifest_dir: Path::new("/app"),
			icons,
			formats: None,
		}
	}

	#[test]
	fn the_calcite_keys_map_onto_the_packager_config() {
		let package = json!({ "name": "preview-app", "version": "0.1.0", "authors": ["Example"] });
		let settings = settings(json!({
			"identifier": "dev.example.preview",
			"name": "Calcite Developer Preview",
			"resources": ["assets/*.svg"],
			"category": "DeveloperTool",
			"minimum-macos": "13.0",
		}));
		let icons = [PathBuf::from("/icons/512x512.png")];
		let config = config(&run(&settings, &package, &icons)).unwrap();
		assert_eq!(config.product_name, "Calcite Developer Preview");
		assert_eq!(config.identifier.as_deref(), Some("dev.example.preview"));
		assert_eq!(
			config.macos.unwrap().minimum_system_version.as_deref(),
			Some("13.0")
		);
	}

	#[test]
	fn the_packager_table_reaches_the_generated_config() {
		let package = json!({ "name": "preview-app", "version": "0.1.0" });
		let settings = settings(json!({
			"identifier": "dev.example.preview",
			"packager": {
				"macos": { "frameworks": ["SDL2"] },
				"deb": { "depends": ["libgtk-3-0"] },
				"productName": "Preview Override",
			},
		}));
		let config = config(&run(&settings, &package, &[])).unwrap();
		assert_eq!(config.product_name, "Preview Override");
		assert_eq!(
			config.macos.unwrap().frameworks.unwrap(),
			vec!["SDL2".to_string()]
		);
		assert!(config.deb.unwrap().depends.is_some());
	}

	#[test]
	fn the_packager_table_cannot_move_the_output_directory() {
		let package = json!({ "name": "preview-app", "version": "0.1.0" });
		let settings = settings(json!({
			"identifier": "dev.example.preview",
			"packager": { "out-dir": "/elsewhere" },
		}));
		let error = config(&run(&settings, &package, &[])).unwrap_err();
		assert!(error.contains("out-dir"), "{error}");
	}

	#[test]
	fn the_packager_table_cannot_override_signing_or_notarization() {
		let package = json!({ "name": "preview-app", "version": "0.1.0" });
		for (table, expected) in [
			(
				json!({ "macos": { "signing-identity": "-" } }),
				"macos.signing-identity",
			),
			(
				json!({ "macos": { "notarizationCredentials": { "keychainProfile": "x" } } }),
				"macos.notarizationCredentials",
			),
			(
				json!({ "windows": { "certificate_thumbprint": "AB" } }),
				"windows.certificate_thumbprint",
			),
		] {
			let settings = settings(json!({ "identifier": "dev.example.preview", "packager": table }));
			let error = config(&run(&settings, &package, &[])).unwrap_err();
			assert!(error.contains(expected), "{error}");
		}
	}

	#[test]
	fn an_unknown_packager_key_is_named_in_the_error() {
		let package = json!({ "name": "preview-app", "version": "0.1.0" });
		let settings = settings(json!({
			"identifier": "dev.example.preview",
			"packager": { "formats": 3 },
		}));
		let error = config(&run(&settings, &package, &[])).unwrap_err();
		assert!(error.contains("packager settings"), "{error}");
	}
}
