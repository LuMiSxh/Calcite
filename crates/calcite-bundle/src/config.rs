// SPDX-License-Identifier: MPL-2.0

//! The `[package.metadata.calcite.bundle]` table, parsed into typed settings.

use std::path::PathBuf;

use serde::Deserialize;
use serde_json::{Map, Value};

/// The manifest table that holds the bundle settings.
const TABLE: &str = "[package.metadata.calcite.bundle]";

/// The Calcite-owned keys of `[package.metadata.calcite.bundle]`.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Settings {
	/// Product name. Default: the package name.
	pub name: Option<String>,
	/// Reverse-DNS identifier. Required.
	pub identifier: Option<String>,
	/// Square PNG, at least 512 px. Relative to the manifest directory.
	pub icon: Option<PathBuf>,
	/// Globs of files copied into the package. Relative to the manifest directory.
	pub resources: Option<Vec<String>>,
	/// Default formats when the command line names none.
	pub formats: Option<Vec<String>>,
	pub category: Option<String>,
	pub copyright: Option<String>,
	pub publisher: Option<String>,
	pub minimum_macos: Option<String>,
	pub macos: Option<Value>,
	pub windows: Option<Value>,
	/// Keys merged verbatim into the cargo-packager configuration.
	pub packager: Option<Map<String, Value>>,
}

/// `[package.metadata.calcite.bundle.macos]`: signing for the `.app` and DMG.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct MacosSettings {
	/// Developer ID identity. Setting it turns on signing and notarization.
	pub signing_identity: Option<String>,
	/// Path to an entitlements plist, relative to the manifest directory.
	pub entitlements: Option<PathBuf>,
}

/// `[package.metadata.calcite.bundle.windows]`: Authenticode signing for the installers and
/// the standalone exe.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct WindowsSettings {
	/// SHA-1 thumbprint of a certificate in the Windows store.
	pub certificate_thumbprint: Option<String>,
	/// RFC 3161 timestamp server. Required when signing.
	pub timestamp_url: Option<String>,
}

impl Settings {
	/// Parses the settings table of one package. `manifest` names the file in errors.
	pub fn from_metadata(value: &Value, manifest: &str) -> Result<Self, String> {
		let settings = value
			.get("calcite")
			.and_then(|calcite| calcite.get("bundle"))
			.ok_or_else(|| format!("{manifest}: add a {TABLE} table with at least `identifier`"))?;
		from_table(settings, TABLE, manifest)
	}

	/// The `identifier`, which every bundle needs.
	pub fn identifier(&self, manifest: &str) -> Result<&str, String> {
		self.identifier
			.as_deref()
			.ok_or_else(|| format!("{manifest}: {TABLE} needs `identifier`"))
	}
}

impl MacosSettings {
	pub fn from_settings(settings: &Settings, manifest: &str) -> Result<Self, String> {
		match &settings.macos {
			Some(value) => from_table(value, "[package.metadata.calcite.bundle.macos]", manifest),
			None => Ok(Self::default()),
		}
	}
}

impl WindowsSettings {
	pub fn from_settings(settings: &Settings, manifest: &str) -> Result<Self, String> {
		match &settings.windows {
			Some(value) => from_table(value, "[package.metadata.calcite.bundle.windows]", manifest),
			None => Ok(Self::default()),
		}
	}
}

/// Deserializes one table and names the table and manifest in the error.
fn from_table<T: serde::de::DeserializeOwned>(
	value: &Value,
	table: &str,
	manifest: &str,
) -> Result<T, String> {
	serde_json::from_value(value.clone()).map_err(|error| format!("{manifest}: {table}: {error}"))
}

#[cfg(test)]
mod tests {
	use serde_json::json;

	use super::*;

	#[test]
	fn an_unknown_key_is_rejected_with_its_name_and_manifest() {
		let metadata = json!({ "calcite": { "bundle": {
			"identifier": "dev.example.app",
			"catagory": "Game",
		} } });
		let error = Settings::from_metadata(&metadata, "/app/Cargo.toml").unwrap_err();
		assert!(error.contains("/app/Cargo.toml"), "{error}");
		assert!(error.contains("catagory"), "{error}");
	}

	#[test]
	fn an_unknown_key_in_a_signing_table_names_the_table() {
		let metadata = json!({ "calcite": { "bundle": {
			"identifier": "dev.example.app",
			"macos": { "signing-identty": "x" },
		} } });
		let settings = Settings::from_metadata(&metadata, "/app/Cargo.toml").unwrap();
		let error = MacosSettings::from_settings(&settings, "/app/Cargo.toml").unwrap_err();
		assert!(
			error.contains("[package.metadata.calcite.bundle.macos]"),
			"{error}"
		);
		assert!(error.contains("signing-identty"), "{error}");
	}
}
