// SPDX-License-Identifier: MPL-2.0

//! Generates the `Lucide` icon enum from `assets/lucide` when the `lucide` feature is on.

use std::{collections::BTreeMap, env, fmt::Write as _, fs, path::PathBuf};

fn main() {
	println!("cargo::rerun-if-changed=assets/lucide");
	println!("cargo::rerun-if-env-changed=CARGO_FEATURE_LUCIDE");
	if env::var_os("CARGO_FEATURE_LUCIDE").is_none() {
		return;
	}
	let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
	let mut icons = BTreeMap::new();
	for entry in fs::read_dir(manifest.join("assets/lucide")).expect("assets/lucide exists") {
		let file = entry
			.expect("readable directory entry")
			.file_name()
			.into_string()
			.expect("UTF-8 file name");
		let Some(stem) = file.strip_suffix(".svg") else {
			continue;
		};
		// Digits keep an underscore between number groups: `arrow-down-0-1` → `ArrowDown0_1`
		// next to `arrow-down-01` → `ArrowDown01`.
		let mut variant = String::new();
		for part in stem.split('-') {
			if part.starts_with(|c: char| c.is_ascii_digit())
				&& variant.ends_with(|c: char| c.is_ascii_digit())
			{
				variant.push('_');
			}
			let mut characters = part.chars();
			if let Some(first) = characters.next() {
				variant.push(first.to_ascii_uppercase());
				variant.push_str(characters.as_str());
			}
		}
		if let Some(previous) = icons.insert(variant.clone(), file.clone()) {
			panic!("Lucide icons {previous} and {file} both map to {variant}");
		}
	}

	let mut source = String::from(
		"crate::icons! {\n\t/// Every icon of the vendored Lucide set (ISC), named in PascalCase.\n\tpub enum Lucide in \"assets/lucide\" {\n",
	);
	for (variant, file) in &icons {
		writeln!(source, "\t\t{variant} => \"{file}\",").expect("writing to a String succeeds");
	}
	source.push_str("\t}\n}\n");
	let out = PathBuf::from(env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
	fs::write(out.join("lucide.rs"), source).expect("OUT_DIR is writable");
}
