// SPDX-License-Identifier: MPL-2.0

//! Prototype of the Arknights Client UI kit on gpui-ce.
//!
//! The kit ports the launcher's design tokens, the macOS 27 glass treatment, the motion
//! curves, and the shared HUD controls. It holds no launcher logic.

pub mod artwork;
pub mod controls;
pub mod glass;
pub mod hud;
pub mod icons;
pub mod motion;
pub mod progress;
pub mod tokens;

use gpui::Rgba;

/// Values every control reads. Views receive them explicitly; there is no global theme.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Appearance {
	/// Dynamic accent from the artwork, or [`tokens::CYAN`] as the fallback.
	pub accent: Rgba,
	/// Near-black HUD glass tint with a whisper of the accent hue.
	pub hud_tint: Rgba,
	pub reduce_transparency: bool,
	pub reduce_motion: bool,
}

impl Appearance {
	pub fn new(accent: Option<Rgba>) -> Self {
		Self {
			accent: accent.unwrap_or(tokens::CYAN),
			hud_tint: tokens::hud_tint_for(accent),
			reduce_transparency: false,
			reduce_motion: false,
		}
	}

	pub fn treatment(&self) -> glass::Treatment {
		if self.reduce_transparency {
			glass::Treatment::ReducedTransparency
		} else {
			glass::Treatment::Glass
		}
	}
}

/// The platform UI font. Inter replaces it on other platforms in the real app.
#[cfg(target_os = "macos")]
pub const UI_FONT: &str = ".SystemUIFont";
#[cfg(not(target_os = "macos"))]
pub const UI_FONT: &str = "Inter";
