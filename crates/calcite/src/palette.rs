// SPDX-License-Identifier: MPL-2.0

//! Every color Calcite draws with, in one value you can replace.
//!
//! Pick a palette at compile time with a `const`, and change it at runtime through the theme:
//!
//! ```ignore
//! const EMBER: Palette = Palette { accent: rgba(0.96, 0.55, 0.22, 1.), ..Palette::ARKNIGHTS };
//!
//! calcite::init_with(cx, Theme::new(EMBER));
//! Theme::update(cx, |theme| *theme = theme.with_palette(Palette::ARKNIGHTS));
//! ```
//!
//! Colors are straight (non-premultiplied) sRGB.

use gpui::Rgba;

use crate::tokens::{black, hsb, hue, is_chromatic, mix, rgba, white};

/// The colors of one look. Build it with struct update syntax from [`Palette::ARKNIGHTS`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
	/// The accent when the app sets no dynamic accent: primary action, selection, progress.
	pub accent: Rgba,
	/// Neutral control tint: secondary actions, unselected switches.
	pub control: Rgba,
	/// The quieter neutral that macOS 27 uses for near-neutral borders.
	pub quiet_control: Rgba,
	/// Separators and HUD edges.
	pub hairline: Rgba,
	pub danger: Rgba,
	/// Danger text and icons, lighter than `danger` for contrast on dark surfaces.
	pub danger_text: Rgba,
	pub warning: Rgba,
	pub success: Rgba,
	pub primary_text: Rgba,
	pub secondary_text: Rgba,
	/// The backdrop behind everything when no artwork is loaded.
	pub backdrop: Rgba,
	/// Settings panels and modals.
	pub panel_background: Rgba,
	pub panel_border: Rgba,
	pub navigation_rail: Rgba,
	pub navigation_hover: Rgba,
	/// Opaque control fills under Reduce Transparency.
	pub opaque_surface: Rgba,
	pub opaque_disabled_surface: Rgba,
	/// The HUD glass tint when the accent has no hue.
	pub hud_tint: Rgba,
	/// How the HUD tint picks up the accent hue: saturation, brightness, opacity.
	pub hud_hue_tint: (f32, f32, f32),
}

impl Palette {
	/// The Arknights Client look: dark chrome and the Arknights cyan.
	pub const ARKNIGHTS: Self = Self {
		accent: rgba(0.094, 0.82, 1., 1.),
		control: rgba(0.72, 0.74, 0.77, 1.),
		quiet_control: rgba(0.58, 0.61, 0.65, 1.),
		hairline: white(0.12),
		danger: rgba(0.69, 0.141, 0.231, 1.),
		danger_text: rgba(0.98, 0.42, 0.48, 1.),
		warning: rgba(0.91, 0.61, 0.21, 1.),
		success: rgba(0.28, 0.78, 0.55, 1.),
		primary_text: white(0.92),
		secondary_text: white(0.55),
		backdrop: rgba(0.035, 0.04, 0.045, 1.),
		panel_background: rgba(0.07, 0.07, 0.08, 1.),
		panel_border: white(0.08),
		navigation_rail: black(0.28),
		navigation_hover: white(0.06),
		opaque_surface: rgba(0.13, 0.14, 0.16, 1.),
		opaque_disabled_surface: rgba(0.094, 0.098, 0.114, 1.),
		hud_tint: black(0.52),
		hud_hue_tint: (0.1, 0.1, 0.52),
	};

	/// This palette with another fallback accent.
	pub const fn with_accent(mut self, accent: Rgba) -> Self {
		self.accent = accent;
		self
	}

	/// Each color mixed from `self` toward `to`, for animated palette changes.
	pub fn mix(&self, to: &Self, t: f32) -> Self {
		let m = |from: Rgba, to: Rgba| mix(from, to, t);
		let (s0, b0, o0) = self.hud_hue_tint;
		let (s1, b1, o1) = to.hud_hue_tint;
		let lerp = |a: f32, b: f32| a + (b - a) * t.clamp(0., 1.);
		Self {
			accent: m(self.accent, to.accent),
			control: m(self.control, to.control),
			quiet_control: m(self.quiet_control, to.quiet_control),
			hairline: m(self.hairline, to.hairline),
			danger: m(self.danger, to.danger),
			danger_text: m(self.danger_text, to.danger_text),
			warning: m(self.warning, to.warning),
			success: m(self.success, to.success),
			primary_text: m(self.primary_text, to.primary_text),
			secondary_text: m(self.secondary_text, to.secondary_text),
			backdrop: m(self.backdrop, to.backdrop),
			panel_background: m(self.panel_background, to.panel_background),
			panel_border: m(self.panel_border, to.panel_border),
			navigation_rail: m(self.navigation_rail, to.navigation_rail),
			navigation_hover: m(self.navigation_hover, to.navigation_hover),
			opaque_surface: m(self.opaque_surface, to.opaque_surface),
			opaque_disabled_surface: m(self.opaque_disabled_surface, to.opaque_disabled_surface),
			hud_tint: m(self.hud_tint, to.hud_tint),
			hud_hue_tint: (lerp(s0, s1), lerp(b0, b1), lerp(o0, o1)),
		}
	}

	/// The near-black HUD tint with a whisper of `accent`'s hue, so the HUD follows the
	/// accent without becoming a second signal color.
	pub fn hud_tint_for(&self, accent: Rgba) -> Rgba {
		if !is_chromatic(accent) {
			return self.hud_tint;
		}
		let (saturation, brightness, opacity) = self.hud_hue_tint;
		hsb(hue(accent), saturation, brightness, opacity)
	}
}

impl Default for Palette {
	fn default() -> Self {
		Self::ARKNIGHTS
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn hud_tint_stays_neutral() {
		let palette = Palette::ARKNIGHTS;
		assert!(!is_chromatic(palette.hud_tint_for(palette.accent)));
	}
}
