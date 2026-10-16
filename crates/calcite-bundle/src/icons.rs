// SPDX-License-Identifier: MPL-2.0

//! The icon set for a packager run, generated from the one source icon.

use std::{
	fs,
	path::{Path, PathBuf},
};

use crate::package::Result;

/// Pixel sizes generated from the source icon. The packager turns them into `.icns` on
/// macOS, `.ico` on Windows, and hicolor PNGs on Linux.
const ICON_SIZES: [u32; 6] = [32, 64, 128, 256, 512, 1024];

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
pub fn generate(source: &Path, directory: &Path, pad_for_macos: bool) -> Result<Vec<PathBuf>> {
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

#[cfg(test)]
mod tests {
	use super::*;

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
		for (pad_for_macos, name) in [(true, "app"), (false, "deb")] {
			let directory = std::env::temp_dir().join(format!("calcite-icons-{}-{name}", std::process::id()));
			generate(&source, &directory, pad_for_macos).unwrap();
			let icon = image::open(directory.join("512x512.png")).unwrap().to_rgba8();
			fs::remove_dir_all(&directory).ok();
			let corner = icon.get_pixel(0, 0)[3];
			let centre = icon.get_pixel(256, 256)[3];
			if pad_for_macos {
				assert_eq!((corner, centre), (0, 255), "{name}");
			} else {
				assert_eq!(corner, 255, "{name}");
			}
		}
		fs::remove_file(&source).ok();
	}
}
