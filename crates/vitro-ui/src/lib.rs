// SPDX-License-Identifier: MPL-2.0

//! Vitro: a glass UI kit on gpui-ce.
//!
//! The kit ports the Arknights Client design language: tokens, the Liquid Glass looks of
//! macOS 26 and 27, the motion curves, and the shared controls and HUD surfaces. Every
//! component takes a [`Theme`] value explicitly; there is no global theme. Asynchronous
//! work that feeds components lives in the `vitro-exec` crate.

pub mod controls;
pub mod glass;
pub mod hud;
pub mod icons;
pub mod motion;
pub mod progress;
pub mod tokens;

use gpui::Rgba;

pub use glass::{GlassStyle, Transparency};

/// Values every component reads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
	/// Dynamic accent from the artwork, or [`tokens::CYAN`] as the fallback.
	pub accent: Rgba,
	/// Near-black HUD glass tint with a whisper of the accent hue.
	pub hud_tint: Rgba,
	pub glass: GlassStyle,
	pub reduce_motion: bool,
	/// Accessibility "Show borders" / Increase Contrast.
	pub shows_borders: bool,
}

impl Theme {
	pub fn new(accent: Option<Rgba>) -> Self {
		Self {
			accent: accent.unwrap_or(tokens::CYAN),
			hud_tint: tokens::hud_tint_for(accent),
			glass: GlassStyle::MacOS27(Transparency::Default),
			reduce_motion: false,
			shows_borders: false,
		}
	}

	pub fn with_glass(mut self, glass: GlassStyle) -> Self {
		self.glass = glass;
		self
	}

	pub fn reduce_transparency(&self) -> bool {
		self.glass == GlassStyle::ReducedTransparency
	}

	/// A glass surface in this theme.
	pub fn surface(&self, tint: Option<Rgba>) -> glass::Glass {
		glass::Glass {
			shows_border: self.shows_borders,
			..glass::Glass::new(tint, self.glass)
		}
	}
}

/// The platform UI font. Inter replaces it outside Apple platforms.
#[cfg(target_os = "macos")]
pub const UI_FONT: &str = ".SystemUIFont";
#[cfg(not(target_os = "macos"))]
pub const UI_FONT: &str = "Inter";
