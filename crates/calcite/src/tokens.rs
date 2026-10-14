// SPDX-License-Identifier: MPL-2.0

//! Design tokens ported from `LauncherVisuals.swift` and `AppConstants.swift`: radii,
//! opacities, type sizes, and color math. Colors live in [`Palette`](crate::palette::Palette).
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

/// Spacing scale (`LauncherVisuals.Spacing`). Use it for gaps and padding instead of raw
/// pixel values.
pub mod space {
	use super::*;
	pub const HAIRLINE: Pixels = px(1.);
	pub const COMPACT: Pixels = px(4.);
	pub const TIGHT: Pixels = px(6.);
	pub const CONTROL: Pixels = px(8.);
	pub const CONTENT: Pixels = px(12.);
	pub const PANEL: Pixels = px(16.);
	pub const SECTION: Pixels = px(20.);
	pub const PAGE: Pixels = px(26.);
}

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
	use super::*;

	pub const COMPACT_HORIZONTAL_PADDING: f32 = 10.;
	pub const COMPACT_VERTICAL_PADDING: f32 = 5.;
	/// Heights of the three control sizes. A button, a text field, and a menu chip of one size
	/// stand level side by side. They are the line height of the size's text (phi times 13, 13,
	/// and 10 points) plus its vertical padding, rounded to whole points.
	pub const HEIGHT_COMPACT: Pixels = px(26.);
	pub const HEIGHT_REGULAR: Pixels = px(33.);
	pub const HEIGHT_LARGE: Pixels = px(37.);
	/// Padding of a small label capsule: a badge or a tag.
	pub const BADGE_HORIZONTAL_PADDING: f32 = 7.;
	pub const BADGE_VERTICAL_PADDING: f32 = 2.;
	/// Gap between the parts of a compact capsule (chip, menu picker).
	pub const CHIP_GAP: f32 = 6.;
	pub const SEMANTIC_BORDER_OPACITY: f32 = 0.28;
	/// The dim behind a modal, on a dark palette; [`Theme::scrim`](crate::theme::Theme::scrim)
	/// softens it on a light one.
	pub const OVERLAY_DIM: f32 = 0.30;
	pub const ENABLED_NEUTRAL_SURFACE_OPACITY: f32 = 0.07;
	pub const DISABLED_NEUTRAL_SURFACE_OPACITY: f32 = 0.025;
	pub const ENABLED_NEUTRAL_BORDER_OPACITY: f32 = 0.24;
	pub const DISABLED_NEUTRAL_BORDER_OPACITY: f32 = 0.10;
	pub const ENABLED_CHROMATIC_SURFACE_OPACITY: f32 = 0.14;
	pub const DISABLED_CHROMATIC_SURFACE_OPACITY: f32 = 0.035;
	pub const ENABLED_CHROMATIC_BORDER_OPACITY: f32 = 0.40;
	pub const DISABLED_CHROMATIC_BORDER_OPACITY: f32 = 0.14;
	pub const HUD_BORDER_OPACITY: f32 = 0.65;
	/// The one hover strength: a white overlay on neutral surfaces, the accent on tinted ones.
	pub const HOVER_OPACITY: f32 = 0.06;
	/// A control's opacity while it is pressed (`ActionPressStyle`).
	pub const PRESSED_OPACITY: f32 = 0.86;
	/// The accent fill of a selected row.
	pub const SELECTED_ROW_FILL_OPACITY: f32 = 0.13;
	pub const DISABLED_FOREGROUND_OPACITY: f32 = 0.44;
	pub const DISABLED_NEUTRAL_FOREGROUND_OPACITY: f32 = 0.36;
}

/// Point sizes of the macOS text styles the launcher uses.
pub mod text {
	use super::*;
	pub const CAPTION: Pixels = px(10.);
	/// Tooltips.
	pub const FOOTNOTE: Pixels = px(11.);
	/// Details under a title and menu rows.
	pub const SUPPORTING: Pixels = px(12.);
	pub const BODY: Pixels = px(13.);
	/// Navigation rail entries.
	pub const NAVIGATION: Pixels = px(14.);
	pub const STATUS_TITLE: Pixels = px(14.);
	pub const STATUS_PERCENT: Pixels = px(16.);
	pub const TITLE: Pixels = px(22.);
	/// Page titles in settings and the setup assistant.
	pub const PAGE_TITLE: Pixels = px(26.);
	pub const COMPLETION_LABEL: Pixels = px(15.);
	pub const HUD_GLYPH: Pixels = px(13.);
	pub const HUD_PROMINENT_GLYPH: Pixels = px(16.);
	pub const MENU_CHEVRON: Pixels = px(9.);
	pub const TAG_GLYPH: Pixels = px(10.);
}

pub mod hud {
	use super::*;
	pub const PILL_HEIGHT: Pixels = px(28.);
	pub const EXPANDED_STATUS_WIDTH: Pixels = px(360.);
	pub const EXPANDED_VERSION_WIDTH: Pixels = px(260.);
	pub const EXPANDED_MUSIC_WIDTH: Pixels = px(330.);
	pub const HORIZONTAL_PADDING: Pixels = px(11.);
}

