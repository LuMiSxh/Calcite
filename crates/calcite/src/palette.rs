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
	/// The neutral that fills, tracks, dividers and hover lights are drawn in: white on a dark
	/// palette, black on a light one. Alpha is ignored; components set their own.
	pub ink: Rgba,
	/// The opposite neutral, laid behind glass as its backplate: black on a dark palette, white on a
	/// light one. Shadows stay black and soften on a light palette.
	pub shade: Rgba,
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
		ink: white(1.),
		shade: black(1.),
	};

	/// A light look: pale glass and backdrop, near-black text, a deeper accent, and ink and shade
	/// swapped so fills, tracks and the backplate turn dark-on-light. The HUD tint is a pale veil,
	/// so menus, tooltips and HUD surfaces keep the dark `primary_text` legible.
	pub const LIGHT: Self = Self {
		accent: rgba(0.0, 0.50, 0.80, 1.),
		control: rgba(0.36, 0.38, 0.42, 1.),
		quiet_control: rgba(0.46, 0.49, 0.53, 1.),
		hairline: black(0.14),
		danger: rgba(0.78, 0.16, 0.22, 1.),
		danger_text: rgba(0.74, 0.12, 0.2, 1.),
		warning: rgba(0.78, 0.46, 0.04, 1.),
		success: rgba(0.10, 0.56, 0.32, 1.),
		primary_text: black(0.88),
		secondary_text: black(0.58),
		backdrop: rgba(0.93, 0.94, 0.96, 1.),
		panel_background: rgba(0.97, 0.97, 0.98, 1.),
		panel_border: black(0.10),
		navigation_rail: white(0.55),
		navigation_hover: black(0.05),
		opaque_surface: rgba(0.90, 0.91, 0.93, 1.),
		opaque_disabled_surface: rgba(0.85, 0.86, 0.88, 1.),
		hud_tint: white(0.55),
		hud_hue_tint: (0.12, 0.95, 0.55),
		ink: black(1.),
		shade: white(1.),
	};

	/// This palette with another fallback accent.
	pub const fn with_accent(mut self, accent: Rgba) -> Self {
		self.accent = accent;
		self
	}

	/// Whether the palette draws dark ink on light surfaces.
	pub fn is_light(&self) -> bool {
		self.ink.red < 0.5
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
			ink: m(self.ink, to.ink),
			shade: m(self.shade, to.shade),
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
	use crate::{
		glass::{GlassStyle, Transparency},
		theme::Theme,
		tokens::{contrast_ratio, on_color, over},
	};

	/// The palette's main surfaces as the opaque color they show over its backdrop, for each glass
	/// style.
	fn surfaces(palette: Palette) -> Vec<(String, Rgba)> {
		let mut out = vec![
			("backdrop".into(), palette.backdrop),
			("panel".into(), palette.panel_background),
			("opaque surface".into(), palette.opaque_surface),
		];
		let styles = [
			GlassStyle::MacOS26,
			GlassStyle::MacOS27(Transparency::Default),
			GlassStyle::MacOS27(Transparency::Clear),
			GlassStyle::ReducedTransparency,
		];
		for style in styles {
			let theme = Theme::new(palette).with_glass(style);
			out.push((
				format!("glass {style:?}"),
				over(theme.surface(None).fill(), palette.backdrop),
			));
			out.push((
				format!("hud {style:?}"),
				over(theme.hud_surface().fill(), palette.backdrop),
			));
		}
		out
	}

	#[test]
	fn text_reads_on_every_main_surface_in_both_palettes() {
		for (name, palette) in [("dark", Palette::ARKNIGHTS), ("light", Palette::LIGHT)] {
			for (surface, base) in surfaces(palette) {
				let base = with_alpha_one(base);
				let primary = contrast_ratio(over(palette.primary_text, base), base);
				let secondary = contrast_ratio(over(palette.secondary_text, base), base);
				assert!(primary >= 4.5, "{name} primary text on {surface}: {primary:.2}");
				assert!(
					secondary >= 3.,
					"{name} secondary text on {surface}: {secondary:.2}"
				);
			}
		}
	}

	fn with_alpha_one(color: Rgba) -> Rgba {
		crate::tokens::with_alpha(color, 1.)
	}

	#[test]
	fn the_light_palette_is_light_and_its_hud_tint_is_a_pale_veil() {
		assert!(Palette::LIGHT.is_light());
		assert!(!is_chromatic(Palette::LIGHT.hud_tint));
		assert!(Palette::LIGHT.hud_tint.red > 0.9);
	}

	#[test]
	fn prominent_labels_read_on_the_accent_and_status_fills() {
		for palette in [Palette::ARKNIGHTS, Palette::LIGHT] {
			for fill in [palette.accent, palette.danger, palette.warning, palette.success] {
				let ratio = contrast_ratio(on_color(fill), fill);
				assert!(ratio >= 3., "label on {fill:?}: {ratio:.2}");
			}
		}
	}

	#[test]
	fn hud_tint_stays_neutral() {
		let palette = Palette::ARKNIGHTS;
		assert!(!is_chromatic(palette.hud_tint_for(palette.accent)));
	}

	#[test]
	fn a_light_palette_flips_the_neutrals_the_glass_and_fills_draw_with() {
		let light = Palette {
			ink: black(1.),
			shade: white(1.),
			..Palette::ARKNIGHTS
		};
		assert!(!Palette::ARKNIGHTS.is_light());
		assert!(light.is_light());
		let theme = crate::theme::Theme::new(light);
		assert_eq!(theme.ink(0.2), black(0.2));
		let glass = theme.surface(None);
		assert_eq!((glass.ink, glass.shade), (black(1.), white(1.)));
		// The backplate behind neutral glass is light, and the shadow softer.
		let dark_fill = crate::theme::Theme::default().surface(None).fill();
		assert!(glass.fill().red > dark_fill.red);
		assert!(theme.shadow(0.4).alpha < 0.4);
		assert_eq!(light.mix(&Palette::ARKNIGHTS, 1.).ink, white(1.));
	}
}
