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

/// The format that copies the bare Windows executable instead of running the packager.
const STANDALONE_EXE: &str = "exe";

/// Names the host platform's single-file format: `exe` on Windows, `dmg` on macOS, and
/// `appimage` on Linux.
const STANDALONE: &str = "standalone";

/// The single file the app ships as on the host platform, without an installer.
fn standalone_format() -> &'static str {
	if cfg!(windows) {
		STANDALONE_EXE
	} else if cfg!(target_os = "macos") {
		"dmg"
	} else {
		"appimage"
	}
}

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
	// The icon set is shared by every format in one packager run, so the formats that need
	// the `.icns` margin run separately from the rest.
	let icon = settings.get("icon").and_then(Value::as_str);
	let mut requested = requested_formats(options.formats.clone(), &settings);
	let mut outputs = Vec::new();
	if let Some(formats) = requested.as_mut()
		&& formats.iter().any(|format| format == STANDALONE_EXE)
	{
		formats.retain(|format| format != STANDALONE_EXE);
		outputs.push(standalone_exe(StandaloneInput {
			product_name: &product_name(package, &settings),
			version: package["version"].as_str().unwrap_or_default(),
			settings: &settings,
			bin: &bin,
			binaries_dir: &target_dir.join(profile),
			out_dir: &out_dir,
		})?);
		if formats.is_empty() {
			return Ok(outputs);
		}
	}
	for (pad_for_macos, formats) in format_groups(requested) {
		let icons = match icon {
			Some(icon) => generate_icons(
				&manifest_dir.join(icon),
				&out_dir.join(if pad_for_macos { "icons-macos" } else { "icons" }),
				pad_for_macos,
			)?,
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
			formats: formats.as_deref(),
		})?;
		let config: cargo_packager::Config =
			serde_json::from_value(config).map_err(|error| format!("bundle settings: {error}"))?;
		let packaged = cargo_packager::package(&config).map_err(|error| error.to_string())?;
		outputs.extend(packaged.into_iter().map(|output| Output {
			format: format!("{:?}", output.format).to_lowercase(),
			paths: output.paths,
		}));
	}
	Ok(outputs)
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

/// Apple's macOS icon template draws the artwork at 824 of 1024 px, centered, inside a
/// transparent margin. Scales `icon` to that size on a transparent canvas of the same size.
fn add_macos_margin(icon: &image::RgbaImage) -> image::RgbaImage {
	let (width, height) = icon.dimensions();
	let inner_width = (width * 824 / 1024).max(1);
	let inner_height = (height * 824 / 1024).max(1);
	let scaled = image::imageops::resize(
		icon,
		inner_width,
		inner_height,
		image::imageops::FilterType::Lanczos3,
	);
	let mut canvas = image::RgbaImage::new(width, height);
	image::imageops::overlay(
		&mut canvas,
		&scaled,
		i64::from((width - inner_width) / 2),
		i64::from((height - inner_height) / 2),
	);
	canvas
}

/// Resizes the source icon to every size the packager needs. `pad_for_macos` adds Apple's
/// transparent margin first, for the `.icns`; the other platforms keep the full-bleed artwork.
fn generate_icons(source: &Path, directory: &Path, pad_for_macos: bool) -> Result<Vec<PathBuf>> {
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
	let image = if pad_for_macos {
		image::DynamicImage::ImageRgba8(add_macos_margin(&image.to_rgba8()))
	} else {
		image
	};
	fs::create_dir_all(directory).map_err(|error| format!("{}: {error}", directory.display()))?;
	ICON_SIZES
		.iter()
		.filter(|&&size| size <= image.width())
		.map(|&size| {
			// The `.icns` writer has no 1x type for 64 and 1024 px: they are the 32 and 512 px icons at 2x.
			let path = directory.join(match size {
				64 => "32x32@2x.png".to_string(),
				1024 => "512x512@2x.png".to_string(),
				_ => format!("{size}x{size}.png"),
			});
			image
				.resize_exact(size, size, image::imageops::FilterType::Lanczos3)
				.save(&path)
				.map_err(|error| format!("{}: {error}", path.display()))?;
			Ok(path)
		})
		.collect()
}

