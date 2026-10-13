// SPDX-License-Identifier: MPL-2.0

//! `cargo calcite bundle`: builds the app and packages it through cargo-packager.

use std::{
	fs,
	path::{Path, PathBuf},
	process::Command,
};

use serde_json::{Map, Value, json};

/// Pixel sizes generated from the source icon. The packager turns them into `.icns` on
/// macOS, `.ico` on Windows, and hicolor PNGs on Linux.
const ICON_SIZES: [u32; 6] = [32, 64, 128, 256, 512, 1024];

/// What to package and how.
#[derive(Clone, Debug, Default)]
pub struct Options {
	/// The package to bundle. Default: the package in the current directory.
	pub package: Option<String>,
	/// The binary to bundle. Default: the package's only binary, or the one named like it.
	pub bin: Option<String>,
	/// Formats instead of the configured or platform default ones.
	pub formats: Option<Vec<String>>,
	/// Builds and packages the debug profile, for a quick check of the bundle layout.
	pub debug: bool,
	/// Skips `cargo build`; the binary must already exist.
	pub no_build: bool,
}

/// One package that was written.
#[derive(Clone, Debug)]
pub struct Output {
	pub format: String,
	pub paths: Vec<PathBuf>,
}

pub type Result<T> = std::result::Result<T, String>;

/// Builds the app and writes its packages to `target/calcite-bundle/<profile>`.
pub fn bundle(options: &Options) -> Result<Vec<Output>> {
	let metadata = cargo_metadata()?;
	let package = select_package(&metadata, options.package.as_deref())?;
	let manifest_dir = PathBuf::from(
		package["manifest_path"]
			.as_str()
			.ok_or("cargo metadata: package has no manifest_path")?,
	)
	.parent()
	.ok_or("cargo metadata: manifest_path has no parent")?
	.to_path_buf();
	let target_dir = PathBuf::from(
		metadata["target_directory"]
			.as_str()
			.ok_or("cargo metadata: no target_directory")?,
	);
	let settings = package["metadata"]["calcite"]["bundle"]
		.as_object()
		.cloned()
		.ok_or_else(|| {
			format!(
				"{}: add a [package.metadata.calcite.bundle] table with at least `identifier`",
				package["name"].as_str().unwrap_or("package")
			)
		})?;
	let bin = select_bin(package, options.bin.as_deref())?;
	let profile = if options.debug { "debug" } else { "release" };

	if !options.no_build {
		build(package["name"].as_str().unwrap_or_default(), &bin, options.debug)?;
	}

	let out_dir = target_dir.join("calcite-bundle").join(profile);
	fs::create_dir_all(&out_dir).map_err(|error| format!("{}: {error}", out_dir.display()))?;
	let icons = match settings.get("icon").and_then(Value::as_str) {
		Some(icon) => generate_icons(&manifest_dir.join(icon), &out_dir.join("icons"))?,
		None => Vec::new(),
	};

	let config = packager_config(PackagerInput {
		package,
		settings: &settings,
		bin: &bin,
		binaries_dir: &target_dir.join(profile),
		out_dir: &out_dir,
		manifest_dir: &manifest_dir,
		icons: &icons,
		formats: options.formats.as_deref(),
	})?;
	let config: cargo_packager::Config =
		serde_json::from_value(config).map_err(|error| format!("bundle settings: {error}"))?;
	let outputs = cargo_packager::package(&config).map_err(|error| error.to_string())?;
	Ok(outputs
		.into_iter()
		.map(|output| Output {
			format: format!("{:?}", output.format).to_lowercase(),
			paths: output.paths,
		})
		.collect())
}

fn cargo() -> Command {
	Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
}

fn cargo_metadata() -> Result<Value> {
	let output = cargo()
		.args(["metadata", "--format-version", "1", "--no-deps"])
		.output()
		.map_err(|error| format!("cargo metadata: {error}"))?;
	if !output.status.success() {
		return Err(format!(
			"cargo metadata failed:\n{}",
			String::from_utf8_lossy(&output.stderr)
		));
	}
	serde_json::from_slice(&output.stdout).map_err(|error| format!("cargo metadata: {error}"))
}

fn select_package<'a>(metadata: &'a Value, name: Option<&str>) -> Result<&'a Value> {
	let packages = metadata["packages"]
		.as_array()
		.ok_or("cargo metadata: no packages")?;
	if let Some(name) = name {
		return packages
			.iter()
			.find(|package| package["name"] == name)
			.ok_or_else(|| format!("no package named `{name}` in this workspace"));
	}
	// Like cargo: the package whose manifest is in the current directory.
	let here = std::env::current_dir()
		.map_err(|error| error.to_string())?
		.join("Cargo.toml");
	packages
		.iter()
		.find(|package| package["manifest_path"].as_str().map(Path::new) == Some(here.as_path()))
		.or_else(|| (packages.len() == 1).then(|| &packages[0]))
		.ok_or_else(|| "several packages here: pass --package NAME".to_string())
}

fn select_bin(package: &Value, name: Option<&str>) -> Result<String> {
	let bins: Vec<&str> = package["targets"]
		.as_array()
		.into_iter()
		.flatten()
		.filter(|target| {
			target["kind"]
				.as_array()
				.is_some_and(|kinds| kinds.iter().any(|kind| kind == "bin"))
		})
		.filter_map(|target| target["name"].as_str())
		.collect();
	let package_name = package["name"].as_str().unwrap_or_default();
	match name {
		Some(name) if bins.contains(&name) => Ok(name.to_string()),
		Some(name) => Err(format!(
			"`{package_name}` has no binary `{name}`; it has: {}",
			bins.join(", ")
		)),
		None if bins.len() == 1 => Ok(bins[0].to_string()),
		None if bins.contains(&package_name) => Ok(package_name.to_string()),
		None => Err(format!(
			"`{package_name}` has several binaries; pass --bin with one of: {}",
			bins.join(", ")
		)),
	}
}

