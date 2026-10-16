// SPDX-License-Identifier: MPL-2.0

//! Signing and notarization. cargo-packager signs the macOS `.app` (with the hardened runtime
//! and the timestamp) and notarizes it; this module notarizes and staples the DMG, and signs the
//! Windows binary and installers with `signtool`. Every tool call goes through [`Commands`], so
//! tests check the exact argv without running a tool.

use std::{
	path::{Path, PathBuf},
	process::Command,
};

use serde_json::Value;

use crate::{
	config::{MacosSettings, Settings, WindowsSettings},
	package::Result,
};

/// Runs an external tool and returns its standard output.
pub trait Commands {
	fn run(&self, program: &str, args: &[String]) -> Result<String>;
}

/// Runs the tools on this machine.
pub struct SystemCommands;

impl Commands for SystemCommands {
	fn run(&self, program: &str, args: &[String]) -> Result<String> {
		let output = Command::new(program)
			.args(args)
			.output()
			.map_err(|error| format!("{program}: {error}"))?;
		if !output.status.success() {
			return Err(format!(
				"{program} failed ({})\n{}{}",
				output.status,
				String::from_utf8_lossy(&output.stdout),
				String::from_utf8_lossy(&output.stderr)
			));
		}
		Ok(String::from_utf8_lossy(&output.stdout).into_owned())
	}
}

/// How `notarytool` authenticates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotaryCredentials {
	AppleId {
		apple_id: String,
		password: String,
		team_id: String,
	},
	KeychainProfile(String),
}

impl NotaryCredentials {
	/// From `APPLE_NOTARY_KEYCHAIN_PROFILE`, else from `APPLE_ID`, `APPLE_TEAM_ID`, and
	/// `APPLE_APP_PASSWORD`. `None` when neither set is complete.
	pub fn from_env(env: impl Fn(&str) -> Option<String>) -> Option<Self> {
		if let Some(profile) = env("APPLE_NOTARY_KEYCHAIN_PROFILE") {
			return Some(Self::KeychainProfile(profile));
		}
		Some(Self::AppleId {
			apple_id: env("APPLE_ID")?,
			password: env("APPLE_APP_PASSWORD")?,
			team_id: env("APPLE_TEAM_ID")?,
		})
	}

	/// The `notarytool` arguments that select these credentials.
	pub fn notarytool_args(&self) -> Vec<String> {
		match self {
			Self::AppleId {
				apple_id,
				password,
				team_id,
			} => [
				"--apple-id",
				apple_id,
				"--password",
				password,
				"--team-id",
				team_id,
			]
			.map(String::from)
			.to_vec(),
			Self::KeychainProfile(profile) => ["--keychain-profile", profile].map(String::from).to_vec(),
		}
	}

	/// The same credentials in cargo-packager's form, for its `.app` notarization.
	pub fn to_packager(&self) -> cargo_packager::config::MacOsNotarizationCredentials {
		use cargo_packager::config::MacOsNotarizationCredentials as Packager;
		match self {
			Self::AppleId {
				apple_id,
				password,
				team_id,
			} => Packager::AppleId {
				apple_id: apple_id.into(),
				password: password.into(),
				team_id: team_id.into(),
			},
			Self::KeychainProfile(profile) => Packager::KeychainProfile {
				keychain_profile: profile.into(),
			},
		}
	}
}

/// Submits a DMG to Apple's notary service, waits for the verdict, and staples the ticket.
pub fn notarize_dmg(commands: &impl Commands, dmg: &Path, credentials: &NotaryCredentials) -> Result<()> {
	let dmg = dmg.display().to_string();
	let mut submit = ["notarytool", "submit", &dmg, "--wait", "--output-format", "json"]
		.map(String::from)
		.to_vec();
	submit.extend(credentials.notarytool_args());
	let output = commands.run("xcrun", &submit)?;
	let verdict: Value = serde_json::from_str(&output)
		.map_err(|error| format!("notarytool: unreadable verdict: {error}\n{output}"))?;
	let status = verdict["status"].as_str().unwrap_or("unknown");
	if status != "Accepted" {
		return Err(format!(
			"notarization of {dmg} was not accepted (status {status}); run `xcrun notarytool log` with id {}",
			verdict["id"].as_str().unwrap_or("?")
		));
	}
	commands.run("xcrun", &["stapler", "staple", "-v", &dmg].map(String::from))?;
	Ok(())
}

/// The `signtool` certificate: a store thumbprint, or a PFX file with an optional password.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Certificate {
	Thumbprint(String),
	Pfx { path: PathBuf, password: Option<String> },
}

