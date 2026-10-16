// SPDX-License-Identifier: MPL-2.0

//! Dynamic theme colors from artwork. Port of the launcher's `WallpaperColorExtractor`.
//!
//! The crate finds the dominant hue of an image and renders it two ways: a legible accent
//! for controls and text, and the same hue, desaturated and darkened, for tinting neutral
//! chrome. A dynamic theme changes only these two values; the rest of the palette stays.
//!
//! Pixels come in through [`ImageView`], which validates the buffer and accepts padded rows
//! and BGRA order. The image is averaged over a fixed grid of cells, so the result follows the
//! picture, not its pixel count.
//!
//! It has no dependencies and no GPU or UI code, so it runs on any thread.
//!
//! ```
//! use calcite_tint::{Extractor, ImageView};
//!
//! // Four RGBA pixels of a teal image.
//! let pixels = [20, 140, 150, 255].repeat(4);
//! let image = ImageView::packed_rgba(2, 2, &pixels).expect("the buffer holds 2x2 pixels");
//! let tint = Extractor::default().extract(image).expect("the image has a hue");
//! assert!((tint.accent.hue() - 0.51).abs() < 0.02);
//! let [red, green, blue] = tint.accent.to_rgb();
//! # let _ = (red, green, blue);
//! ```

use std::f32::consts::TAU;
use std::ops::Range;

/// A color as hue, saturation, and brightness (SwiftUI's `Color(hue:...)`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hsb {
	hue: f32,
	saturation: f32,
	brightness: f32,
}

impl Hsb {
	/// Hue wraps around the circle; saturation and brightness clamp to 0..=1. Non-finite
	/// values become 0.
	pub fn new(hue: f32, saturation: f32, brightness: f32) -> Self {
		Self {
			hue: if hue.is_finite() { hue.rem_euclid(1.) } else { 0. },
			saturation: unit(saturation),
			brightness: unit(brightness),
		}
	}

	/// Hue in 0..1, where 0 and 1 are the same red.
	pub const fn hue(self) -> f32 {
		self.hue
	}

	/// Saturation in 0..=1.
	pub const fn saturation(self) -> f32 {
		self.saturation
	}

	/// Brightness in 0..=1.
	pub const fn brightness(self) -> f32 {
		self.brightness
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

/// A closed interval for the 0..=1 values that gate and clamp the extraction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
	low: f32,
	high: f32,
}

impl Bounds {
	/// The interval between two bounds, in either order.
	pub fn new(first: f32, second: f32) -> Self {
		Self {
			low: first.min(second),
			high: first.max(second),
		}
	}

	/// The lower bound.
	pub const fn low(self) -> f32 {
		self.low
	}

	/// The upper bound.
	pub const fn high(self) -> f32 {
		self.high
	}

	fn contains(self, value: f32) -> bool {
		(self.low..=self.high).contains(&value)
	}

	fn clamp(self, value: f32) -> f32 {
		value.max(self.low).min(self.high)
	}
}

/// The background tint: near-black chrome with a whisper of the accent hue.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BackgroundTint {
	/// Saturation, in 0..=1.
	pub saturation: f32,
	/// Brightness, in 0..=1.
	pub brightness: f32,
	/// Opacity of the tint over the backdrop, in 0..=1.
	pub opacity: f32,
}

/// The byte order of one 8-bit pixel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelFormat {
	/// Red, green, blue, alpha.
	Rgba8,
	/// Blue, green, red, alpha.
	Bgra8,
}

/// Why an [`ImageView`] does not describe its buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageError {
	/// The width or height is zero.
	Empty,
	/// The stride cannot hold one row of pixels.
	StrideTooSmall { stride: usize, minimum: usize },
	/// The buffer ends before the last pixel it must hold.
	BufferTooShort { length: usize, required: usize },
	/// The dimensions do not fit in `usize` byte counts.
	TooLarge,
}

impl std::fmt::Display for ImageError {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Empty => formatter.write_str("the image has no pixels"),
			Self::StrideTooSmall { stride, minimum } => {
				write!(formatter, "stride {stride} cannot hold a {minimum}-byte row")
			}
			Self::BufferTooShort { length, required } => {
				write!(formatter, "buffer has {length} bytes, needs {required}")
			}
			Self::TooLarge => formatter.write_str("the image size overflows"),
		}
	}
}

impl std::error::Error for ImageError {}

