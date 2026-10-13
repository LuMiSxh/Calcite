// SPDX-License-Identifier: MPL-2.0

//! Dynamic theme colors from artwork. Port of the launcher's `WallpaperColorExtractor`.
//!
//! The crate finds the dominant hue of an image and renders it two ways: a legible accent
//! for controls and text, and the same hue, desaturated and darkened, for tinting neutral
//! chrome. A dynamic theme changes only these two values; the rest of the palette stays.
//!
//! It has no dependencies and no GPU or UI code, so it runs on any thread.
//!
//! ```
//! use calcite_tint::{Extractor, Hsb};
//!
//! // Four RGBA pixels of a teal image.
//! let pixels = [20, 140, 150, 255].repeat(4);
//! let tint = Extractor::default().extract(&pixels, 2, 2).expect("the image has a hue");
//! assert!((tint.accent.hue - 0.51).abs() < 0.02);
//! let [red, green, blue] = tint.accent.to_rgb();
//! # let _ = (red, green, blue, Hsb::new(0., 0., 0.));
//! ```

/// A color as hue, saturation, and brightness, each in 0..=1 (SwiftUI's `Color(hue:...)`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hsb {
	pub hue: f32,
	pub saturation: f32,
	pub brightness: f32,
}

impl Hsb {
	pub const fn new(hue: f32, saturation: f32, brightness: f32) -> Self {
		Self {
			hue,
			saturation,
			brightness,
		}
	}

	/// Converts straight sRGB in 0..=1.
	pub fn from_rgb(red: f32, green: f32, blue: f32) -> Self {
		let max = red.max(green).max(blue);
		let min = red.min(green).min(blue);
		let delta = max - min;
		let saturation = if max == 0. { 0. } else { delta / max };
		if delta == 0. {
			return Self::new(0., saturation, max);
		}
		let sector = if max == red {
			((green - blue) / delta).rem_euclid(6.)
		} else if max == green {
			(blue - red) / delta + 2.
		} else {
			(red - green) / delta + 4.
		};
		Self::new(sector / 6., saturation, max)
	}

	/// Straight sRGB in 0..=1.
	pub fn to_rgb(self) -> [f32; 3] {
		let h = self.hue.rem_euclid(1.) * 6.;
		let c = self.brightness * self.saturation;
		let x = c * (1. - ((h % 2.) - 1.).abs());
		let m = self.brightness - c;
		let (r, g, b) = match h as u32 {
			0 => (c, x, 0.),
			1 => (x, c, 0.),
			2 => (0., c, x),
			3 => (0., x, c),
			4 => (x, 0., c),
			_ => (c, 0., x),
		};
		[r + m, g + m, b + m]
	}
}

/// The two colors a dynamic theme changes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tint {
	/// The legible signal color: primary action, selection, progress.
	pub accent: Hsb,
	/// The near-black chrome tint with a whisper of the accent hue.
	pub background: Hsb,
	/// Opacity of `background` over the backdrop.
	pub background_opacity: f32,
}

/// Tuning of the extraction. The defaults match `AppConstants.Theme` in the launcher.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Extractor {
	/// The image is sampled on a square grid of this side.
	pub sample_side: usize,
	/// Hue buckets that compete for the dominant hue.
	pub hue_buckets: usize,
	/// Pixels below this saturation count as gray.
	pub minimum_pixel_saturation: f32,
	/// Pixels outside this brightness range count as black or blown out.
	pub pixel_brightness: (f32, f32),
	/// The accent's saturation is clamped into this range to stay legible on dark chrome.
	pub accent_saturation: (f32, f32),
	/// The accent's brightness is clamped into this range.
	pub accent_brightness: (f32, f32),
	/// Saturation, brightness, and opacity of the background tint.
	pub background: (f32, f32, f32),
}

