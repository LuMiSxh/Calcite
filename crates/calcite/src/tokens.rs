// SPDX-License-Identifier: MPL-2.0

//! Design tokens ported from `LauncherVisuals.swift` and `AppConstants.swift`.
//!
//! Colors stay in straight (non-premultiplied) sRGB so that the glass module can
//! composite the macOS 27 layer stack into one fill.

use gpui::{Pixels, Rgba, px};

pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Rgba {
	Rgba::new(r, g, b, a)
}

pub const fn white(a: f32) -> Rgba {
	rgba(1., 1., 1., a)
}

pub const fn black(a: f32) -> Rgba {
	rgba(0., 0., 0., a)
}

pub const CLEAR: Rgba = rgba(0., 0., 0., 0.);

/// Fallback accent. Use it only when no artwork or theme accent exists.
pub const CYAN: Rgba = rgba(0.094, 0.82, 1., 1.);
pub const CONTROL_TINT: Rgba = rgba(0.72, 0.74, 0.77, 1.);
/// macOS 27 replaces near-neutral border tints with this quieter neutral.
pub const QUIET_CONTROL_TINT: Rgba = rgba(0.58, 0.61, 0.65, 1.);
pub const HAIRLINE: Rgba = white(0.12);
pub const DANGER: Rgba = rgba(0.69, 0.141, 0.231, 1.);
pub const DANGER_FOREGROUND: Rgba = rgba(0.98, 0.42, 0.48, 1.);
pub const WARNING: Rgba = rgba(0.91, 0.61, 0.21, 1.);
pub const SUCCESS: Rgba = rgba(0.28, 0.78, 0.55, 1.);
pub const ARTWORK_FALLBACK: Rgba = rgba(0.035, 0.04, 0.045, 1.);
pub const SECONDARY_TEXT: Rgba = white(0.55);
pub const PRIMARY_TEXT: Rgba = white(0.92);

pub mod radius {
	use super::*;
	pub const CONTROL: Pixels = px(10.);
	pub const ROW: Pixels = px(10.);
	pub const PANEL: Pixels = px(16.);
	/// Fixed radius of an expanded HUD surface.
	pub const HUD_PILL: Pixels = px(22.);
	pub const MODAL: Pixels = px(24.);
}

pub mod control {
	pub const COMPACT_HORIZONTAL_PADDING: f32 = 10.;
	pub const COMPACT_VERTICAL_PADDING: f32 = 5.;
	pub const SEMANTIC_BORDER_OPACITY: f32 = 0.28;
	pub const PANEL_BORDER: gpui::Rgba = super::white(0.08);
	pub const NAVIGATION_HOVER_FILL: gpui::Rgba = super::white(0.06);
	pub const NAVIGATION_RAIL_BACKGROUND: gpui::Rgba = super::black(0.28);
	pub const MODAL_BACKGROUND: gpui::Rgba = super::rgba(0.07, 0.07, 0.08, 1.);
	pub const OVERLAY_DIM: f32 = 0.55;
	pub const ENABLED_NEUTRAL_SURFACE_OPACITY: f32 = 0.07;
	pub const DISABLED_NEUTRAL_SURFACE_OPACITY: f32 = 0.025;
	pub const ENABLED_NEUTRAL_BORDER_OPACITY: f32 = 0.24;
	pub const DISABLED_NEUTRAL_BORDER_OPACITY: f32 = 0.10;
	pub const ENABLED_CHROMATIC_SURFACE_OPACITY: f32 = 0.14;
	pub const DISABLED_CHROMATIC_SURFACE_OPACITY: f32 = 0.035;
	pub const ENABLED_CHROMATIC_BORDER_OPACITY: f32 = 0.40;
	pub const DISABLED_CHROMATIC_BORDER_OPACITY: f32 = 0.14;
	pub const HUD_BORDER_OPACITY: f32 = 0.65;
	pub const HUD_HOVER_SURFACE_OPACITY: f32 = 0.06;
	pub const DISABLED_FOREGROUND_OPACITY: f32 = 0.44;
	pub const DISABLED_NEUTRAL_FOREGROUND_OPACITY: f32 = 0.36;
	pub const OPAQUE_NEUTRAL_SURFACE: gpui::Rgba = super::rgba(0.13, 0.14, 0.16, 1.);
	pub const OPAQUE_DISABLED_SURFACE: gpui::Rgba = super::rgba(0.094, 0.098, 0.114, 1.);
}

/// Point sizes of the macOS text styles the launcher uses.
pub mod text {
	use super::*;
	pub const CAPTION: Pixels = px(10.);
	pub const BODY: Pixels = px(13.);
	pub const STATUS_TITLE: Pixels = px(14.);
	pub const STATUS_PERCENT: Pixels = px(16.);
	pub const TITLE: Pixels = px(22.);
}

