// SPDX-License-Identifier: MPL-2.0

//! The theme every component reads: palette, accent, HUD tint, glass style, and
//! accessibility.
//!
//! The theme lives in a GPUI global. Components read it with [`ActiveTheme::theme`] and
//! accept `.theme(...)` to override it, for example to show several styles side by side.

use std::time::{Duration, Instant};

use gpui::{App, AsyncApp, AsyncWindowContext, Context, Global, Rgba, Window};

use crate::{
	glass::{self, GlassStyle, Transparency},
	motion::Curve,
	palette::Palette,
	tokens::mix,
};

/// Values every component reads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
	/// Every color the components draw with.
	pub palette: Palette,
	/// The accent in use: the dynamic accent, or the palette accent without one.
	pub accent: Rgba,
	/// Near-black HUD glass tint with a whisper of the accent hue.
	pub hud_tint: Rgba,
	/// The dynamic accent, for example from the artwork. `None` uses the palette accent.
	pub dynamic_accent: Option<Rgba>,
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
		Self::new(Palette::ARKNIGHTS)
	}
}

impl Theme {
	/// A theme with `palette` and no dynamic accent. Pass a `const` palette to pick the
	/// look at compile time.
	pub fn new(palette: Palette) -> Self {
		Self {
			palette,
			accent: palette.accent,
			hud_tint: palette.hud_tint,
			dynamic_accent: None,
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

	/// Sets the dynamic accent and derives the HUD tint from it. `None` uses the palette
	/// accent.
	pub fn with_accent(mut self, accent: Option<Rgba>) -> Self {
		self.dynamic_accent = accent;
		self.resolve()
	}

	/// Replaces the palette and keeps the dynamic accent.
	pub fn with_palette(mut self, palette: Palette) -> Self {
		self.palette = palette;
		self.resolve()
	}

	fn resolve(mut self) -> Self {
		self.accent = self.dynamic_accent.unwrap_or(self.palette.accent);
		// Like the launcher: only a dynamic accent tints the HUD; the fallback keeps it neutral.
		self.hud_tint = match self.dynamic_accent {
			Some(accent) => self.palette.hud_tint_for(accent),
			None => self.palette.hud_tint,
		};
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
			quiet_edge: self.palette.quiet_control,
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

	/// This theme part of the way toward `to`: colors mix, everything else is `to`'s.
	pub fn mix(&self, to: &Self, t: f32) -> Self {
		Self {
			palette: self.palette.mix(&to.palette, t),
			accent: mix(self.accent, to.accent, t),
			hud_tint: mix(self.hud_tint, to.hud_tint, t),
			..*to
		}
	}

	/// Installs `theme` as the app theme. Color changes crossfade over
	/// [`Curve::Crossfade`]; under Reduce Motion they apply at once.
	pub fn set(cx: &mut App, theme: Theme) {
		let from = cx.theme();
		let animates = !theme.reduce_motion && cx.has_global::<ThemeState>() && from != theme;
		if !animates {
			return Self::set_immediately(cx, theme);
		}
		let generation = cx.try_global::<ThemeState>().map_or(0, |state| state.generation) + 1;
		cx.set_global(ThemeState {
			from,
			to: theme,
			started: Instant::now(),
			generation,
		});
		cx.refresh_windows();
		// Redraw every frame until the crossfade ends or a newer change replaces it.
		cx.spawn(async move |cx| {
			loop {
				cx.background_executor().timer(FRAME).await;
				let running = cx.update(|cx| {
					let Some(state) = cx.try_global::<ThemeState>() else {
						return false;
					};
					if state.generation != generation {
						return false;
					}
					let running = state.started.elapsed() < CROSSFADE.duration();
					cx.refresh_windows();
					running
				});
				if !running {
					break;
				}
			}
		})
		.detach();
	}

	/// Installs `theme` without a transition.
	pub fn set_immediately(cx: &mut App, theme: Theme) {
		let generation = cx.try_global::<ThemeState>().map_or(0, |state| state.generation) + 1;
		cx.set_global(ThemeState {
			from: theme,
			to: theme,
			started: Instant::now(),
			generation,
		});
		cx.refresh_windows();
	}

	/// Changes the app theme in place, starting from the theme it is heading to.
	pub fn update(cx: &mut App, update: impl FnOnce(&mut Theme)) {
		let mut theme = cx.target_theme();
		update(&mut theme);
		Self::set(cx, theme);
	}

	/// Sets only the dynamic accent, for example the color picked from the artwork. The HUD
	/// tint takes its hue; the palette stays.
	pub fn set_dynamic_accent(cx: &mut App, accent: Option<Rgba>) {
		Self::update(cx, |theme| *theme = theme.with_accent(accent));
	}

	/// Sets the dynamic accent from an artwork tint (see [`calcite_tint::Extractor`]).
	/// `None`, for artwork without a hue, returns to the palette accent.
	#[cfg(feature = "tint")]
	pub fn set_artwork_tint(cx: &mut App, tint: Option<calcite_tint::Tint>) {
		Self::set_dynamic_accent(cx, tint.map(|tint| tint_color(tint.accent)));
	}
}

/// An artwork color as an opaque straight-sRGB color.
#[cfg(feature = "tint")]
pub fn tint_color(color: calcite_tint::Hsb) -> Rgba {
	let [red, green, blue] = color.to_rgb();
	crate::tokens::rgba(red, green, blue, 1.)
}

const CROSSFADE: Curve = Curve::Crossfade;
const FRAME: Duration = Duration::from_millis(16);

/// Whether the app has a theme yet.
pub(crate) fn is_installed(cx: &App) -> bool {
	cx.has_global::<ThemeState>()
}

/// The app theme and the crossfade toward it.
struct ThemeState {
	from: Theme,
	to: Theme,
	started: Instant,
	generation: u64,
}

impl Global for ThemeState {}

impl ThemeState {
	fn current(&self) -> Theme {
		let t = CROSSFADE.progress(self.started.elapsed());
		if t >= 1. {
			self.to
		} else {
			self.from.mix(&self.to, t)
		}
	}
}

/// Reads the app theme from any GPUI context: `App`, `Context<V>` (through `App`),
/// `AsyncApp`, and `AsyncWindowContext`.
pub trait ActiveTheme {
	/// The theme to draw with now, part of the way through a crossfade.
	fn theme(&self) -> Theme;
	/// The theme a running crossfade is heading to.
	fn target_theme(&self) -> Theme;
}

impl ActiveTheme for App {
	fn theme(&self) -> Theme {
		self.try_global::<ThemeState>()
			.map(ThemeState::current)
			.unwrap_or_default()
	}

	fn target_theme(&self) -> Theme {
		self.try_global::<ThemeState>()
			.map(|state| state.to)
			.unwrap_or_default()
	}
}

impl ActiveTheme for AsyncApp {
	fn theme(&self) -> Theme {
		self.update(|cx| cx.theme())
	}

	fn target_theme(&self) -> Theme {
		self.update(|cx| cx.target_theme())
	}
}

impl ActiveTheme for AsyncWindowContext {
	fn theme(&self) -> Theme {
		(**self).theme()
	}

	fn target_theme(&self) -> Theme {
		(**self).target_theme()
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
