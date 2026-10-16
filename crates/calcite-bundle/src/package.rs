// SPDX-License-Identifier: MPL-2.0

//! `cargo calcite bundle`: builds the app and packages it through cargo-packager.

use std::{
	fs,
	path::{Path, PathBuf},
	process::Command,
};

use serde_json::Value;

use crate::{
	config::Settings,
	icons,
	packager::{CargoPackager, Packager, Run},
	sign::{Commands, Signing, SystemCommands, notarize_dmg},
	stage::Staging,
};

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
	/// Also writes a signed calcite-update feed beside the bundles (`--feed`).
	pub feed: Option<FeedOptions>,
}

/// The calcite-update feed that `--feed` asks for.
#[derive(Clone, Debug, Default)]
pub struct FeedOptions {
	/// The release sequence. Required; it must rise with every feed that the apps have seen.
	pub sequence: Option<u64>,
	/// Release notes in Markdown, read from this file.
	pub notes: Option<PathBuf>,
}

/// One package that was written.
#[derive(Clone, Debug)]
pub struct Output {
	pub format: String,
	pub paths: Vec<PathBuf>,
}

pub type Result<T> = std::result::Result<T, String>;

/// Everything a bundle needs, resolved from cargo metadata and the manifest.
pub struct Plan {
	pub product_name: String,
	pub version: String,
	pub identifier: String,
	/// The cargo metadata of the package.
	pub package: Value,
	pub settings: Settings,
	pub signing: Signing,
	pub bin: String,
	pub binaries_dir: PathBuf,
	pub out_dir: PathBuf,
	/// `target/calcite-bundle`: the staging root goes here, beside the output directory.
	pub bundle_dir: PathBuf,
	pub manifest_dir: PathBuf,
	/// The formats after the `standalone` alias is resolved, or `None` for the platform defaults.
	pub formats: Option<Vec<String>>,
	/// The calcite-update feed that `--feed` writes beside the bundles.
	#[cfg(feature = "feed")]
	pub feed: Option<crate::feed::Plan>,
}

/// Builds the app and writes its packages to `target/calcite-bundle/<profile>`.
pub fn bundle(options: &Options) -> Result<Vec<Output>> {
	let metadata = cargo_metadata()?;
	let package = select_package(&metadata, options.package.as_deref())?;
	let name = package["name"]
		.as_str()
		.ok_or("cargo metadata: package has no name")?;
	let version = package["version"]
		.as_str()
		.ok_or("cargo metadata: package has no version")?;
	let manifest = package["manifest_path"]
		.as_str()
		.ok_or("cargo metadata: package has no manifest_path")?;
	let manifest_dir = Path::new(manifest)
		.parent()
		.ok_or("cargo metadata: manifest_path has no parent")?
		.to_path_buf();
	let target_dir = PathBuf::from(
		metadata["target_directory"]
			.as_str()
			.ok_or("cargo metadata: no target_directory")?,
	);
	let settings = Settings::from_metadata(&package["metadata"], manifest)?;
	let identifier = settings.identifier(manifest)?.to_string();
	let signing = crate::sign::resolve(&settings, &manifest_dir, manifest, |key| std::env::var(key).ok())?;
	for warning in signing.warnings() {
		eprintln!("{warning}");
	}
	let bin = select_bin(package, options.bin.as_deref())?;
	// The feed settings are checked before the build, so a missing key fails in seconds.
	#[cfg(feature = "feed")]
	let feed = match &options.feed {
		Some(request) => Some(crate::feed::Plan::resolve(
			request,
			&settings,
			manifest,
			&identifier,
			name,
			version,
			|key| std::env::var(key).ok(),
		)?),
		None => None,
	};
	#[cfg(not(feature = "feed"))]
	if options.feed.is_some() {
		return Err("--feed needs calcite-bundle built with the `feed` feature".into());
	}
	let profile = if options.debug { "debug" } else { "release" };

	if !options.no_build {
		build(name, &bin, options.debug)?;
	}

	let bundle_dir = target_dir.join("calcite-bundle");
	let plan = Plan {
		product_name: settings.name.clone().unwrap_or_else(|| name.to_string()),
		version: version.to_string(),
		identifier,
		package: package.clone(),
		signing,
		binaries_dir: target_dir.join(profile),
		out_dir: bundle_dir.join(profile),
		bundle_dir,
		manifest_dir,
		formats: requested_formats(options.formats.clone(), &settings),
		#[cfg(feature = "feed")]
		feed,
		settings,
		bin,
	};
	publish(&plan, &CargoPackager, &SystemCommands)
}

