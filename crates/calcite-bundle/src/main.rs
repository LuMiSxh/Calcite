// SPDX-License-Identifier: MPL-2.0

//! `cargo calcite bundle`: packages a Calcite app. See the crate docs for the settings.

use std::process::ExitCode;

use calcite_bundle::{Options, bundle};

const USAGE: &str = "\
Packages a Calcite app for distribution.

Usage: cargo calcite bundle [OPTIONS]

Options:
  -p, --package NAME   The package to bundle (default: the one in this directory)
      --bin NAME       The binary to bundle (default: the package's main binary)
  -f, --format LIST    Comma-separated formats: app, dmg, nsis, wix, deb, appimage, pacman
      --debug          Build and package the debug profile
      --no-build       Package an existing build
  -h, --help           Show this help

Settings live in [package.metadata.calcite.bundle] of the app's Cargo.toml.";

fn main() -> ExitCode {
	// Cargo runs `cargo-calcite calcite bundle ...`; a direct call is `cargo-calcite bundle ...`.
	let mut args: Vec<String> = std::env::args().skip(1).collect();
	if args.first().map(String::as_str) == Some("calcite") {
		args.remove(0);
	}
	match args.first().map(String::as_str) {
		Some("bundle") => {}
		Some("-h" | "--help") | None => {
			println!("{USAGE}");
			return ExitCode::SUCCESS;
		}
		Some(other) => {
			eprintln!("error: unknown command `{other}`\n\n{USAGE}");
			return ExitCode::FAILURE;
		}
	}
	let options = match parse(&args[1..]) {
		Ok(Some(options)) => options,
		Ok(None) => {
			println!("{USAGE}");
			return ExitCode::SUCCESS;
		}
		Err(error) => {
			eprintln!("error: {error}\n\n{USAGE}");
			return ExitCode::FAILURE;
		}
	};
	match bundle(&options) {
		Ok(outputs) => {
			for output in outputs {
				for path in output.paths {
					println!("{:>9}  {}", output.format, path.display());
				}
			}
			ExitCode::SUCCESS
		}
		Err(error) => {
			eprintln!("error: {error}");
			ExitCode::FAILURE
		}
	}
}

/// `None` asks for the help text.
fn parse(args: &[String]) -> Result<Option<Options>, String> {
	let mut options = Options::default();
	let mut args = args.iter();
	while let Some(arg) = args.next() {
		let mut value = |name: &str| {
			args.next()
				.cloned()
				.ok_or_else(|| format!("{name} needs a value"))
		};
		match arg.as_str() {
			"-p" | "--package" => options.package = Some(value(arg)?),
			"--bin" => options.bin = Some(value(arg)?),
			"-f" | "--format" | "--formats" => {
				options.formats = Some(
					value(arg)?
						.split(',')
						.map(|format| format.trim().to_string())
						.collect(),
				)
			}
			"--debug" => options.debug = true,
			"--no-build" => options.no_build = true,
			"-h" | "--help" => return Ok(None),
			other => return Err(format!("unknown option `{other}`")),
		}
	}
	Ok(Some(options))
}