pub mod hud {
	use super::*;
	pub const PILL_HEIGHT: Pixels = px(28.);
	pub const EXPANDED_STATUS_WIDTH: Pixels = px(360.);
	pub const EXPANDED_VERSION_WIDTH: Pixels = px(260.);
	pub const EXPANDED_MUSIC_WIDTH: Pixels = px(330.);
	pub const HORIZONTAL_PADDING: Pixels = px(11.);
}

/// Linear mix of two colors, `t = 0` gives `from`.
pub fn mix(from: Rgba, to: Rgba, t: f32) -> Rgba {
	let t = t.clamp(0., 1.);
	let lerp = |a: f32, b: f32| a + (b - a) * t;
	rgba(
		lerp(from.red, to.red),
		lerp(from.green, to.green),
		lerp(from.blue, to.blue),
		lerp(from.alpha, to.alpha),
	)
}

pub fn with_alpha(color: Rgba, a: f32) -> Rgba {
	rgba(color.red, color.green, color.blue, a)
}

pub fn scale_alpha(color: Rgba, factor: f32) -> Rgba {
	with_alpha(color, color.alpha * factor)
}

pub fn chroma(color: Rgba) -> f32 {
	color.red.max(color.green).max(color.blue) - color.red.min(color.green).min(color.blue)
}

/// The launcher treats a tint as chromatic above this chroma. Below it, a tint reads as
/// neutral (for example the near-black HUD tint) and must not turn a surface blue.
pub fn is_chromatic(color: Rgba) -> bool {
	chroma(color) > 0.08
}

/// Port of `LauncherVisuals.contrastTint`: macOS 27 glass blends a tint into the surface,
/// so a chromatic foreground mixes 28 % white to stay readable with the same hue.
pub fn contrast_tint(color: Rgba) -> Rgba {
	if !is_chromatic(color) {
		return color;
	}
	let mix = 0.28;
	rgba(
		color.red + (1. - color.red) * mix,
		color.green + (1. - color.green) * mix,
		color.blue + (1. - color.blue) * mix,
		color.alpha,
	)
}

/// Source-over composite of `top` onto `bottom`, both straight alpha.
pub fn over(top: Rgba, bottom: Rgba) -> Rgba {
	let a = top.alpha + bottom.alpha * (1. - top.alpha);
	if a <= f32::EPSILON {
		return CLEAR;
	}
	let channel = |t: f32, b: f32| (t * top.alpha + b * bottom.alpha * (1. - top.alpha)) / a;
	rgba(
		channel(top.red, bottom.red),
		channel(top.green, bottom.green),
		channel(top.blue, bottom.blue),
		a,
	)
}

/// Composites layers listed from bottom to top.
pub fn stack(layers: &[Rgba]) -> Rgba {
	layers.iter().fold(CLEAR, |acc, layer| over(*layer, acc))
}

/// HSB to RGB, matching SwiftUI `Color(hue:saturation:brightness:)`.
pub fn hsb(hue: f32, saturation: f32, brightness: f32, alpha: f32) -> Rgba {
	let h = (hue.rem_euclid(1.)) * 6.;
	let c = brightness * saturation;
	let x = c * (1. - ((h % 2.) - 1.).abs());
	let m = brightness - c;
	let (r, g, b) = match h as u32 {
		0 => (c, x, 0.),
		1 => (x, c, 0.),
		2 => (0., c, x),
		3 => (0., x, c),
		4 => (x, 0., c),
		_ => (c, 0., x),
	};
	rgba(r + m, g + m, b + m, alpha)
}

/// RGB to hue in 0..1, for deriving the HUD tint from an accent.
pub fn hue(color: Rgba) -> f32 {
	let max = color.red.max(color.green).max(color.blue);
	let min = color.red.min(color.green).min(color.blue);
	let delta = max - min;
	if delta <= f32::EPSILON {
		return 0.;
	}
	let h = if max == color.red {
		((color.green - color.blue) / delta).rem_euclid(6.)
	} else if max == color.green {
		(color.blue - color.red) / delta + 2.
	} else {
		(color.red - color.green) / delta + 4.
	};
	h / 6.
}

/// Port of `ExtractedAccent.backgroundTint`: the HUD glass tint stays near black and takes
/// a whisper of the accent hue.
pub fn hud_tint_for(accent: Option<Rgba>) -> Rgba {
	match accent {
		Some(accent) if is_chromatic(accent) => hsb(hue(accent), 0.1, 0.1, 0.52),
		_ => black(0.52),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn neutral_backplate_stack_matches_hand_composite() {
		let fill = stack(&[white(0.06), black(0.68)]);
		assert!((fill.alpha - 0.6992).abs() < 1e-4);
		assert!((fill.red - 0.0192 / 0.6992).abs() < 1e-4);
	}

	#[test]
	fn hud_tint_is_not_chromatic() {
		assert!(!is_chromatic(hud_tint_for(Some(CYAN))));
	}
}