/// Runs every packager step into a staging root, then moves the outputs into `plan.out_dir`.
/// A failure removes the staging root and leaves `plan.out_dir` as it was.
pub fn publish(plan: &Plan, packager: &impl Packager, commands: &impl Commands) -> Result<Vec<Output>> {
	fs::create_dir_all(&plan.out_dir).map_err(|error| format!("{}: {error}", plan.out_dir.display()))?;
	let staging = Staging::create(&plan.bundle_dir)?;
	match produce(plan, &staging, packager, commands).and_then(|outputs| with_feed(plan, &staging, outputs)) {
		Ok(outputs) => staging.commit(&plan.out_dir, outputs),
		Err(error) => Err(match staging.discard() {
			Ok(()) => error,
			Err(cleanup) => format!("{error}\n{cleanup}"),
		}),
	}
}

/// Adds the calcite-update feed to the outputs when `--feed` asks for it. The feed is staged
/// with the bundles, so it moves into the output directory in the same commit.
#[cfg(feature = "feed")]
fn with_feed(plan: &Plan, staging: &Staging, mut outputs: Vec<Output>) -> Result<Vec<Output>> {
	if let Some(feed) = &plan.feed {
		outputs.push(feed.write(staging, &outputs)?);
	}
	Ok(outputs)
}

#[cfg(not(feature = "feed"))]
fn with_feed(_plan: &Plan, _staging: &Staging, outputs: Vec<Output>) -> Result<Vec<Output>> {
	Ok(outputs)
}

fn produce(
	plan: &Plan,
	staging: &Staging,
	packager: &impl Packager,
	commands: &impl Commands,
) -> Result<Vec<Output>> {
	let mut outputs = Vec::new();
	let mut formats = plan.formats.clone();
	let wants_standalone = formats
		.as_ref()
		.is_some_and(|formats| formats.iter().any(|format| format == STANDALONE_EXE));
	if wants_standalone {
		formats
			.as_mut()
			.unwrap()
			.retain(|format| format != STANDALONE_EXE);
	}
	let wants_windows = wants_standalone
		|| formats.as_ref().map_or(cfg!(windows), |formats| {
			formats.iter().any(|format| is_windows_format(format))
		});

	// Windows signing signs the binary once, before any installer packs it. The installers and
	// the standalone exe then carry the same signed file.
	let binaries_dir = match &plan.signing.windows {
		Some(authenticode) if wants_windows => {
			let shared = staging.dir("shared")?;
			let source = plan.binaries_dir.join(format!("{}.exe", plan.bin));
			let signed = shared.join(format!("{}.exe", plan.bin));
			fs::copy(&source, &signed).map_err(|error| format!("{}: {error}", source.display()))?;
			authenticode.sign(commands, &signed)?;
			shared
		}
		_ => plan.binaries_dir.clone(),
	};

	if wants_standalone {
		outputs.push(standalone_exe(plan, staging, &binaries_dir)?);
	}
	if formats.as_ref().is_some_and(Vec::is_empty) {
		return Ok(outputs);
	}

	for (index, (pad_for_macos, run_formats)) in format_groups(formats).into_iter().enumerate() {
		let run_dir = staging.dir(&format!("run-{index}"))?;
		let icons = match &plan.settings.icon {
			Some(icon) => icons::generate(
				&plan.manifest_dir.join(icon),
				&run_dir.join(if pad_for_macos { "icons-macos" } else { "icons" }),
				pad_for_macos,
			)?,
			None => Vec::new(),
		};
		let config = crate::packager::config(&Run {
			product_name: &plan.product_name,
			version: &plan.version,
			identifier: &plan.identifier,
			package: &plan.package,
			settings: &plan.settings,
			macos: plan.signing.macos.as_ref(),
			bin: &plan.bin,
			binaries_dir: &binaries_dir,
			out_dir: &run_dir,
			manifest_dir: &plan.manifest_dir,
			icons: &icons,
			formats: run_formats.as_deref(),
		})?;
		for packaged in packager.package(&config)? {
			for path in &packaged.paths {
				match (packaged.format.as_str(), &plan.signing) {
					(
						"dmg",
						crate::sign::Signing {
							macos: Some(macos), ..
						},
					) => {
						notarize_dmg(commands, path, &macos.notary)?;
					}
					(
						"nsis" | "wix",
						crate::sign::Signing {
							windows: Some(authenticode),
							..
						},
					) => {
						authenticode.sign(commands, path)?;
					}
					_ => {}
				}
			}
			outputs.push(Output {
				format: packaged.format,
				paths: packaged.paths,
			});
		}
	}
	Ok(outputs)
}

