// SPDX-License-Identifier: MPL-2.0

//! The theme every component reads: palette, accent, HUD tint, glass style, and
//! accessibility.
//!
//! The theme lives in a GPUI global. Components read it with [`ActiveTheme::theme`] and
//! accept `.theme(...)` to override it, for example to show several styles side by side.

use std::{
	cell::Cell,
	time::{Duration, Instant},
};

use gpui::{
	App, AsyncApp, AsyncWindowContext, Context, Global, Rgba, SharedString, Subscription, Window,
	WindowAppearance,
};

use crate::{
	glass::{self, GlassStyle},
	motion::Curve,
	palette::Palette,
	style::{Fonts, Style},
	tokens::{self, mix, scale_alpha},
};

/// Values every component reads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
	/// Radii, glass look and strength, and pointer light.
	pub style: Style,
	/// Every color the components draw with.
	pub palette: Palette,
	/// The accent in use: the dynamic accent, or the palette accent without one.
	pub accent: Rgba,
	/// Near-black HUD glass tint with a whisper of the accent hue.
	pub hud_tint: Rgba,
	/// The dynamic accent, for example from the artwork. `None` uses the palette accent.
	pub dynamic_accent: Option<Rgba>,
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
			style: Style::default(),
			palette,
			accent: palette.accent,
			hud_tint: palette.hud_tint,
			dynamic_accent: None,
			reduce_motion: false,
			shows_borders: false,
			pauses_decorative_motion: false,
		}
	}

	/// The default theme with the system's Reduce Motion setting.
	///
	/// This reads the setting once; [`Theme::follow_system`] keeps the theme current.
	pub fn from_system(cx: &App) -> Self {
		Self {
			reduce_motion: cx.reduce_motion(),
			..Self::default()
		}
	}

	/// Follows the system appearance: installs [`Palette::ARKNIGHTS`] while `window` is dark and
	/// [`Palette::LIGHT`] while it is light, and switches (with the theme crossfade) when the OS
	/// changes. Hold or `.detach()` the returned subscription.
	///
	/// It also copies `cx.reduce_motion()` into [`Theme::reduce_motion`] on every change. gpui-ce
	/// has no notification for that flag, so it is re-read when the appearance changes; call
	/// [`Theme::sync_reduce_motion`] after `cx.set_reduce_motion(..)` for an immediate update.
	///
	/// gpui-ce does not expose Reduce Transparency or Increase Contrast, so they are not
	/// followed: set [`GlassStyle::ReducedTransparency`] with [`Theme::with_glass`] and
	/// [`Theme::shows_borders`] yourself when your app learns them from the platform.
	pub fn follow_system(window: &mut Window, cx: &mut App) -> Subscription {
		Self::follow_system_with(Palette::ARKNIGHTS, Palette::LIGHT, window, cx)
	}

	/// Like [`Theme::follow_system`], with your own `dark` and `light` palettes.
	pub fn follow_system_with(
		dark: Palette,
		light: Palette,
		window: &mut Window,
		cx: &mut App,
	) -> Subscription {
		let sync = move |appearance: WindowAppearance, cx: &mut App, animate: bool| {
			let palette = match appearance {
				WindowAppearance::Light | WindowAppearance::VibrantLight => light,
				WindowAppearance::Dark | WindowAppearance::VibrantDark => dark,
			};
			let mut theme = cx.target_theme();
			if theme.palette != palette {
				theme = theme.with_palette(palette);
			}
			if theme == cx.target_theme() {
				return;
			}
			if animate {
				Self::set(cx, theme);
			} else {
				Self::set_immediately(cx, theme);
			}
		};
		sync(window.appearance(), cx, false);
		window.observe_window_appearance(move |window, cx| {
			sync(window.appearance(), cx, true);
		})
	}

	/// Copies the app's Reduce Motion flag (`cx.reduce_motion()`) into the installed theme.
	pub fn sync_reduce_motion(cx: &mut App) {
		let reduce_motion = cx.reduce_motion();
		if cx.target_theme().reduce_motion != reduce_motion {
			Self::update(cx, |theme| theme.reduce_motion = reduce_motion);
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

	/// Replaces the style: radii, glass, glass strength, and pointer light.
	pub fn with_style(mut self, style: Style) -> Self {
		self.style = style;
		self
	}

	/// Replaces only the font families of the style.
	pub fn with_fonts(mut self, fonts: Fonts) -> Self {
		self.style.fonts = fonts;
		self
	}

	/// The sans family name for `.font_family(..)`.
	pub fn sans(&self) -> SharedString {
		self.style.fonts.sans.into()
	}

	/// The monospace family name for `.font_family(..)`.
	pub fn mono(&self) -> SharedString {
		self.style.fonts.mono.into()
	}

	/// Replaces only the glass look of the style.
	pub fn with_glass(mut self, glass: GlassStyle) -> Self {
		self.style.glass = glass;
		self
	}

	pub fn reduce_transparency(&self) -> bool {
		self.style.glass == GlassStyle::ReducedTransparency
	}

	/// A glass surface in this theme.
	pub fn surface(&self, tint: Option<Rgba>) -> glass::Glass {
		glass::Glass {
			shows_border: self.shows_borders,
			quiet_edge: self.palette.quiet_control,
			ink: self.palette.ink,
			shade: self.palette.shade,
			shadow_scale: if self.palette.is_light() { LIGHT_SHADOW } else { 1. },
			..glass::Glass::new(tint, self.style.glass).strength(self.style.glass_strength)
		}
	}

	/// The palette's neutral ink at `alpha`: fills, tracks, dividers and hover lights.
	pub fn ink(&self, alpha: f32) -> Rgba {
		tokens::with_alpha(self.palette.ink, alpha)
	}

	/// The dim behind a modal: [`tokens::control::OVERLAY_DIM`] of black, lighter on a light
	/// palette.
	pub fn scrim(&self) -> Rgba {
		tokens::black(tokens::control::OVERLAY_DIM * if self.palette.is_light() { LIGHT_SCRIM } else { 1. })
	}

	/// A drop-shadow colour: black, softer on a light palette.
	pub fn shadow(&self, alpha: f32) -> Rgba {
		tokens::black(alpha * if self.palette.is_light() { LIGHT_SHADOW } else { 1. })
	}

	/// `color` as text or an icon on this palette's surfaces: lightened toward white on a dark
	/// palette, darkened toward black on a light one, so a chromatic label stays readable.
	pub fn legible(&self, color: Rgba) -> Rgba {
		if self.palette.is_light() {
			tokens::mix(color, tokens::black(color.alpha), LIGHT_TEXT_DEPTH)
		} else {
			tokens::contrast_tint(color)
		}
	}

	/// The HUD surface: the near-black tint of the control bar and pills.
	pub fn hud_surface(&self) -> glass::Glass {
		self.surface(Some(self.hud_tint))
	}

	/// The hairline edge of a HUD surface.
	pub fn hud_border(&self) -> Rgba {
		scale_alpha(self.palette.hairline, tokens::control::HUD_BORDER_OPACITY)
	}

	/// Whether decorative loops may run in `window` now: not under Reduce Motion, not while
	/// the app pauses them, and not while the window is in the background.
	pub fn decorative_motion(&self, window: &Window) -> bool {
		!self.reduce_motion && !self.pauses_decorative_motion && window.is_window_active()
	}

	/// This theme part of the way toward `to`: colors mix, everything else is `to`'s.
	/// Numeric style fields (radii, glass strength) mix too.
	pub fn mix(&self, to: &Self, t: f32) -> Self {
		Self {
			style: self.style.mix(&to.style, t),
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
		let now = Instant::now();
		cx.set_global(ThemeState {
			from,
			to: theme,
			started: now,
			sampled: Cell::new(now),
			generation,
		});
		cx.refresh_windows();
		// Redraw on every display refresh until the crossfade ends or a newer change replaces it.
		cx.defer(move |cx| {
			for handle in cx.windows() {
				handle
					.update(cx, |_, window, _| follow_crossfade(window, generation))
					.ok();
			}
		});
	}

	/// Installs `theme` without a transition.
	pub fn set_immediately(cx: &mut App, theme: Theme) {
		let generation = cx.try_global::<ThemeState>().map_or(0, |state| state.generation) + 1;
		let now = Instant::now();
		cx.set_global(ThemeState {
			from: theme,
			to: theme,
			started: now,
			sampled: Cell::new(now),
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

/// How much of a dark palette's shadow a light palette keeps.
const LIGHT_SHADOW: f32 = 0.45;

/// How far a light palette darkens a chromatic label toward black.
const LIGHT_TEXT_DEPTH: f32 = 0.38;

/// How much of a dark palette's modal dim a light palette keeps.
const LIGHT_SCRIM: f32 = 0.7;

/// A crossfade sample older than this is not from a running crossfade, so it is not used.
const SAMPLE_LIFETIME: Duration = Duration::from_millis(100);

/// Redraws `window` before each display refresh until the crossfade `generation` ends. The
/// callback runs just before the window draws, so it fixes the time every component
/// samples this frame; sampling the clock per `cx.theme()` call put components of one frame
/// on different mixes.
fn follow_crossfade(window: &mut Window, generation: u64) {
	window.on_next_frame(move |window, cx| {
		let Some(state) = cx.try_global::<ThemeState>() else {
			return;
		};
		if state.generation != generation {
			return;
		}
		let now = Instant::now();
		state.sampled.set(now);
		let running = now.saturating_duration_since(state.started) < CROSSFADE.duration();
		// Intentional: the crossfade changes every colour in every view, including cached ones,
		// so the whole window must repaint each frame until it ends.
		window.refresh();
		if running {
			follow_crossfade(window, generation);
		}
	});
}

/// Whether the app has a theme yet.
pub(crate) fn is_installed(cx: &App) -> bool {
	cx.has_global::<ThemeState>()
}

/// The app theme and the crossfade toward it.
struct ThemeState {
	from: Theme,
	to: Theme,
	started: Instant,
	/// The time of the frame being drawn; see [`follow_crossfade`].
	sampled: Cell<Instant>,
	generation: u64,
}

impl Global for ThemeState {}

impl ThemeState {
	fn current(&self) -> Theme {
		let sampled = self.sampled.get();
		let now = if sampled.elapsed() < SAMPLE_LIFETIME {
			sampled
		} else {
			Instant::now()
		};
		let t = CROSSFADE.progress(now.saturating_duration_since(self.started));
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

#[cfg(test)]
mod tests {
	use super::*;
	use crate::style::{Fonts, Radii};

	#[test]
	fn mix_interpolates_style_numbers() {
		let from = Theme::default();
		let to =
			Theme::default().with_style(Style::default().with_radii(Radii::ROUND).with_glass_strength(0.));
		let mid = from.mix(&to, 0.5);
		assert_eq!(mid.style.glass_strength, 0.5);
		assert_eq!(mid.style.radii, Radii::DEFAULT.mix(Radii::ROUND, 0.5));
		assert_eq!(from.mix(&to, 1.).style, to.style);
	}

	#[test]
	fn the_modal_scrim_is_lighter_on_a_light_palette() {
		let dark = Theme::default().scrim();
		let light = Theme::new(crate::palette::Palette::LIGHT).scrim();
		assert_eq!(dark.alpha, tokens::control::OVERLAY_DIM);
		assert!(light.alpha < dark.alpha && light.alpha > 0.1);
	}

	#[test]
	fn with_glass_and_reduce_transparency_follow_the_style() {
		let theme = Theme::default().with_glass(GlassStyle::ReducedTransparency);
		assert!(theme.reduce_transparency());
		assert_eq!(theme.style.glass, GlassStyle::ReducedTransparency);
	}

	#[test]
	fn with_fonts_changes_the_families_components_draw_with() {
		let theme = Theme::default().with_fonts(Fonts::new("Georgia", "Menlo"));
		assert_eq!(theme.sans().as_ref(), "Georgia");
		assert_eq!(theme.mono().as_ref(), "Menlo");
		assert_eq!(Theme::default().sans().as_ref(), "Plus Jakarta Sans");
	}
}
