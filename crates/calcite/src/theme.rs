// SPDX-License-Identifier: MPL-2.0

//! The theme every component reads: accent, HUD tint, glass style, and accessibility.
//!
//! The theme lives in a GPUI global. Components read it with [`ActiveTheme::theme`] and
//! accept `.theme(...)` to override it, for example to show several styles side by side.

use gpui::{App, Context, Global, Rgba, Window};

use crate::{
	glass::{self, GlassStyle, Transparency},
	tokens,
};

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
	/// Pauses decorative loops (progress sweep, halo pulse) while the app covers the view
	/// with a sheet or runs a game, like `DecorativeMotionVisibility`.
	pub pauses_decorative_motion: bool,
}

impl Default for Theme {
	fn default() -> Self {
		Self::new(None)
	}
}

impl Global for Theme {}

impl Theme {
	pub fn new(accent: Option<Rgba>) -> Self {
		Self {
			accent: accent.unwrap_or(tokens::CYAN),
			hud_tint: tokens::hud_tint_for(accent),
			glass: GlassStyle::MacOS27(Transparency::Default),
			reduce_motion: false,
			shows_borders: false,
			pauses_decorative_motion: false,
		}
	}

	/// The default theme with the system's Reduce Motion setting.
	pub fn from_system(cx: &App) -> Self {
		Self {
			reduce_motion: cx.reduce_motion(),
			..Self::default()
		}
	}

	/// Sets the accent and derives the HUD tint from it. `None` uses the fallback cyan.
	pub fn with_accent(mut self, accent: Option<Rgba>) -> Self {
		self.accent = accent.unwrap_or(tokens::CYAN);
		self.hud_tint = tokens::hud_tint_for(accent);
		self
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

	/// The HUD surface: the near-black tint of the control bar and pills.
	pub fn hud_surface(&self) -> glass::Glass {
		self.surface(Some(self.hud_tint))
	}

	/// Whether decorative loops may run in `window` now: not under Reduce Motion, not while
	/// the app pauses them, and not while the window is in the background.
	pub fn decorative_motion(&self, window: &Window) -> bool {
		!self.reduce_motion && !self.pauses_decorative_motion && window.is_window_active()
	}

	/// Installs `theme` as the app theme and redraws every window.
	pub fn set(cx: &mut App, theme: Theme) {
		cx.set_global(theme);
		cx.refresh_windows();
	}

	/// Changes the app theme in place and redraws every window.
	pub fn update(cx: &mut App, update: impl FnOnce(&mut Theme)) {
		let mut theme = cx.theme();
		update(&mut theme);
		Self::set(cx, theme);
	}
}

/// Reads the app theme from any context.
pub trait ActiveTheme {
	fn theme(&self) -> Theme;
}

impl ActiveTheme for App {
	fn theme(&self) -> Theme {
		self.try_global::<Theme>().copied().unwrap_or_default()
	}
}

/// View helpers that keep themed views current.
pub trait ThemedView {
	/// Redraws the view when its window becomes active or inactive, so decorative motion
	/// stops in the background and resumes in the foreground.
	fn redraw_on_window_activation(&mut self, window: &mut Window);
}

impl<V: 'static> ThemedView for Context<'_, V> {
	fn redraw_on_window_activation(&mut self, window: &mut Window) {
		self.observe_window_activation(window, |_, _, cx| cx.notify())
			.detach();
	}
}