/// Copies the executable alone, named like the NSIS installer without `-setup`:
/// `<name>_<version>_<arch>.exe`. Its icon comes from [`crate::build::windows_icon`].
fn standalone_exe(plan: &Plan, staging: &Staging, binaries_dir: &Path) -> Result<Output> {
	if !cfg!(windows) {
		return Err(format!(
			"the `{STANDALONE_EXE}` format needs a Windows host: it copies the Windows build"
		));
	}
	if plan.settings.resources.is_some() {
		return Err(format!(
			"the `{STANDALONE_EXE}` format is a single file and cannot carry `resources`; use `nsis` or `wix`"
		));
	}
	let source = binaries_dir.join(format!("{}.exe", plan.bin));
	let path = staging.dir(STANDALONE_EXE)?.join(standalone_exe_name(
		&plan.product_name,
		&plan.version,
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

fn is_macos_format(format: &str) -> bool {
	matches!(format, "app" | "dmg")
}

fn is_windows_format(format: &str) -> bool {
	matches!(format, "nsis" | "wix" | STANDALONE_EXE)
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

/// The CLI's formats, else the manifest's `formats`, with `standalone` resolved for the host;
/// `None` leaves the choice to the packager.
fn requested_formats(cli: Option<Vec<String>>, settings: &Settings) -> Option<Vec<String>> {
	let formats = cli.or_else(|| settings.formats.clone())?;
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
	if !others.is_empty() {
		groups.push((false, Some(others)));
	}
	groups
}

#[cfg(test)]
mod tests {
	use std::cell::Cell;

	use serde_json::json;

	use super::*;
	use crate::{
		packager::{Packaged, config},
		sign::{Authenticode, Certificate, MacosSigning, NotaryCredentials},
	};

	/// Writes one file per run and fails the run at `fail_at`.
	struct FakePackager {
		calls: Cell<usize>,
		fail_at: usize,
	}

	impl Packager for FakePackager {
		fn package(&self, config: &cargo_packager::Config) -> Result<Vec<Packaged>> {
			let call = self.calls.get();
			self.calls.set(call + 1);
			if call == self.fail_at {
				return Err("packager run 2 failed".to_string());
			}
			let path = config.out_dir.join(format!("App-{call}.deb"));
			fs::write(&path, b"package").unwrap();
			Ok(vec![Packaged {
				format: "deb".to_string(),
				paths: vec![path],
			}])
		}
	}

	/// Records every tool call. Notarization answers with an accepted verdict.
	#[derive(Default)]
	struct Recorder {
		calls: std::cell::RefCell<Vec<(String, Vec<String>)>>,
	}

	impl Commands for Recorder {
		fn run(&self, program: &str, args: &[String]) -> Result<String> {
			self.calls.borrow_mut().push((program.to_string(), args.to_vec()));
			Ok(r#"{"status":"Accepted","id":"1"}"#.to_string())
		}
	}

	fn temp(name: &str) -> PathBuf {
		let dir = std::env::temp_dir().join(format!("calcite-package-{}-{name}", std::process::id()));
		let _ = fs::remove_dir_all(&dir);
		fs::create_dir_all(dir.join("release")).unwrap();
		dir
	}

	fn plan(bundle_dir: &Path, formats: Option<Vec<String>>, signing: Signing) -> Plan {
		let settings: Settings = serde_json::from_value(json!({
			"identifier": "dev.example.preview",
			"name": "Calcite Developer Preview",
		}))
		.unwrap();
		Plan {
			product_name: "Calcite Developer Preview".to_string(),
			version: "0.1.0".to_string(),
			identifier: "dev.example.preview".to_string(),
			package: json!({ "name": "preview-app", "version": "0.1.0" }),
			settings,
			signing,
			bin: "preview".to_string(),
			binaries_dir: bundle_dir.join("target-release"),
			out_dir: bundle_dir.join("release"),
			bundle_dir: bundle_dir.to_path_buf(),
			manifest_dir: bundle_dir.to_path_buf(),
			#[cfg(feature = "feed")]
			feed: None,
			formats,
		}
	}

	#[test]
	fn a_failed_second_run_leaves_the_output_directory_untouched() {
		let bundle = temp("failed-run");
		// `dmg` and `deb` are two runs: `publish` must not publish run 1 when run 2 fails.
		let plan = plan(
			&bundle,
			Some(vec!["dmg".into(), "deb".into()]),
			Signing::default(),
		);
		let packager = FakePackager {
			calls: Cell::new(0),
			fail_at: 1,
		};
		let error = publish(&plan, &packager, &Recorder::default()).unwrap_err();
		assert!(error.contains("run 2 failed"), "{error}");
		assert_eq!(fs::read_dir(&plan.out_dir).unwrap().count(), 0);
		assert!(!bundle.read_dir().unwrap().any(|entry| {
			entry
				.unwrap()
				.file_name()
				.to_string_lossy()
				.starts_with(".staging")
		}));
		fs::remove_dir_all(&bundle).unwrap();
	}

	#[test]
	fn a_successful_publish_moves_every_run_into_the_output_directory() {
		let bundle = temp("publish");
		let plan = plan(
			&bundle,
			Some(vec!["dmg".into(), "deb".into()]),
			Signing::default(),
		);
		let packager = FakePackager {
			calls: Cell::new(0),
			fail_at: usize::MAX,
		};
		let outputs = publish(&plan, &packager, &Recorder::default()).unwrap();
		assert_eq!(outputs.len(), 2);
		assert!(
			outputs
				.iter()
				.all(|output| output.paths[0].starts_with(&plan.out_dir))
		);
		assert!(outputs.iter().all(|output| output.paths[0].exists()));
		fs::remove_dir_all(&bundle).unwrap();
	}

	#[test]
	fn a_dmg_is_notarized_and_stapled_after_packaging() {
		struct DmgPackager;
		impl Packager for DmgPackager {
			fn package(&self, config: &cargo_packager::Config) -> Result<Vec<Packaged>> {
				let path = config.out_dir.join("App.dmg");
				fs::write(&path, b"dmg").unwrap();
				Ok(vec![Packaged {
					format: "dmg".to_string(),
					paths: vec![path],
				}])
			}
		}
		let bundle = temp("notarize");
		let signing = Signing {
			macos: Some(MacosSigning {
				identity: "Developer ID Application: Example".to_string(),
				entitlements: None,
				notary: NotaryCredentials::KeychainProfile("ci".to_string()),
			}),
			windows: None,
		};
		let plan = plan(&bundle, Some(vec!["dmg".into()]), signing);
		let recorder = Recorder::default();
		publish(&plan, &DmgPackager, &recorder).unwrap();
		let calls = recorder.calls.into_inner();
		assert_eq!(calls[0].0, "xcrun");
		assert_eq!(calls[0].1[1], "submit");
		// The DMG is stapled in its staging directory, then moved: the ticket travels with it.
		assert_eq!(calls[1].1[..3], ["stapler", "staple", "-v"]);
		assert!(calls[1].1[3].ends_with("App.dmg"));
		fs::remove_dir_all(&bundle).unwrap();
	}

	#[test]
	fn windows_signing_signs_the_binary_before_packaging_and_each_installer() {
		struct InstallerPackager;
		impl Packager for InstallerPackager {
			fn package(&self, config: &cargo_packager::Config) -> Result<Vec<Packaged>> {
				// The packager must read the signed copy, not the build output.
				assert!(config.binaries_dir.as_ref().unwrap().ends_with("shared"));
				let path = config.out_dir.join("App-setup.exe");
				fs::write(&path, b"installer").unwrap();
				Ok(vec![Packaged {
					format: "nsis".to_string(),
					paths: vec![path],
				}])
			}
		}
		let bundle = temp("windows");
		let signing = Signing {
			macos: None,
			windows: Some(Authenticode {
				certificate: Certificate::Thumbprint("ABCDEF".to_string()),
				timestamp_url: "http://ts.example".to_string(),
			}),
		};
		let plan = Plan {
			binaries_dir: bundle.join("target-release"),
			..plan(&bundle, Some(vec!["nsis".into()]), signing)
		};
		fs::create_dir_all(&plan.binaries_dir).unwrap();
		fs::write(plan.binaries_dir.join("preview.exe"), b"exe").unwrap();
		let recorder = Recorder::default();
		publish(&plan, &InstallerPackager, &recorder).unwrap();
		let calls = recorder.calls.into_inner();
		assert_eq!(calls.len(), 2);
		assert_eq!(calls[0].0, "signtool");
		assert!(calls[0].1[0] == "sign" && calls[0].1.last().unwrap().ends_with("preview.exe"));
		assert!(calls[1].1.last().unwrap().ends_with("App-setup.exe"));
		fs::remove_dir_all(&bundle).unwrap();
	}

	#[test]
	fn the_standalone_exe_is_split_from_the_packager_formats() {
		let settings: Settings = serde_json::from_value(json!({ "formats": ["exe", "nsis"] })).unwrap();
		assert_eq!(
			requested_formats(None, &settings),
			Some(vec!["exe".to_string(), "nsis".to_string()])
		);
		let cli = requested_formats(Some(vec!["deb".to_string()]), &settings);
		assert_eq!(cli, Some(vec!["deb".to_string()]));
		let standalone = requested_formats(
			Some(vec![STANDALONE.to_string(), standalone_format().to_string()]),
			&Settings::default(),
		);
		assert_eq!(standalone, Some(vec![standalone_format().to_string()]));
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
	fn only_the_macos_formats_get_the_icon_margin() {
		let groups = format_groups(Some(vec!["nsis".into(), "dmg".into(), "app".into()]));
		assert_eq!(groups[0], (true, Some(vec!["dmg".into(), "app".into()])));
		assert_eq!(groups[1], (false, Some(vec!["nsis".into()])));
	}

	#[test]
	fn the_config_for_a_run_carries_the_packager_formats() {
		let settings: Settings =
			serde_json::from_value(json!({ "identifier": "dev.example.preview" })).unwrap();
		let run = Run {
			product_name: "App",
			version: "1.0.0",
			identifier: "dev.example.preview",
			package: &json!({}),
			settings: &settings,
			macos: None,
			bin: "preview",
			binaries_dir: Path::new("/target/release"),
			out_dir: Path::new("/out"),
			manifest_dir: Path::new("/app"),
			icons: &[],
			formats: Some(&["deb".to_string()]),
		};
		let config = config(&run).unwrap();
		assert_eq!(config.formats.unwrap().len(), 1);
	}
}
