// SPDX-License-Identifier: MPL-2.0

//! The shape of the look, apart from its colors: corner radii, which system glass is
//! imitated and how strongly, and whether controls carry the pointer light.
//!
//! A [`Style`] lives in [`Theme::style`](crate::theme::Theme::style). Colors live in the
//! [`Palette`](crate::palette::Palette).
//!
//! ```ignore
//! let theme = Theme::default().with_style(
//!     Style::macos_27(Transparency::Clear)
//!         .with_radii(Radii::ROUND)
//!         .with_glass_strength(0.8)
//!         .with_pointer_glow(true),
//! );
//! ```

use std::{
	collections::HashSet,
	sync::{LazyLock, Mutex},
};

use gpui::{Pixels, px};

use crate::{
	fonts::Family,
	glass::{GlassStyle, Transparency},
	tokens::radius,
};

/// The font families components use: `sans` for text, `mono` for code. Any family
/// name the text system knows works, bundled or installed. The names are `'static` so that
/// [`Style`] and [`Theme`](crate::theme::Theme) stay `Copy`; [`Fonts::new`] interns a name
/// once, so calling it repeatedly with the same names does not grow memory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fonts {
	pub sans: &'static str,
	pub mono: &'static str,
}

/// Leaks each distinct family name once.
fn intern(name: &str) -> &'static str {
	static NAMES: LazyLock<Mutex<HashSet<&'static str>>> = LazyLock::new(Mutex::default);
	let mut names = NAMES.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
	if let Some(known) = names.get(name) {
		return known;
	}
	let leaked: &'static str = Box::leak(name.to_owned().into_boxed_str());
	names.insert(leaked);
	leaked
}

impl Fonts {
	/// Both families by name.
	pub fn new(sans: &str, mono: &str) -> Self {
		Self {
			sans: intern(sans),
			mono: intern(mono),
		}
	}

	/// The platform's own UI and monospace fonts, ignoring the bundled families.
	pub fn system() -> Self {
		let mono = if cfg!(target_os = "macos") {
			"Menlo"
		} else if cfg!(target_os = "windows") {
			"Consolas"
		} else {
			"DejaVu Sans Mono"
		};
		Self {
			sans: ".SystemUIFont",
			mono,
		}
	}
}

impl Default for Fonts {
	/// The bundled families, [`Family::PlusJakartaSans`] and [`Family::FiraCode`].
	fn default() -> Self {
		Self {
			sans: Family::PlusJakartaSans.name(),
			mono: Family::FiraCode.name(),
		}
	}
}

/// Corner radius of each role.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Radii {
	/// Small controls: tooltips, menu rows, list rows, option chips.
	pub control: Pixels,
	/// Selectable rows and cards.
	pub row: Pixels,
	/// Panels and menus.
	pub panel: Pixels,
	/// An expanded HUD surface. Collapsed pills stay capsules.
	pub hud_pill: Pixels,
	/// Modals and dialogs.
	pub modal: Pixels,
	/// Corner radius of capsule-shaped controls, as a fraction of half their height: `1.` is a
	/// full capsule and `0.` a square. Read it through [`Radii::capsule_radius`].
	pub capsule: f32,
}

impl Radii {
	/// Nearly square corners.
	pub const SHARP: Self = Self {
		control: px(3.),
		row: px(3.),
		panel: px(5.),
		hud_pill: px(8.),
		modal: px(8.),
		capsule: 0.2,
	};
	/// The launcher's radii.
	pub const DEFAULT: Self = Self {
		control: radius::CONTROL,
		row: radius::ROW,
		panel: radius::PANEL,
		hud_pill: radius::HUD_PILL,
		modal: radius::MODAL,
		capsule: 1.,
	};
	/// Generous, soft corners.
	pub const ROUND: Self = Self {
		control: px(14.),
		row: px(16.),
		panel: px(24.),
		hud_pill: px(30.),
		modal: px(34.),
		capsule: 1.,
	};

	/// Every pixel radius times `factor`. `capsule` is a fraction, not a length, so it stays as it is.
	pub fn scale(self, factor: f32) -> Self {
		let by = |radius: Pixels| px(f32::from(radius) * factor);
		Self {
			control: by(self.control),
			row: by(self.row),
			panel: by(self.panel),
			hud_pill: by(self.hud_pill),
			modal: by(self.modal),
			capsule: self.capsule,
		}
	}

	/// The corner radius of a capsule-shaped control `height` tall.
	/// The corner radius of a capsule-shaped control about `height` tall. A full capsule
	/// (`capsule >= 1`) returns gpui's `rounded_full` radius, which the renderer clamps to the
	/// real half-height, so controls whose height is only estimated stay true capsules.
	pub fn capsule_radius(&self, height: Pixels) -> Pixels {
		if self.capsule >= 1. {
			px(9999.)
		} else {
			height / 2. * self.capsule
		}
	}

	/// These radii part of the way toward `to`; `t = 0` gives `self`.
	pub fn mix(self, to: Self, t: f32) -> Self {
		let t = t.clamp(0., 1.);
		let lerp = |a: Pixels, b: Pixels| px(f32::from(a) + (f32::from(b) - f32::from(a)) * t);
		Self {
			control: lerp(self.control, to.control),
			row: lerp(self.row, to.row),
			panel: lerp(self.panel, to.panel),
			hud_pill: lerp(self.hud_pill, to.hud_pill),
			modal: lerp(self.modal, to.modal),
			capsule: self.capsule + (to.capsule - self.capsule) * t,
		}
	}
}

impl Default for Radii {
	fn default() -> Self {
		Self::DEFAULT
	}
}