/// Authenticode settings for Windows outputs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Authenticode {
	pub certificate: Certificate,
	pub timestamp_url: String,
}

impl Authenticode {
	/// `signtool sign /fd sha256 /tr URL /td sha256 <certificate> FILE`.
	pub fn signtool_args(&self, file: &Path) -> Vec<String> {
		let mut args: Vec<String> = [
			"sign",
			"/fd",
			"sha256",
			"/tr",
			&self.timestamp_url,
			"/td",
			"sha256",
		]
		.map(String::from)
		.to_vec();
		match &self.certificate {
			Certificate::Thumbprint(thumbprint) => {
				args.extend(["/sha1".to_string(), thumbprint.clone()]);
			}
			Certificate::Pfx { path, password } => {
				args.extend(["/f".to_string(), path.display().to_string()]);
				if let Some(password) = password {
					args.extend(["/p".to_string(), password.clone()]);
				}
			}
		}
		args.push(file.display().to_string());
		args
	}

	pub fn sign(&self, commands: &impl Commands, file: &Path) -> Result<()> {
		commands
			.run("signtool", &self.signtool_args(file))
			.map(|_| ())
			.map_err(|error| format!("signing {}: {error}", file.display()))
	}
}

/// The macOS signing inputs.
#[derive(Clone, Debug)]
pub struct MacosSigning {
	pub identity: String,
	pub entitlements: Option<PathBuf>,
	pub notary: NotaryCredentials,
}

/// Every signing input resolved from the manifest and the environment.
#[derive(Clone, Debug, Default)]
pub struct Signing {
	pub macos: Option<MacosSigning>,
	pub windows: Option<Authenticode>,
}

/// Resolves the signing inputs. `env` reads environment variables; tests pass a closure.
/// Signing is off unless the manifest names an identity or a Windows certificate.
pub fn resolve(
	settings: &Settings,
	manifest_dir: &Path,
	manifest: &str,
	env: impl Fn(&str) -> Option<String>,
) -> Result<Signing> {
	let macos = MacosSettings::from_settings(settings, manifest)?;
	let windows = WindowsSettings::from_settings(settings, manifest)?;
	Ok(Signing {
		macos: resolve_macos(&macos, manifest_dir, manifest, &env)?,
		windows: resolve_windows(&windows, manifest, &env)?,
	})
}

fn resolve_macos(
	settings: &MacosSettings,
	manifest_dir: &Path,
	manifest: &str,
	env: &impl Fn(&str) -> Option<String>,
) -> Result<Option<MacosSigning>> {
	let Some(identity) = &settings.signing_identity else {
		return Ok(None);
	};
	let notary = NotaryCredentials::from_env(env).ok_or_else(|| {
		format!(
			"{manifest}: signing_identity is set, so the DMG must be notarized: set \
			 APPLE_NOTARY_KEYCHAIN_PROFILE, or APPLE_ID, APPLE_TEAM_ID, and APPLE_APP_PASSWORD"
		)
	})?;
	Ok(Some(MacosSigning {
		identity: identity.clone(),
		entitlements: settings.entitlements.as_ref().map(|path| manifest_dir.join(path)),
		notary,
	}))
}

fn resolve_windows(
	settings: &WindowsSettings,
	manifest: &str,
	env: &impl Fn(&str) -> Option<String>,
) -> Result<Option<Authenticode>> {
	let pfx = env("CALCITE_SIGN_CERT");
	let certificate = match (&settings.certificate_thumbprint, pfx) {
		(Some(_), Some(_)) => {
			return Err(format!(
				"{manifest}: set one certificate: certificate-thumbprint or CALCITE_SIGN_CERT, not both"
			));
		}
		(Some(thumbprint), None) => Certificate::Thumbprint(thumbprint.clone()),
		(None, Some(path)) => Certificate::Pfx {
			path: PathBuf::from(path),
			password: env("CALCITE_SIGN_PASSWORD"),
		},
		(None, None) => {
			if env("CALCITE_SIGN_PASSWORD").is_some() {
				return Err("CALCITE_SIGN_PASSWORD needs CALCITE_SIGN_CERT".to_string());
			}
			return Ok(None);
		}
	};
	let timestamp_url = settings.timestamp_url.clone().ok_or_else(|| {
		format!(
			"{manifest}: Windows signing needs `timestamp-url` in [package.metadata.calcite.bundle.windows]"
		)
	})?;
	Ok(Some(Authenticode {
		certificate,
		timestamp_url,
	}))
}

#[cfg(test)]
mod tests {
	use std::{cell::RefCell, collections::HashMap};

	use super::*;

