// SPDX-License-Identifier: MPL-2.0

//! Command-line flags of the developer preview: the section to open, the palette, the motion and
//! transparency settings, and the glass style. Parsed with std only. `--help` prints the usage
//! and exits 0; an unknown flag or value prints it to stderr and exits 2.

use calcite::{GlassStyle, Transparency, prelude::*};

use super::{EMBER, LIGHT, section_view::Kind};

pub const USAGE: &str = "\
usage: preview [OPTIONS]

Options:
  --section <NAME>        open this section, e.g. buttons, forms, motion-lab
  --palette <NAME>        neutral (the default dark look), arknights, ember, or light
  --reduce-motion         start with Reduce Motion on
  --reduce-transparency   start with Reduce Transparency on
  --glass <STYLE>         macos26, macos27-clear, macos27-default, macos27-tinted,
                          or reduce-transparency
  -h, --help              print this help and exit";

/// What the flags ask for. An absent flag keeps the default.
#[derive(Default)]
pub struct Options {
	pub section: Option<Kind>,
	pub palette: Option<Palette>,
	pub reduce_motion: bool,
	pub reduce_transparency: bool,
	pub glass: Option<GlassStyle>,
}

impl Options {
	/// The process arguments, parsed; exits the process on `--help` or a bad argument.
	pub fn from_env() -> Self {
		match Self::parse(std::env::args().skip(1)) {
			Ok(Some(options)) => options,
			Ok(None) => {
				println!("{USAGE}");
				std::process::exit(0);
			}
			Err(message) => {
				eprintln!("preview: {message}\n\n{USAGE}");
				std::process::exit(2);
			}
		}
	}

	/// `Ok(None)` asks for the usage. Accepts `--flag value` and `--flag=value`.
	fn parse(args: impl IntoIterator<Item = String>) -> Result<Option<Self>, String> {
		let mut options = Self::default();
		let mut args = args.into_iter();
		while let Some(arg) = args.next() {
			// `just run preview -- --section buttons` leaves a bare `--` in some setups.
			if arg == "--" {
				continue;
			}
			let (flag, inline) = match arg.split_once('=') {
				Some((flag, value)) => (flag.to_string(), Some(value.to_string())),
				None => (arg, None),
			};
			let switch = |on: bool| -> Result<bool, String> {
				match &inline {
					Some(_) => Err(format!("{flag} takes no value")),
					None => Ok(on),
				}
			};
			match flag.as_str() {
				"-h" | "--help" => return Ok(None),
				"--reduce-motion" => options.reduce_motion = switch(true)?,
				"--reduce-transparency" => options.reduce_transparency = switch(true)?,
				"--section" | "--palette" | "--glass" => {
					let value = match &inline {
						Some(value) => value.clone(),
						None => args.next().ok_or_else(|| format!("{flag} needs a value"))?,
					};
					match flag.as_str() {
						"--section" => {
							options.section =
								Some(section(&value).ok_or_else(|| format!("unknown section `{value}`"))?)
						}
						"--palette" => {
							options.palette =
								Some(palette(&value).ok_or_else(|| format!("unknown palette `{value}`"))?)
						}
						_ => {
							options.glass =
								Some(glass(&value).ok_or_else(|| format!("unknown glass style `{value}`"))?)
						}
					}
				}
				_ => return Err(format!("unknown option `{flag}`")),
			}
		}
		Ok(Some(options))
	}

	/// The theme the flags start from: `base` with the palette, glass and motion settings applied.
	pub fn theme(&self, mut base: Theme) -> Theme {
		if let Some(palette) = self.palette {
			base = base.with_palette(palette);
		}
		if let Some(glass) = self.glass {
			base.style.glass = glass;
		}
		if self.reduce_transparency {
			base.style.glass = GlassStyle::ReducedTransparency;
		}
		base.reduce_motion |= self.reduce_motion;
		base
	}
}

/// A section by its sidebar title, lowercase and dashed (`motion-lab`), or by the first word of
/// its title (`buttons` for "Buttons and actions").
fn section(name: &str) -> Option<Kind> {
	let name = slug(name);
	Kind::ALL
		.into_iter()
		.find(|kind| slug(kind.title()) == name)
		.or_else(|| {
			Kind::ALL
				.into_iter()
				.find(|kind| slug(kind.title()).split('-').next() == Some(name.as_str()))
		})
}

fn slug(text: &str) -> String {
	text.split(|c: char| !c.is_ascii_alphanumeric())
		.filter(|word| !word.is_empty())
		.map(str::to_ascii_lowercase)
		.collect::<Vec<_>>()
		.join("-")
}

fn palette(name: &str) -> Option<Palette> {
	match name.to_ascii_lowercase().as_str() {
		"neutral" => Some(Palette::NEUTRAL),
		"arknights" => Some(Palette::ARKNIGHTS),
		"ember" => Some(EMBER),
		"light" => Some(LIGHT),
		_ => None,
	}
}

fn glass(name: &str) -> Option<GlassStyle> {
	Some(match name.to_ascii_lowercase().as_str() {
		"macos26" => GlassStyle::MacOS26,
		"macos27-clear" => GlassStyle::MacOS27(Transparency::Clear),
		"macos27" | "macos27-default" => GlassStyle::MacOS27(Transparency::Default),
		"macos27-tinted" => GlassStyle::MacOS27(Transparency::Tinted),
		"reduce-transparency" => GlassStyle::ReducedTransparency,
		_ => return None,
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	fn args(line: &str) -> Vec<String> {
		line.split_whitespace().map(str::to_string).collect()
	}

	#[test]
	fn sections_match_by_slug_or_first_word() {
		assert!(matches!(section("buttons"), Some(Kind::Actions)));
		assert!(matches!(section("Motion Lab"), Some(Kind::MotionLab)));
		assert!(matches!(section("motion"), Some(Kind::Motion)));
		assert!(section("nope").is_none());
	}

	#[test]
	fn flags_parse_with_or_without_equals() {
		let options = Options::parse(args("--section=forms --reduce-motion --glass macos27-tinted"))
			.expect("valid flags")
			.expect("not help");
		assert!(matches!(options.section, Some(Kind::Forms)));
		assert!(options.reduce_motion && !options.reduce_transparency);
		assert!(matches!(
			options.glass,
			Some(GlassStyle::MacOS27(Transparency::Tinted))
		));
	}

	#[test]
	fn bad_arguments_are_errors() {
		assert!(Options::parse(args("--frobnicate")).is_err());
		assert!(Options::parse(args("--section")).is_err());
		assert!(Options::parse(args("--palette neon")).is_err());
		assert!(Options::parse(args("--reduce-motion=yes")).is_err());
		assert!(matches!(Options::parse(args("--help")), Ok(None)));
	}
}