struct StandaloneInput<'a> {
	product_name: &'a str,
	version: &'a str,
	settings: &'a Map<String, Value>,
	bin: &'a str,
	binaries_dir: &'a Path,
	out_dir: &'a Path,
}

/// Copies the executable alone, named like the NSIS installer without `-setup`:
/// `<name>_<version>_<arch>.exe`. Its icon comes from [`crate::build::windows_icon`].
fn standalone_exe(input: StandaloneInput) -> Result<Output> {
	if !cfg!(windows) {
		return Err(format!(
			"the `{STANDALONE_EXE}` format needs a Windows host: it copies the Windows build"
		));
	}
	if input.settings.contains_key("resources") {
		return Err(format!(
			"the `{STANDALONE_EXE}` format is a single file and cannot carry `resources`; use `nsis` or `wix`"
		));
	}
	let source = input.binaries_dir.join(format!("{}.exe", input.bin));
	let path = input.out_dir.join(standalone_exe_name(
		input.product_name,
		input.version,
		std::env::consts::ARCH,
	));
	fs::copy(&source, &path).map_err(|error| format!("{}: {error}", source.display()))?;
	Ok(Output {
		format: STANDALONE_EXE.to_string(),
		paths: vec![path],
	})
}

fn standalone_exe_name(product_name: &str, version: &str, arch: &str) -> String {
	// The architecture names of the NSIS and MSI outputs.
	let arch = match arch {
		"x86_64" => "x64",
		"aarch64" => "arm64",
		other => other,
	};
	format!("{product_name}_{version}_{arch}.exe")
}

fn product_name(package: &Value, settings: &Map<String, Value>) -> String {
	settings
		.get("name")
		.and_then(Value::as_str)
		.unwrap_or_else(|| package["name"].as_str().unwrap_or_default())
		.to_string()
}

fn is_macos_format(format: &str) -> bool {
	matches!(format, "app" | "dmg")
}

/// The CLI's formats, else the manifest's `formats`, with `standalone` resolved for the host;
/// `None` leaves the choice to the packager.
fn requested_formats(cli: Option<Vec<String>>, settings: &Map<String, Value>) -> Option<Vec<String>> {
	let formats = cli.or_else(|| {
		settings.get("formats").and_then(Value::as_array).map(|formats| {
			formats
				.iter()
				.filter_map(Value::as_str)
				.map(str::to_string)
				.collect()
		})
	})?;
	let mut resolved: Vec<String> = Vec::with_capacity(formats.len());
	for format in formats {
		let format = if format == STANDALONE {
			standalone_format().to_string()
		} else {
			format
		};
		if !resolved.contains(&format) {
			resolved.push(format);
		}
	}
	Some(resolved)
}