fn build(package: &str, bin: &str, debug: bool) -> Result<()> {
	let mut command = cargo();
	command.args(["build", "--package", package, "--bin", bin]);
	if !debug {
		command.arg("--release");
	}
	let status = command
		.status()
		.map_err(|error| format!("cargo build: {error}"))?;
	if status.success() {
		Ok(())
	} else {
		Err(format!("cargo build failed ({status})"))
	}
}

/// Resizes the source icon to every size the packager needs.
fn generate_icons(source: &Path, directory: &Path) -> Result<Vec<PathBuf>> {
	let image = image::open(source).map_err(|error| format!("icon {}: {error}", source.display()))?;
	if image.width() != image.height() {
		return Err(format!(
			"icon {}: must be square, is {}×{}",
			source.display(),
			image.width(),
			image.height()
		));
	}
	if image.width() < 512 {
		return Err(format!(
			"icon {}: must be at least 512 px, is {} px",
			source.display(),
			image.width()
		));
	}
	fs::create_dir_all(directory).map_err(|error| format!("{}: {error}", directory.display()))?;
	ICON_SIZES
		.iter()
		.filter(|&&size| size <= image.width())
		.map(|&size| {
			let path = directory.join(format!("{size}x{size}.png"));
			image
				.resize_exact(size, size, image::imageops::FilterType::Lanczos3)
				.save(&path)
				.map_err(|error| format!("{}: {error}", path.display()))?;
			Ok(path)
		})
		.collect()
}

struct PackagerInput<'a> {
	package: &'a Value,
	settings: &'a Map<String, Value>,
	bin: &'a str,
	binaries_dir: &'a Path,
	out_dir: &'a Path,
	manifest_dir: &'a Path,
	icons: &'a [PathBuf],
	formats: Option<&'a [String]>,
}

/// The cargo-packager configuration as JSON, in its own camelCase schema.
fn packager_config(input: PackagerInput) -> Result<Value> {
	let PackagerInput {
		package,
		settings,
		bin,
		binaries_dir,
		out_dir,
		manifest_dir,
		icons,
		formats,
	} = input;
	let text = |key: &str| settings.get(key).and_then(Value::as_str).map(str::to_string);
	let identifier =
		text("identifier").ok_or("[package.metadata.calcite.bundle]: `identifier` is required")?;
	let product_name =
		text("name").unwrap_or_else(|| package["name"].as_str().unwrap_or_default().to_string());

	let mut config = json!({
		"productName": product_name,
		"version": package["version"],
		"identifier": identifier,
		"binaries": [{ "path": bin, "main": true }],
		"binariesDir": binaries_dir,
		"outDir": out_dir,
		"icons": icons,
	});
	let object = config.as_object_mut().expect("the literal is an object");
	let mut set = |key: &str, value: Value| {
		if !value.is_null() {
			object.insert(key.to_string(), value);
		}
	};
	set("description", package["description"].clone());
	set("homepage", package["homepage"].clone());
	set("authors", package["authors"].clone());
	set(
		"publisher",
		settings.get("publisher").cloned().unwrap_or(Value::Null),
	);
	set(
		"copyright",
		settings.get("copyright").cloned().unwrap_or(Value::Null),
	);
	set(
		"category",
		settings.get("category").cloned().unwrap_or(Value::Null),
	);
	if let Some(file) = package["license_file"].as_str() {
		set("licenseFile", json!(manifest_dir.join(file)));
	}
	if let Some(resources) = settings.get("resources").and_then(Value::as_array) {
		// Globs resolve against the app's manifest directory, not the shell's.
		let resources: Vec<Value> = resources
			.iter()
			.filter_map(Value::as_str)
			.map(|glob| json!(manifest_dir.join(glob).to_string_lossy()))
			.collect();
		set("resources", json!(resources));
	}
	let formats = formats
		.map(|formats| json!(formats))
		.or_else(|| settings.get("formats").cloned())
		.unwrap_or(Value::Null);
	set("formats", formats);
	if let Some(version) = text("minimum-macos") {
		set("macos", json!({ "minimumSystemVersion": version }));
	}
	Ok(config)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn the_settings_map_onto_a_valid_packager_config() {
		let package = json!({
			"name": "gallery-app",
			"version": "0.1.0",
			"description": "A demo",
			"authors": ["Example"],
			"homepage": null,
			"license_file": null,
		});
		let settings = json!({
			"identifier": "dev.example.gallery",
			"name": "Calcite Gallery",
			"resources": ["assets/*.svg"],
			"category": "DeveloperTool",
			"minimum-macos": "13.0",
		});
		let config = packager_config(PackagerInput {
			package: &package,
			settings: settings.as_object().unwrap(),
			bin: "gallery",
			binaries_dir: Path::new("/target/release"),
			out_dir: Path::new("/target/calcite-bundle/release"),
			manifest_dir: Path::new("/app"),
			icons: &[PathBuf::from("/icons/512x512.png")],
			formats: Some(&["deb".to_string()]),
		})
		.unwrap();
		let config: cargo_packager::Config = serde_json::from_value(config).unwrap();
		assert_eq!(config.product_name, "Calcite Gallery");
		assert_eq!(config.identifier.as_deref(), Some("dev.example.gallery"));
	}
}