impl Default for Extractor {
	fn default() -> Self {
		Self {
			sample_side: 32,
			hue_buckets: 24,
			minimum_pixel_saturation: 0.15,
			pixel_brightness: (0.08, 0.97),
			accent_saturation: (0.65, 0.85),
			accent_brightness: (0.80, 0.95),
			background: (0.1, 0.1, 0.52),
		}
	}
}

impl Extractor {
	/// The tint of tightly packed 8-bit RGBA pixels, `width` × `height`. Returns `None` when
	/// no pixel has a real hue (gray, near-black, or blank artwork): keep the palette then.
	pub fn extract(&self, rgba: &[u8], width: usize, height: usize) -> Option<Tint> {
		if width == 0 || height == 0 || rgba.len() < width * height * 4 || self.hue_buckets == 0 {
			return None;
		}
		#[derive(Clone, Copy, Default)]
		struct Bucket {
			score: f32,
			hue: f32,
			saturation: f32,
			brightness: f32,
			count: u32,
		}
		let mut buckets = vec![Bucket::default(); self.hue_buckets];
		let side = self.sample_side.max(1);
		for row in 0..side {
			for column in 0..side {
				// Nearest pixel at the center of each grid cell, like drawing into a thumbnail.
				let x = ((column * 2 + 1) * width / (side * 2)).min(width - 1);
				let y = ((row * 2 + 1) * height / (side * 2)).min(height - 1);
				let offset = (y * width + x) * 4;
				let pixel = &rgba[offset..offset + 4];
				if pixel[3] < 128 {
					continue;
				}
				let channel = |value: u8| f32::from(value) / 255.;
				let color = Hsb::from_rgb(channel(pixel[0]), channel(pixel[1]), channel(pixel[2]));
				let (darkest, brightest) = self.pixel_brightness;
				if color.saturation < self.minimum_pixel_saturation
					|| color.brightness < darkest
					|| color.brightness > brightest
				{
					continue;
				}
				let index = ((color.hue * self.hue_buckets as f32) as usize).min(self.hue_buckets - 1);
				let bucket = &mut buckets[index];
				bucket.score += color.saturation * color.brightness;
				bucket.hue += color.hue;
				bucket.saturation += color.saturation;
				bucket.brightness += color.brightness;
				bucket.count += 1;
			}
		}
		let winner = buckets
			.iter()
			.filter(|bucket| bucket.count > 0)
			.max_by(|a, b| a.score.total_cmp(&b.score))?;
		let count = winner.count as f32;
		let clamp = |value: f32, (low, high): (f32, f32)| value.clamp(low, high);
		let hue = winner.hue / count;
		let (saturation, brightness, opacity) = self.background;
		Some(Tint {
			accent: Hsb::new(
				hue,
				clamp(winner.saturation / count, self.accent_saturation),
				clamp(winner.brightness / count, self.accent_brightness),
			),
			background: Hsb::new(hue, saturation, brightness),
			background_opacity: opacity,
		})
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn gray_artwork_keeps_the_palette() {
		let pixels = [128, 128, 128, 255].repeat(64);
		assert_eq!(Extractor::default().extract(&pixels, 8, 8), None);
	}

	#[test]
	fn the_dominant_hue_wins_and_stays_legible() {
		// Three quarters dark red, one quarter pale blue.
		let mut pixels = [90, 10, 10, 255].repeat(48);
		pixels.extend([150, 170, 230, 255].repeat(16));
		let tint = Extractor::default().extract(&pixels, 8, 8).unwrap();
		assert!(tint.accent.hue < 0.05 || tint.accent.hue > 0.95);
		assert!(tint.accent.brightness >= 0.80);
	}

	#[test]
	fn rgb_round_trips() {
		let color = Hsb::from_rgb(0.2, 0.6, 0.9);
		let [r, g, b] = color.to_rgb();
		assert!((r - 0.2).abs() < 1e-5 && (g - 0.6).abs() < 1e-5 && (b - 0.9).abs() < 1e-5);
	}
}
