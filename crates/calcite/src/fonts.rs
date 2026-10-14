// SPDX-License-Identifier: MPL-2.0

//! The fonts Calcite ships, all under the SIL Open Font License 1.1 (see
//! `assets/fonts/README.md`): Plus Jakarta Sans for the UI and Fira Code for code.
//!
//! With the default `fonts` feature, [`register`] adds them to the gpui text system, and
//! [`init`](crate::init) calls it. Without the feature, [`register`] does nothing and the app
//! supplies its own fonts. Which family the components use comes from
//! [`Style::fonts`](crate::style::Style::fonts), so an app can pick any installed family.
//!
//! The bundled set is the single `TABLE`: swapping a family means editing that table, the
//! [`Family`] variant, and the files under `assets/fonts`.
//!
//! ```ignore
//! use gpui::font;
//! use calcite::fonts::Family;
//!
//! let code = font(Family::FiraCode.name());
//! ```

#[cfg(feature = "fonts")]
use std::borrow::Cow;

use gpui::App;

/// A family that Calcite bundles. [`Family::name`] is the name to pass to `gpui::font`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
	/// The UI sans. Regular, Medium, and SemiBold are bundled.
	PlusJakartaSans,
	/// The monospace for code, through [`Theme::mono`](crate::theme::Theme::mono). Regular and
	/// Bold are bundled.
	FiraCode,
}

/// One bundled family: its enum entry, the name the text system resolves, and its font files.
struct Entry {
	family: Family,
	name: &'static str,
	#[cfg(feature = "fonts")]
	files: &'static [&'static [u8]],
}

/// Every bundled family with its files. The only place that names fonts.
const TABLE: &[Entry] = &[
	Entry {
		family: Family::PlusJakartaSans,
		name: "Plus Jakarta Sans",
		#[cfg(feature = "fonts")]
		files: &[
			include_bytes!("../assets/fonts/plus-jakarta-sans/PlusJakartaSans-Regular.ttf"),
			include_bytes!("../assets/fonts/plus-jakarta-sans/PlusJakartaSans-Medium.ttf"),
			include_bytes!("../assets/fonts/plus-jakarta-sans/PlusJakartaSans-SemiBold.ttf"),
		],
	},
	Entry {
		family: Family::FiraCode,
		name: "Fira Code",
		#[cfg(feature = "fonts")]
		files: &[
			include_bytes!("../assets/fonts/fira-code/FiraCode-Regular.ttf"),
			include_bytes!("../assets/fonts/fira-code/FiraCode-Bold.ttf"),
		],
	},
];

impl Family {
	/// Every bundled family.
	pub const ALL: [Family; 2] = [Family::PlusJakartaSans, Family::FiraCode];

	/// The family name the text system resolves, such as `"Fira Code"`.
	pub fn name(self) -> &'static str {
		TABLE
			.iter()
			.find(|entry| entry.family == self)
			.map_or("", |entry| entry.name)
	}
}

/// Adds the bundled fonts to the gpui text system, so `gpui::font(Family::X.name())` resolves.
/// [`init`](crate::init) and [`init_with`](crate::init_with) call this; call it yourself only
/// when you set up the text system without them.
pub fn register(cx: &mut App) {
	#[cfg(feature = "fonts")]
	{
		let fonts = TABLE
			.iter()
			.flat_map(|entry| entry.files.iter().map(|bytes| Cow::Borrowed(*bytes)))
			.collect();
		cx.text_system()
			.add_fonts(fonts)
			.expect("Calcite's bundled fonts are valid TrueType files");
	}
	#[cfg(not(feature = "fonts"))]
	let _ = cx;
}

#[cfg(all(test, feature = "fonts"))]
mod tests {
	use super::*;

	/// The `TrueType` (`0x00010000`), `true`, and `OpenType` (`OTTO`) signatures.
	fn has_sfnt_signature(bytes: &[u8]) -> bool {
		matches!(bytes.get(..4), Some([0, 1, 0, 0] | b"true" | b"OTTO"))
	}

	#[test]
	fn every_bundled_file_is_a_font_of_its_family() {
		assert_eq!(TABLE.iter().map(|entry| entry.files.len()).sum::<usize>(), 5);
		for entry in TABLE {
			for bytes in entry.files {
				assert!(
					has_sfnt_signature(bytes),
					"{} is not a TrueType or OpenType file",
					entry.name
				);
				let font = skrifa::FontRef::new(bytes).expect("the font table directory parses");
				let names: Vec<String> = [
					skrifa::string::StringId::FAMILY_NAME,
					skrifa::string::StringId::TYPOGRAPHIC_FAMILY_NAME,
				]
				.into_iter()
				.filter_map(|id| {
					skrifa::string::LocalizedStrings::new(&font, id)
						.english_or_first()
						.map(|name| name.to_string())
				})
				.collect();
				assert!(
					names.iter().any(|name| name == entry.name),
					"{:?} does not name the family {}",
					names,
					entry.name
				);
			}
		}
	}

	#[test]
	fn families_have_stable_names() {
		let names: Vec<&str> = Family::ALL.iter().map(|family| family.name()).collect();
		assert_eq!(names, ["Plus Jakarta Sans", "Fira Code"]);
		assert_eq!(TABLE.len(), Family::ALL.len());
	}
}