	/// Records every call and answers `xcrun notarytool` with a fixed verdict.
	struct Recorder {
		calls: RefCell<Vec<(String, Vec<String>)>>,
		verdict: &'static str,
	}

	impl Commands for Recorder {
		fn run(&self, program: &str, args: &[String]) -> Result<String> {
			self.calls.borrow_mut().push((program.to_string(), args.to_vec()));
			Ok(if args.first().map(String::as_str) == Some("notarytool") {
				format!(r#"{{"status":"{}","id":"abc"}}"#, self.verdict)
			} else {
				String::new()
			})
		}
	}

	fn env(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
		let map: HashMap<String, String> = vars
			.iter()
			.map(|(key, value)| (key.to_string(), value.to_string()))
			.collect();
		move |key| map.get(key).cloned()
	}

	#[test]
	fn the_dmg_is_submitted_then_stapled_with_apple_id_credentials() {
		let recorder = Recorder {
			calls: RefCell::default(),
			verdict: "Accepted",
		};
		let credentials = NotaryCredentials::from_env(env(&[
			("APPLE_ID", "dev@example.com"),
			("APPLE_TEAM_ID", "TEAM123"),
			("APPLE_APP_PASSWORD", "app-pw"),
		]))
		.unwrap();
		notarize_dmg(&recorder, Path::new("/out/App.dmg"), &credentials).unwrap();
		let calls = recorder.calls.into_inner();
		let submit: Vec<&str> = calls[0].1.iter().map(String::as_str).collect();
		assert_eq!(
			submit,
			[
				"notarytool",
				"submit",
				"/out/App.dmg",
				"--wait",
				"--output-format",
				"json",
				"--apple-id",
				"dev@example.com",
				"--password",
				"app-pw",
				"--team-id",
				"TEAM123",
			]
		);
		assert_eq!(calls[1].1, ["stapler", "staple", "-v", "/out/App.dmg"]);
	}

	#[test]
	fn a_rejected_dmg_is_not_stapled() {
		let recorder = Recorder {
			calls: RefCell::default(),
			verdict: "Invalid",
		};
		let credentials =
			NotaryCredentials::from_env(env(&[("APPLE_NOTARY_KEYCHAIN_PROFILE", "ci")])).unwrap();
		let error = notarize_dmg(&recorder, Path::new("/out/App.dmg"), &credentials).unwrap_err();
		assert!(error.contains("status Invalid"), "{error}");
		assert_eq!(recorder.calls.borrow().len(), 1);
	}

	#[test]
	fn thumbprint_signing_builds_the_signtool_argv() {
		let signing = Authenticode {
			certificate: Certificate::Thumbprint("ABCDEF".to_string()),
			timestamp_url: "http://ts.example".to_string(),
		};
		assert_eq!(
			signing.signtool_args(Path::new("C:/out/App_x64-setup.exe")),
			[
				"sign",
				"/fd",
				"sha256",
				"/tr",
				"http://ts.example",
				"/td",
				"sha256",
				"/sha1",
				"ABCDEF",
				"C:/out/App_x64-setup.exe",
			]
		);
	}

	#[test]
	fn pfx_signing_passes_the_file_and_password() {
		let signing = Authenticode {
			certificate: Certificate::Pfx {
				path: PathBuf::from("C:/certs/app.pfx"),
				password: Some("pw".to_string()),
			},
			timestamp_url: "http://ts.example".to_string(),
		};
		assert_eq!(
			signing.signtool_args(Path::new("app.exe")),
			[
				"sign",
				"/fd",
				"sha256",
				"/tr",
				"http://ts.example",
				"/td",
				"sha256",
				"/f",
				"C:/certs/app.pfx",
				"/p",
				"pw",
				"app.exe",
			]
		);
	}

	#[test]
	fn a_thumbprint_and_a_pfx_are_ambiguous() {
		let settings = Settings {
			windows: Some(serde_json::json!({
				"certificate-thumbprint": "ABCDEF",
				"timestamp-url": "http://ts.example",
			})),
			..Settings::default()
		};
		let error = resolve(
			&settings,
			Path::new("/app"),
			"/app/Cargo.toml",
			env(&[("CALCITE_SIGN_CERT", "app.pfx")]),
		)
		.unwrap_err();
		assert!(error.contains("not both"), "{error}");
	}

	#[test]
	fn signing_stays_off_without_identity_or_certificate() {
		let signing = resolve(
			&Settings::default(),
			Path::new("/app"),
			"/app/Cargo.toml",
			env(&[]),
		)
		.unwrap();
		assert!(signing.macos.is_none() && signing.windows.is_none());
	}
}