/// Radii, glass, and pointer light of a theme.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style {
	pub radii: Radii,
	/// Which system look the glass imitates.
	pub glass: GlassStyle,
	/// Multiplies the blur, bezel, refraction, and tint and backplate opacity of glass.
	/// 1 is the system look, 0 is flat.
	pub glass_strength: f32,
	/// Whether controls carry the pointer light by default. Components override it with
	/// `.glow(bool)`.
	pub pointer_glow: bool,
	/// The font families components use.
	pub fonts: Fonts,
	/// Multiplies text sizes, control heights, paddings, and icon sizes. 1 is the base size
	/// of the [`tokens`](crate::tokens).
	pub scale: f32,
}

impl Style {
	/// The macOS 26 look.
	pub fn macos_26() -> Self {
		Self::with_look(GlassStyle::MacOS26)
	}

	/// The macOS 27 look at one stop of the transparency slider.
	pub fn macos_27(transparency: Transparency) -> Self {
		Self::with_look(GlassStyle::MacOS27(transparency))
	}

	fn with_look(glass: GlassStyle) -> Self {
		Self {
			radii: Radii::DEFAULT,
			glass,
			glass_strength: 1.,
			pointer_glow: false,
			fonts: Fonts::default(),
			scale: 1.,
		}
	}

	pub fn with_radii(mut self, radii: Radii) -> Self {
		self.radii = radii;
		self
	}

	pub fn with_glass(mut self, glass: GlassStyle) -> Self {
		self.glass = glass;
		self
	}

	pub fn with_glass_strength(mut self, strength: f32) -> Self {
		self.glass_strength = strength.max(0.);
		self
	}

	pub fn with_pointer_glow(mut self, pointer_glow: bool) -> Self {
		self.pointer_glow = pointer_glow;
		self
	}

	pub fn with_fonts(mut self, fonts: Fonts) -> Self {
		self.fonts = fonts;
		self
	}

	/// Scales text, controls, and icons. Clamped to 0.5 to 2.
	pub fn with_scale(mut self, scale: f32) -> Self {
		self.scale = scale.clamp(0.5, 2.);
		self
	}

	/// This style part of the way toward `to`: radii, glass strength, and scale interpolate,
	/// the rest is `to`'s.
	pub fn mix(&self, to: &Self, t: f32) -> Self {
		let t = t.clamp(0., 1.);
		Self {
			radii: self.radii.mix(to.radii, t),
			glass_strength: self.glass_strength + (to.glass_strength - self.glass_strength) * t,
			scale: self.scale + (to.scale - self.scale) * t,
			..*to
		}
	}
}

impl Default for Style {
	/// macOS 27 at the Default transparency stop.
	fn default() -> Self {
		Self::macos_27(Transparency::Default)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn radii_scale_every_role() {
		let fonts = Fonts::default();
		assert_eq!((fonts.sans, fonts.mono), ("Plus Jakarta Sans", "Fira Code"));
		let custom = Fonts::new("Georgia", "Menlo");
		assert!(std::ptr::eq(custom.sans, Fonts::new("Georgia", "Menlo").sans));
		let style = Style::default().with_fonts(Fonts::system());
		assert_eq!(style.fonts.sans, ".SystemUIFont");
		assert_eq!(style.mix(&Style::default(), 0.5).fonts, Fonts::default());
		let half = Radii::DEFAULT.scale(0.5);
		assert_eq!(half.control, px(5.));
		assert_eq!(half.panel, px(8.));
		assert_eq!(half.modal, px(12.));
		assert_eq!(Radii::DEFAULT.scale(1.), Radii::DEFAULT);
	}

	#[test]
	fn radii_mix_interpolates() {
		let mid = Radii::SHARP.mix(Radii::ROUND, 0.5);
		assert_eq!(mid.control, px((3. + 14.) / 2.));
		assert_eq!(Radii::SHARP.mix(Radii::ROUND, 0.), Radii::SHARP);
		assert_eq!(Radii::SHARP.mix(Radii::ROUND, 1.), Radii::ROUND);
	}

	#[test]
	fn capsule_radius_is_half_height_scaled_by_the_capsule_fraction() {
		let height = px(28.);
		assert!(Radii::DEFAULT.capsule_radius(height) >= px(14.));
		assert!(Radii::SHARP.capsule_radius(height) < px(14.));
		let mid = Radii::SHARP.mix(Radii::ROUND, 0.5);
		assert!((mid.capsule - (Radii::SHARP.capsule + 1.) / 2.).abs() < 1e-6);
		assert_eq!(mid.capsule_radius(height), mid.capsule * px(14.));
	}

	#[test]
	fn default_style_is_macos_27_default_without_glow() {
		let style = Style::default();
		assert_eq!(style.scale, 1.);
		assert_eq!(style.glass, GlassStyle::MacOS27(Transparency::Default));
		assert_eq!(style.radii, Radii::DEFAULT);
		assert_eq!(style.glass_strength, 1.);
		assert!(!style.pointer_glow);
	}

	#[test]
	fn mix_interpolates_numbers_and_snaps_the_rest() {
		let small = Style::default().with_scale(0.8);
		assert_eq!(Style::default().with_scale(9.).scale, 2.);
		assert!((Style::default().mix(&small, 0.5).scale - 0.9).abs() < 1e-6);
		assert_eq!(Style::default().mix(&small, 1.).scale, 0.8);
		let from = Style::default();
		let to = Style::macos_26()
			.with_radii(Radii::ROUND)
			.with_glass_strength(0.)
			.with_pointer_glow(true);
		let mid = from.mix(&to, 0.5);
		assert_eq!(mid.glass_strength, 0.5);
		assert_eq!(mid.radii, Radii::DEFAULT.mix(Radii::ROUND, 0.5));
		assert_eq!(mid.glass, GlassStyle::MacOS26);
		assert!(mid.pointer_glow);
	}
}