/// A borrowed 8-bit image: `height` rows of `width` pixels, each row `stride` bytes after the last.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImageView<'a> {
	width: usize,
	height: usize,
	stride: usize,
	format: PixelFormat,
	pixels: &'a [u8],
}

impl<'a> ImageView<'a> {
	/// Checks the buffer, then borrows it. Rows may hold padding after their pixels.
	pub fn new(
		width: usize,
		height: usize,
		stride: usize,
		format: PixelFormat,
		pixels: &'a [u8],
	) -> Result<Self, ImageError> {
		if width == 0 || height == 0 {
			return Err(ImageError::Empty);
		}
		let row = width.checked_mul(4).ok_or(ImageError::TooLarge)?;
		if stride < row {
			return Err(ImageError::StrideTooSmall { stride, minimum: row });
		}
		let required = (height - 1)
			.checked_mul(stride)
			.and_then(|bytes| bytes.checked_add(row))
			.ok_or(ImageError::TooLarge)?;
		if pixels.len() < required {
			return Err(ImageError::BufferTooShort {
				length: pixels.len(),
				required,
			});
		}
		Ok(Self {
			width,
			height,
			stride,
			format,
			pixels,
		})
	}

	/// Tightly packed RGBA rows: each row is `width * 4` bytes.
	pub fn packed_rgba(width: usize, height: usize, pixels: &'a [u8]) -> Result<Self, ImageError> {
		let stride = width.checked_mul(4).ok_or(ImageError::TooLarge)?;
		Self::new(width, height, stride, PixelFormat::Rgba8, pixels)
	}
}

/// Hue buckets that compete for the dominant hue. Fixed, so the buckets live on the stack.
const HUE_BUCKETS: usize = 24;

/// Pixels with less alpha than this are transparent and do not count.
const OPAQUE_ALPHA: u8 = 128;

/// Tuning of the extraction. The defaults match `AppConstants.Theme` in the launcher.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Extractor {
	/// The image is averaged over a square grid of this side.
	pub sample_side: usize,
	/// Pixels below this saturation count as gray.
	pub minimum_pixel_saturation: f32,
	/// Pixels outside this brightness band count as black or blown out.
	pub pixel_brightness: Bounds,
	/// The accent's saturation is clamped into this band to stay legible on dark chrome.
	pub accent_saturation: Bounds,
	/// The accent's brightness is clamped into this band.
	pub accent_brightness: Bounds,
	/// The background tint.
	pub background: BackgroundTint,
}

impl Default for Extractor {
	fn default() -> Self {
		Self {
			sample_side: 32,
			minimum_pixel_saturation: 0.15,
			pixel_brightness: Bounds::new(0.08, 0.97),
			accent_saturation: Bounds::new(0.65, 0.85),
			accent_brightness: Bounds::new(0.80, 0.95),
			background: BackgroundTint {
				saturation: 0.1,
				brightness: 0.1,
				opacity: 0.52,
			},
		}
	}
}

impl Extractor {
	/// The tint of `image`. Returns `None` when no pixel has a real hue (gray, near-black,
	/// blank, or fully transparent artwork): keep the palette then.
	///
	/// Each grid cell averages the opaque pixels that pass the saturation and brightness gates.
	/// Each cell mean votes for a hue bucket. The winning bucket's hue is the circular mean of
	/// its votes, so reds at hue 0 and hue 1 count as one hue.
	pub fn extract(&self, image: ImageView<'_>) -> Option<Tint> {
		let side = self.sample_side.max(1);
		let mut buckets = [Bucket::default(); HUE_BUCKETS];
		for row in 0..side {
			let rows = cell_range(row, side, image.height);
			for column in 0..side {
				let columns = cell_range(column, side, image.width);
				if let Some((mean, weight)) = self.cell_mean(image, columns, rows.clone()) {
					buckets[bucket_of(mean.hue())].add(mean, weight);
				}
			}
		}
		let winner = buckets
			.iter()
			.filter(|bucket| bucket.weight > 0.)
			.max_by(|a, b| a.score.total_cmp(&b.score))?;
		let hue = winner.hue();
		let saturation = winner.saturation / winner.weight;
		let brightness = winner.brightness / winner.weight;
		let background = self.background;
		Some(Tint {
			accent: Hsb::new(
				hue,
				self.accent_saturation.clamp(saturation),
				self.accent_brightness.clamp(brightness),
			),
			background: Hsb::new(hue, background.saturation, background.brightness),
			background_opacity: unit(background.opacity),
		})
	}