/// The fill and border of a surface tinted with a chromatic color: the selected state of a
/// card, chip, or segment, and the accent capsule.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Chromatic {
	pub fill: Rgba,
	pub border: Rgba,
}

impl Chromatic {
	/// The enabled look: a 14 % fill and a 40 % border in `tint`.
	pub fn enabled(tint: Rgba) -> Self {
		Self {
			fill: with_alpha(tint, control::ENABLED_CHROMATIC_SURFACE_OPACITY),
			border: with_alpha(tint, control::ENABLED_CHROMATIC_BORDER_OPACITY),
		}
	}

	/// The disabled look on a dark palette: a faint white fill and a 14 % border in `tint`. Use
	/// [`Chromatic::disabled_with`] with the palette's ink on a light one.
	pub fn disabled(tint: Rgba) -> Self {
		Self::disabled_with(tint, white(1.))
	}

	/// The disabled look: a faint fill in `ink` (the palette's neutral) and a 14 % border in `tint`.
	pub fn disabled_with(tint: Rgba, ink: Rgba) -> Self {
		Self {
			fill: with_alpha(ink, control::DISABLED_CHROMATIC_SURFACE_OPACITY),
			border: with_alpha(tint, control::DISABLED_CHROMATIC_BORDER_OPACITY),
		}
	}
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

/// WCAG relative luminance of an sRGB color; alpha is ignored.
pub fn luminance(color: Rgba) -> f32 {
	let linear = |c: f32| {
		if c <= 0.04045 {
			c / 12.92
		} else {
			((c + 0.055) / 1.055).powf(2.4)
		}
	};
	0.2126 * linear(color.red) + 0.7152 * linear(color.green) + 0.0722 * linear(color.blue)
}

/// WCAG contrast ratio of two colors (alpha ignored), 1 to 21.
pub fn contrast_ratio(a: Rgba, b: Rgba) -> f32 {
	let (la, lb) = (luminance(a), luminance(b));
	(la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// The label color for text drawn on a solid `fill`: white where it reads at 3:1 or better
/// (large, semibold text), otherwise near-black.
pub fn on_color(fill: Rgba) -> Rgba {
	if contrast_ratio(white(1.), fill) >= ON_COLOR_WHITE_MINIMUM {
		white(1.)
	} else {
		black(0.88)
	}
}

/// The least contrast white text needs on a solid fill to stay white.
const ON_COLOR_WHITE_MINIMUM: f32 = 3.;

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

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn control_heights_step_up_with_the_size() {
		assert!(control::HEIGHT_COMPACT < control::HEIGHT_REGULAR);
		assert!(control::HEIGHT_REGULAR < control::HEIGHT_LARGE);
		// Regular text with 6 pt padding and compact text with 5 pt padding, at line height phi.
		let phi = 1.62_f32;
		let regular = f32::from(text::BODY) * phi + 12.;
		let compact = f32::from(text::CAPTION) * phi + 2. * control::COMPACT_VERTICAL_PADDING;
		let large = f32::from(text::BODY) * phi + 16.;
		assert!((f32::from(control::HEIGHT_REGULAR) - regular).abs() < 0.5);
		assert!((f32::from(control::HEIGHT_COMPACT) - compact).abs() < 0.5);
		assert!((f32::from(control::HEIGHT_LARGE) - large).abs() < 0.5);
	}

	#[test]
	fn hover_and_press_tokens_are_subtle() {
		const { assert!(control::HOVER_OPACITY > 0. && control::HOVER_OPACITY < 0.1) };
		const { assert!(control::PRESSED_OPACITY > 0.8 && control::PRESSED_OPACITY < 1.) };
	}

	#[test]
	fn chromatic_surface_uses_the_enabled_and_disabled_endpoints() {
		let tint = rgba(0.2, 0.4, 0.9, 1.);
		let on = Chromatic::enabled(tint);
		assert_eq!(
			on.fill,
			with_alpha(tint, control::ENABLED_CHROMATIC_SURFACE_OPACITY)
		);
		assert_eq!(
			on.border,
			with_alpha(tint, control::ENABLED_CHROMATIC_BORDER_OPACITY)
		);
		let off = Chromatic::disabled(tint);
		assert_eq!(off.fill, white(control::DISABLED_CHROMATIC_SURFACE_OPACITY));
		assert_eq!(
			off.border,
			with_alpha(tint, control::DISABLED_CHROMATIC_BORDER_OPACITY)
		);
		assert!(on.fill.alpha > off.fill.alpha && on.border.alpha > off.border.alpha);
	}

	#[test]
	fn neutral_backplate_stack_matches_hand_composite() {
		let fill = stack(&[white(0.06), black(0.68)]);
		assert!((fill.alpha - 0.6992).abs() < 1e-4);
		assert!((fill.red - 0.0192 / 0.6992).abs() < 1e-4);
	}
}