/// Splits the requested formats into packager runs, each as `(pad_for_macos, formats)`.
/// `app` and `dmg` read the `.icns` and get the margin; the others get the full-bleed artwork.
/// With no formats named, the packager uses the host platform's defaults, which are the macOS
/// ones only on a Mac.
fn format_groups(requested: Option<Vec<String>>) -> Vec<(bool, Option<Vec<String>>)> {
	let Some(formats) = requested else {
		return vec![(cfg!(target_os = "macos"), None)];
	};
	let (macos, others): (Vec<String>, Vec<String>) =
		formats.into_iter().partition(|format| is_macos_format(format));
	let mut groups = Vec::new();
	if !macos.is_empty() {
		groups.push((true, Some(macos)));
	}
	if !others.is_empty() || groups.is_empty() {
		groups.push((false, Some(others)));
	}
	groups
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
	let product_name = product_name(package, settings);

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
	set("formats", formats.map_or(Value::Null, |formats| json!(formats)));
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
			"name": "preview-app",
			"version": "0.1.0",
			"description": "A demo",
			"authors": ["Example"],
			"homepage": null,
			"license_file": null,
		});
		let settings = json!({
			"identifier": "dev.example.preview",
			"name": "Calcite Developer Preview",
			"resources": ["assets/*.svg"],
			"category": "DeveloperTool",
			"minimum-macos": "13.0",
		});
		let config = packager_config(PackagerInput {
			package: &package,
			settings: settings.as_object().unwrap(),
			bin: "preview",
			binaries_dir: Path::new("/target/release"),
			out_dir: Path::new("/target/calcite-bundle/release"),
			manifest_dir: Path::new("/app"),
			icons: &[PathBuf::from("/icons/512x512.png")],
			formats: Some(&["deb".to_string()]),
		})
		.unwrap();
		let config: cargo_packager::Config = serde_json::from_value(config).unwrap();
		assert_eq!(config.product_name, "Calcite Developer Preview");
		assert_eq!(config.identifier.as_deref(), Some("dev.example.preview"));
	}

	#[test]
	fn the_standalone_exe_is_named_like_the_installer() {
		assert_eq!(
			standalone_exe_name("Calcite - preview", "0.2.1", "x86_64"),
			"Calcite - preview_0.2.1_x64.exe"
		);
		assert_eq!(
			standalone_exe_name("app", "1.0.0", "aarch64"),
			"app_1.0.0_arm64.exe"
		);
	}

	#[test]
	fn the_standalone_exe_is_split_from_the_packager_formats() {
		let settings = json!({ "formats": ["exe", "nsis"] });
		let requested = requested_formats(None, settings.as_object().unwrap());
		assert_eq!(requested, Some(vec!["exe".to_string(), "nsis".to_string()]));
		let cli = requested_formats(Some(vec!["deb".to_string()]), settings.as_object().unwrap());
		assert_eq!(cli, Some(vec!["deb".to_string()]));
		let standalone = requested_formats(
			Some(vec!["standalone".to_string(), standalone_format().to_string()]),
			&Map::new(),
		);
		assert_eq!(standalone, Some(vec![standalone_format().to_string()]));
	}

	#[test]
	fn macos_icons_keep_the_size_and_get_a_transparent_margin() {
		let icon = image::RgbaImage::from_pixel(512, 512, image::Rgba([200, 30, 30, 255]));
		let padded = add_macos_margin(&icon);
		assert_eq!(padded.dimensions(), (512, 512));
		assert_eq!(padded.get_pixel(0, 0)[3], 0);
		assert_eq!(padded.get_pixel(256, 256)[3], 255);
	}

	#[test]
	fn only_the_macos_formats_get_the_icon_margin() {
		let source = std::env::temp_dir().join(format!("calcite-icon-{}.png", std::process::id()));
		image::RgbaImage::from_pixel(512, 512, image::Rgba([200, 30, 30, 255]))
			.save(&source)
			.unwrap();
		for format in ["nsis", "deb", "appimage", "dmg", "app"] {
			let (pad_for_macos, _) = format_groups(Some(vec![format.to_string()])).remove(0);
			let directory =
				std::env::temp_dir().join(format!("calcite-icons-{}-{format}", std::process::id()));
			generate_icons(&source, &directory, pad_for_macos).unwrap();
			let icon = image::open(directory.join("512x512.png")).unwrap().to_rgba8();
			fs::remove_dir_all(&directory).ok();
			if pad_for_macos {
				assert_eq!(icon.get_pixel(0, 0)[3], 0, "{format}: corner must be transparent");
				assert_eq!(
					icon.get_pixel(256, 256)[3],
					255,
					"{format}: centre must be opaque"
				);
			} else {
				assert_eq!(icon.get_pixel(0, 0)[3], 255, "{format}: corner must stay opaque");
			}
		}
		fs::remove_file(&source).ok();
	}
}