	/// The mean color of a cell's pixels that pass the gates, and the share of the cell's
	/// opaque pixels that pass. Transparent pixels are not part of the cell.
	fn cell_mean(
		&self,
		image: ImageView<'_>,
		columns: Range<usize>,
		rows: Range<usize>,
	) -> Option<(Hsb, f32)> {
		let mut opaque = 0_u32;
		let mut kept = 0_u32;
		let mut sum = [0_f32; 3];
		for y in rows {
			for x in columns.clone() {
				let offset = y * image.stride + x * 4;
				let pixel = &image.pixels[offset..offset + 4];
				if pixel[3] < OPAQUE_ALPHA {
					continue;
				}
				opaque += 1;
				let (red, green, blue) = match image.format {
					PixelFormat::Rgba8 => (pixel[0], pixel[1], pixel[2]),
					PixelFormat::Bgra8 => (pixel[2], pixel[1], pixel[0]),
				};
				let [red, green, blue] = [red, green, blue].map(unit_byte);
				let color = Hsb::from_rgb(red, green, blue);
				if color.saturation() < self.minimum_pixel_saturation
					|| !self.pixel_brightness.contains(color.brightness())
				{
					continue;
				}
				kept += 1;
				for (total, channel) in sum.iter_mut().zip([red, green, blue]) {
					*total += channel;
				}
			}
		}
		if kept == 0 {
			return None;
		}
		let count = kept as f32;
		let [red, green, blue] = sum.map(|total| total / count);
		Some((Hsb::from_rgb(red, green, blue), count / opaque as f32))
	}
}

/// A hue bucket's accumulated votes.
#[derive(Clone, Copy, Default)]
struct Bucket {
	/// Total vote weight.
	weight: f32,
	/// Weight times saturation times brightness: how strongly the votes stand out.
	score: f32,
	/// Weighted sums of the hues' unit vectors, for the circular mean.
	cosine: f32,
	sine: f32,
	saturation: f32,
	brightness: f32,
}

impl Bucket {
	fn add(&mut self, color: Hsb, weight: f32) {
		let angle = color.hue() * TAU;
		self.weight += weight;
		self.score += weight * color.saturation() * color.brightness();
		self.cosine += weight * angle.cos();
		self.sine += weight * angle.sin();
		self.saturation += weight * color.saturation();
		self.brightness += weight * color.brightness();
	}

	/// The circular mean of the votes' hues, in 0..1.
	fn hue(&self) -> f32 {
		(self.sine.atan2(self.cosine) / TAU).rem_euclid(1.)
	}
}

/// The bucket of a hue. Bucket `k` covers hues within half a bucket of `k / HUE_BUCKETS`, so
/// hue 0 and hue 1 share bucket 0 instead of splitting red across two ends.
fn bucket_of(hue: f32) -> usize {
	((hue * HUE_BUCKETS as f32 + 0.5) as usize) % HUE_BUCKETS
}

/// The pixels of cell `index` when an axis of `length` pixels splits into `side` cells. Every
/// cell covers at least one pixel, so small images still fill the whole grid.
fn cell_range(index: usize, side: usize, length: usize) -> Range<usize> {
	let start = index * length / side;
	let end = ((index + 1) * length / side).max(start + 1);
	start..end
}

/// A byte as a 0..=1 channel.
fn unit_byte(value: u8) -> f32 {
	f32::from(value) / 255.
}

