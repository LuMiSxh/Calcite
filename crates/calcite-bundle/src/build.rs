// SPDX-License-Identifier: MPL-2.0

//! Build-script helpers.

use std::{
	env, fs,
	path::{Path, PathBuf},
};

use image::{
	codecs::ico::{IcoEncoder, IcoFrame},
	imageops::FilterType,
};

/// Sizes in the Windows `.ico`; Explorer and the task bar pick from them.
const ICO_SIZES: [u32; 6] = [16, 24, 32, 48, 64, 256];

/// Embeds `png` (relative to the package's `Cargo.toml`) as icon resource 1 of the Windows
/// executable, the icon GPUI loads for its windows and Explorer shows for the file. Does
/// nothing when the target is not Windows. Call it from `build.rs`:
///
/// ```no_run
/// calcite_bundle::build::windows_icon("assets/icon.png");
/// ```
///
/// # Panics
///
/// Panics with a readable message when the PNG is missing or invalid, or when no resource
/// compiler is available (`rc.exe` with MSVC, `windres` or `llvm-rc` otherwise).
pub fn windows_icon(png: impl AsRef<Path>) {
	let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("run from a build script"));
	let png = manifest_dir.join(png);
	println!("cargo::rerun-if-changed={}", png.display());
	if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
		return;
	}
	let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("run from a build script"));
	let ico = out_dir.join("app.ico");
	write_ico(&png, &ico).unwrap_or_else(|error| panic!("calcite-bundle: {error}"));
	let rc = out_dir.join("app-icon.rc");
	// Resource compilers read backslashes in strings as escapes.
	let ico_path = ico.display().to_string().replace('\\', "/");
	fs::write(&rc, format!("1 ICON \"{ico_path}\"\n")).expect("OUT_DIR is writable");
	embed_resource::compile(&rc, embed_resource::NONE)
		.manifest_optional()
		.unwrap_or_else(|error| panic!("calcite-bundle: cannot embed the Windows icon: {error}"));
}

/// Writes a multi-size `.ico` from a square PNG.
pub fn write_ico(png: &Path, ico: &Path) -> Result<(), String> {
	let image = image::open(png).map_err(|error| format!("icon {}: {error}", png.display()))?;
	if image.width() != image.height() {
		return Err(format!("icon {}: must be square, is {}×{}", png.display(), image.width(), image.height()));
	}
	let frames = ICO_SIZES
		.iter()
		.map(|&size| {
			let frame = image.resize_exact(size, size, FilterType::Lanczos3).to_rgba8();
			IcoFrame::as_png(frame.as_raw(), size, size, image::ExtendedColorType::Rgba8)
				.map_err(|error| format!("icon {} at {size} px: {error}", png.display()))
		})
		.collect::<Result<Vec<_>, _>>()?;
	let file = fs::File::create(ico).map_err(|error| format!("{}: {error}", ico.display()))?;
	IcoEncoder::new(file)
		.encode_images(&frames)
		.map_err(|error| format!("{}: {error}", ico.display()))
}