/// Clamps to 0..=1. Non-finite values become 0.
fn unit(value: f32) -> f32 {
	if value.is_finite() {
		value.clamp(0., 1.)
	} else {
		0.
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn tint_of(pixels: &[u8], width: usize, height: usize) -> Option<Tint> {
		let image = ImageView::packed_rgba(width, height, pixels).expect("valid test image");
		Extractor::default().extract(image)
	}

	/// The distance between two hues, going the short way around the circle.
	fn hue_distance(a: f32, b: f32) -> f32 {
		let difference = (a - b).rem_euclid(1.);
		difference.min(1. - difference)
	}

	#[test]
	fn gray_artwork_keeps_the_palette() {
		let pixels = [128, 128, 128, 255].repeat(64);
		assert_eq!(tint_of(&pixels, 8, 8), None);
	}

	#[test]
	fn the_dominant_hue_wins_and_stays_legible() {
		// Three quarters dark red, one quarter pale blue.
		let mut pixels = [90, 10, 10, 255].repeat(48);
		pixels.extend([150, 170, 230, 255].repeat(16));
		let tint = tint_of(&pixels, 8, 8).unwrap();
		assert!(hue_distance(tint.accent.hue(), 0.) < 0.05);
		assert!(tint.accent.brightness() >= 0.80);
	}

	#[test]
	fn red_split_across_hue_zero_and_one_beats_a_smaller_blue() {
		// Twenty pixels each at hues 0.02 and 0.98 (both red), and 24 blue. Linear bins split
		// the red into two buckets, and the blue bucket wins.
		let mut pixels = [200, 40, 20, 255].repeat(20);
		pixels.extend([200, 20, 40, 255].repeat(20));
		pixels.extend([40, 80, 220, 255].repeat(24));
		let tint = tint_of(&pixels, 8, 8).unwrap();
		assert!(hue_distance(tint.accent.hue(), 0.) < 0.02);
	}

	#[test]
	fn padded_rows_match_packed_rows() {
		let (width, height) = (4, 3);
		let packed: Vec<u8> = (0..width * height)
			.flat_map(|index| [(index * 20) as u8, 60, 200, 255])
			.collect();
		let expected = tint_of(&packed, width, height);
		assert!(expected.is_some());
		for padding in [1, 8] {
			let stride = width * 4 + padding;
			let mut padded = vec![0xEE; stride * height];
			for row in 0..height {
				let source = &packed[row * width * 4..(row + 1) * width * 4];
				padded[row * stride..row * stride + width * 4].copy_from_slice(source);
			}
			let image = ImageView::new(width, height, stride, PixelFormat::Rgba8, &padded).unwrap();
			assert_eq!(Extractor::default().extract(image), expected, "padding {padding}");
		}
	}

	#[test]
	fn the_tint_is_stable_across_image_sizes() {
		let side = 40_usize;
		let red: [u8; 4] = [200, 40, 20, 255];
		let blue: [u8; 4] = [40, 80, 220, 255];
		let base: Vec<u8> = (0..side * side)
			.flat_map(|index| if index % side < 26 { red } else { blue })
			.collect();
		let mut doubled = Vec::new();
		for y in 0..2 * side {
			for x in 0..2 * side {
				let source = (y / 2) * side + x / 2;
				doubled.extend(&base[source * 4..source * 4 + 4]);
			}
		}
		let small = tint_of(&base, side, side).unwrap();
		let large = tint_of(&doubled, 2 * side, 2 * side).unwrap();
		assert!(hue_distance(small.accent.hue(), large.accent.hue()) < 0.01);
		assert!((small.accent.saturation() - large.accent.saturation()).abs() < 0.01);
		assert!((small.accent.brightness() - large.accent.brightness()).abs() < 0.01);
	}

	#[test]
	fn transparent_pixels_do_not_vote() {
		// 48 transparent blue pixels outnumber 16 opaque red ones.
		let mut pixels = [40, 80, 220, 0].repeat(48);
		pixels.extend([200, 40, 20, 255].repeat(16));
		let tint = tint_of(&pixels, 8, 8).unwrap();
		assert!(hue_distance(tint.accent.hue(), 0.) < 0.05);
		assert_eq!(tint_of(&[40, 80, 220, 0].repeat(64), 8, 8), None);
	}

	#[test]
	fn malformed_buffers_are_rejected_without_panicking() {
		let pixels = [0_u8; 16];
		let cases: [(usize, usize, usize, ImageError); 3] = [
			(0, 2, 8, ImageError::Empty),
			(
				2,
				2,
				7,
				ImageError::StrideTooSmall {
					stride: 7,
					minimum: 8,
				},
			),
			(
				2,
				3,
				8,
				ImageError::BufferTooShort {
					length: 16,
					required: 24,
				},
			),
		];
		for (width, height, stride, expected) in cases {
			let view = ImageView::new(width, height, stride, PixelFormat::Rgba8, &pixels);
			assert_eq!(view.err(), Some(expected));
		}
	}

	#[test]
	fn hsb_wraps_hue_and_clamps_the_rest() {
		let color = Hsb::new(1.25, 2., -1.);
		assert_eq!(
			(color.hue(), color.saturation(), color.brightness()),
			(0.25, 1., 0.)
		);
	}

	#[test]
	fn rgb_round_trips() {
		let color = Hsb::from_rgb(0.2, 0.6, 0.9);
		let [r, g, b] = color.to_rgb();
		assert!((r - 0.2).abs() < 1e-5 && (g - 0.6).abs() < 1e-5 && (b - 0.9).abs() < 1e-5);
	}
}
