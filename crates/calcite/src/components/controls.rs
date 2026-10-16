// SPDX-License-Identifier: MPL-2.0

//! Actions: [`button`] (`CapsuleActionButton`), [`icon_button`] (`IconActionButton`), and
//! [`halo`] (`AccentHalo`).
//!
//! ```ignore
//! button("play", "Play").accent().large().icon(Icon::Play).on_click(cx.listener(|this, _, _, cx| ..))
//! icon_button("settings", Icon::Gear, "Settings").on_click(..)
//! ```

use std::{rc::Rc, time::Duration};

use gpui::{
	Animation, AnimationExt, AnyElement, App, ClickEvent, ElementId, Filter, FontWeight, IntoElement,
	ParentElement, Pixels, Rgba, Role, SharedString, Styled, Window, div, ease_in_out, prelude::*, px,
};

use crate::{
	icons::IconRef,
	interaction::{
		ControlInteraction, ControlState, Feedback, FocusShape, KeyRole, KeyboardFocus, ScaleExt,
		focus_handle, glow_enabled, layer, register_key_target,
	},
	motion::cycle,
	surface::Surface,
	theme::{ActiveTheme, Theme},
	tokens::{self, control, is_chromatic, scale_alpha, with_alpha},
};

pub(crate) type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// A click handler that several listeners share, such as a click and a key.
pub(crate) type SharedClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// The one semantic tone of the library: it tints buttons and badges, banners
/// ([`Banner::tone`](crate::notice::Banner::tone)), and settings panels
/// ([`SettingsPanel::tone`](crate::layout::SettingsPanel::tone)).
///
/// ```ignore
/// button("save", "Save").tone(Tone::Accent)
/// banner("saved", "Preset saved").tone(Tone::Success)
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tone {
	/// The theme's dynamic accent: the current primary operation, or an informational notice.
	Accent,
	/// A fixed chromatic color.
	Color(Rgba),
	/// Secondary actions: quiet neutral surface. On a panel, the plain text color.
	Neutral,
	/// A positive outcome.
	Success,
	Warning,
	Danger,
}

impl Tone {
	pub(crate) fn foreground(self, theme: &Theme) -> Rgba {
		match self {
			Self::Accent => theme.accent,
			Self::Color(color) => color,
			Self::Neutral => theme.palette.control,
			Self::Success => theme.palette.success,
			Self::Warning => theme.palette.warning,
			Self::Danger => theme.palette.danger_text,
		}
	}

	/// The color of a notice or panel title in this tone: like [`Tone::foreground`], except
	/// that neutral is the plain text color.
	pub(crate) fn text(self, theme: &Theme) -> Rgba {
		match self {
			Self::Neutral => theme.palette.primary_text,
			tone => tone.foreground(theme),
		}
	}

	fn surface(self, theme: &Theme) -> Rgba {
		match self {
			Self::Danger => theme.palette.danger,
			_ => self.foreground(theme),
		}
	}

	/// Whether the surface is chromatic glass. Neutral never is: it stays a quiet flat
	/// fill whatever the palette's control tint is, so a palette crossfade cannot flip the
	/// paint kind when the mixed tint passes the chroma threshold.
	fn chromatic(self, theme: &Theme) -> bool {
		self != Self::Neutral && is_chromatic(self.surface(theme))
	}

	/// The label color: the tone's color, lightened toward white if chromatic. Neutral
	/// keeps its exact color so the label mixes linearly across palette changes.
	fn label(self, theme: &Theme) -> Rgba {
		match self {
			Self::Neutral => self.foreground(theme),
			_ => theme.legible(self.foreground(theme)),
		}
	}

	fn disabled_foreground(self, theme: &Theme) -> Rgba {
		match self {
			Self::Neutral => theme.ink(control::DISABLED_NEUTRAL_FOREGROUND_OPACITY),
			// Legible like the enabled label, so a pale warning or success stays readable in light.
			_ => with_alpha(self.label(theme), control::DISABLED_FOREGROUND_OPACITY),
		}
	}

	/// The solid fill of a prominent button. Neutral has no color of its own, so a prominent
	/// neutral button takes the accent, like the default button of a dialog.
	fn prominent_fill(self, theme: &Theme) -> Rgba {
		match self {
			Self::Neutral => with_alpha(theme.accent, 1.),
			_ => with_alpha(self.surface(theme), 1.),
		}
	}
}

/// Control sizes (`controlSize`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Size {
	Regular,
	Large,
	Compact,
}

impl Size {
	/// The control's height, shared with the other controls of its size.
	pub fn height(self) -> Pixels {
		match self {
			Self::Regular => control::HEIGHT_REGULAR,
			Self::Large => control::HEIGHT_LARGE,
			Self::Compact => control::HEIGHT_COMPACT,
		}
	}

	/// [`Size::height`] times the theme's scale.
	pub fn height_in(self, theme: &Theme) -> Pixels {
		theme.px(self.height())
	}

	/// Horizontal padding, scaled; the height comes from [`Size::height_in`].
	fn padding(self, theme: &Theme) -> Pixels {
		theme.pt(match self {
			Self::Regular => 12.,
			Self::Large => 16.,
			Self::Compact => control::COMPACT_HORIZONTAL_PADDING,
		})
	}

	fn text_size(self, theme: &Theme) -> Pixels {
		match self {
			Self::Compact => theme.text_caption(),
			_ => theme.text_body(),
		}
	}

	/// The corner radius: half the height, so every size is a capsule.
	fn corner(self, theme: &Theme) -> Pixels {
		theme.style.radii.capsule_radius(self.height_in(theme))
	}
}

/// A text action whose visible capsule and hit target share the same bounds. Standard and
/// large actions carry the pointer light.
#[derive(IntoElement)]
pub struct Button {
	id: ElementId,
	title: SharedString,
	icon: Option<IconRef>,
	tone: Tone,
	size: Size,
	enabled: bool,
	default_action: bool,
	prominent: bool,
	glow: Option<bool>,
	theme: Option<Theme>,
	on_click: Option<SharedClickHandler>,
}

/// A capsule action. Defaults: neutral tone, regular size, enabled.
pub fn button(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Button {
	Button {
		id: id.into(),
		title: title.into(),
		icon: None,
		tone: Tone::Neutral,
		size: Size::Regular,
		enabled: true,
		default_action: false,
		prominent: false,
		glow: None,
		theme: None,
		on_click: None,
	}
}

impl Button {
	pub fn tone(mut self, tone: Tone) -> Self {
		self.tone = tone;
		self
	}

	/// The theme accent: use it for the current primary operation only.
	pub fn accent(self) -> Self {
		self.tone(Tone::Accent)
	}

	pub fn neutral(self) -> Self {
		self.tone(Tone::Neutral)
	}

	pub fn success(self) -> Self {
		self.tone(Tone::Success)
	}

	pub fn warning(self) -> Self {
		self.tone(Tone::Warning)
	}

	pub fn danger(self) -> Self {
		self.tone(Tone::Danger)
	}

	pub fn size(mut self, size: Size) -> Self {
		self.size = size;
		self
	}

	pub fn large(self) -> Self {
		self.size(Size::Large)
	}

	pub fn compact(self) -> Self {
		self.size(Size::Compact)
	}

	pub fn icon(mut self, icon: impl Into<IconRef>) -> Self {
		self.icon = Some(icon.into());
		self
	}

	/// Whether the button responds. Enabled by default.
	pub fn enabled(mut self, enabled: bool) -> Self {
		self.enabled = enabled;
		self
	}

	/// The inverse of [`Button::enabled`].
	pub fn disabled(self, disabled: bool) -> Self {
		self.enabled(!disabled)
	}

	/// Return clicks this button when no focused control consumes Return
	/// (`.keyboardShortcut(.defaultAction)`). The last default action rendered wins. A default
	/// action is also [`prominent`](Button::prominent).
	pub fn default_action(mut self) -> Self {
		self.default_action = true;
		self.prominent = true;
		self
	}

	/// The filled style: a solid fill in the tone (the accent for a neutral button) with a label
	/// that stays legible on it, white where it reads and near-black on a light accent.
	pub fn prominent(mut self) -> Self {
		self.prominent = true;
		self
	}

	/// Forces the pointer light on or off. Without it the button follows
	/// [`Style::pointer_glow`](crate::style::Style::pointer_glow); compact buttons never glow.
	pub fn glow(mut self, glow: bool) -> Self {
		self.glow = Some(glow);
		self
	}

	pub fn on_click(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
		self.on_click = Some(Rc::new(handler));
		self
	}
}

impl RenderOnce for Button {
	fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let state = ControlState::of(&self.id, window, cx);
		let focus = focus_handle(&self.id, window, cx);
		if let (true, true, Some(on_click)) = (self.default_action, self.enabled, self.on_click.clone()) {
			let handler = Rc::new(move |window: &mut Window, cx: &mut App| {
				on_click(&ClickEvent::default(), window, cx)
			});
			register_key_target(&self.id, KeyRole::DefaultAction, Some(handler), None, window, cx);
		}
		let frame = ControlState::frame(
			&state,
			Feedback::Standard,
			self.enabled,
			theme.reduce_motion,
			window,
			cx,
		);
		let filled = self.prominent && self.enabled;
		let fill = self.tone.prominent_fill(&theme);
		let on_fill = tokens::on_color(fill);
		let surface = if filled {
			Surface::solid(fill, on_fill)
		} else {
			Surface::resolve(
				self.tone.surface(&theme),
				self.tone.chromatic(&theme),
				self.enabled,
				&theme,
			)
		};
		let foreground = if filled {
			on_fill
		} else if self.enabled {
			self.tone.label(&theme)
		} else {
			self.tone.disabled_foreground(&theme)
		};
		let horizontal = self.size.padding(&theme);
		let text_size = self.size.text_size(&theme);
		let has_sheen = self.enabled && glow_enabled(&theme, self.glow, self.size == Size::Compact);
		let pointer_tint = (has_sheen && !filled).then(|| self.tone.foreground(&theme));

		let surface_layer = surface
			.paint(layer(), &frame, pointer_tint)
			.rounded(self.size.corner(&theme))
			.border_1()
			.border_color(surface.border);
		let content = div()
			.relative()
			.flex()
			.flex_row()
			.items_center()
			.gap(theme.pt(6.))
			.when_some(self.icon, |content, icon| {
				content.child(icon.render(text_size, foreground))
			})
			.child(self.title.clone());

		let button = div()
			.id(self.id)
			.relative()
			.flex()
			.items_center()
			.justify_center()
			.px(horizontal)
			.h(self.size.height_in(&theme))
			.flex_shrink_0()
			.font_family(theme.sans())
			.text_size(text_size)
			.font_weight(FontWeight::SEMIBOLD)
			.text_color(foreground)
			.role(Role::Button)
			.aria_label(self.title)
			.when(self.default_action, |button| button.aria_keyshortcuts("Enter"))
			.child(surface_layer)
			.child(content);
		if !self.enabled {
			return button.aria_disabled(true).cursor_default().scaled(frame.scale);
		}
		button
			.keyboard_focus(
				&focus,
				FocusShape::Rounded(self.size.corner(&theme)),
				&theme,
				window,
			)
			// `ActionPressStyle`: opacity 0.86 while pressed; the surface carries the scale.
			.opacity(frame.press_opacity())
			.control_state(&state, has_sheen)
			.when_some(self.on_click, |button, on_click| {
				button.on_click(move |event, window, cx| on_click(event, window, cx))
			})
			.scaled(frame.scale)
	}
}

/// A circular icon-only action, used for the Settings control over the artwork.
#[derive(IntoElement)]
pub struct IconButton {
	id: ElementId,
	icon: IconRef,
	label: SharedString,
	diameter: Pixels,
	surface: f32,
	enabled: bool,
	glow: Option<bool>,
	theme: Option<Theme>,
	on_click: Option<ClickHandler>,
}

/// A 40 pt HUD icon action with a 44 pt hit target. `label` is the accessible name.
pub fn icon_button(
	id: impl Into<ElementId>,
	icon: impl Into<IconRef>,
	label: impl Into<SharedString>,
) -> IconButton {
	IconButton {
		id: id.into(),
		icon: icon.into(),
		label: label.into(),
		diameter: px(40.),
		surface: 1.,
		enabled: true,
		glow: None,
		theme: None,
		on_click: None,
	}
}

impl IconButton {
	pub fn diameter(mut self, diameter: Pixels) -> Self {
		self.diameter = diameter;
		self
	}

	/// Whether the button responds. Enabled by default.
	pub fn enabled(mut self, enabled: bool) -> Self {
		self.enabled = enabled;
		self
	}

	/// The inverse of [`IconButton::enabled`].
	pub fn disabled(self, disabled: bool) -> Self {
		self.enabled(!disabled)
	}

	/// Opacity of the button's own glass and border, 1 by default. At 0 only the icon remains,
	/// for a button that sits inside a larger surface (see [`crate::attached`]).
	pub fn surface_opacity(mut self, opacity: f32) -> Self {
		self.surface = opacity.clamp(0., 1.);
		self
	}

	/// Forces the pointer light on or off. Without it the button follows
	/// [`Style::pointer_glow`](crate::style::Style::pointer_glow).
	pub fn glow(mut self, glow: bool) -> Self {
		self.glow = Some(glow);
		self
	}

	pub fn on_click(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
		self.on_click = Some(Box::new(handler));
		self
	}
}

impl RenderOnce for IconButton {
	fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let state = ControlState::of(&self.id, window, cx);
		let focus = focus_handle(&self.id, window, cx);
		let enabled = self.enabled;
		let frame = ControlState::frame(
			&state,
			Feedback::Standard,
			enabled,
			theme.reduce_motion,
			window,
			cx,
		);
		let glows = enabled && glow_enabled(&theme, self.glow, false);
		let glass = theme
			.hud_surface()
			.overlay(scale_alpha(theme.ink(control::HOVER_OPACITY), frame.light));
		let glass = match frame.pointer {
			Some(point) if glows => glass
				.pointer(Some(point), with_alpha(theme.palette.primary_text, frame.light))
				.pointer_radius(frame.pointer_radius),
			_ => glass,
		};
		let border = glass.border().unwrap_or(theme.hud_border());
		let radius = theme.style.radii.capsule_radius(self.diameter);
		let surface = layer()
			.rounded(radius)
			.shadow(frame.shadows(glass.shadows()))
			.border_1()
			.border_color(border);
		let surface = match glass.filter() {
			Some(filter) => surface.backdrop_filter(vec![Filter::Glass(filter)]),
			None => surface.bg(glass.fill()),
		};
		// `IconActionButton` scales the glyph with the button: 17 pt at 36 pt, clamped 15–20.
		let glyph = (f32::from(self.diameter) / 36. * 17.).clamp(15., 20.);
		let hit_target = px(f32::from(self.diameter).max(44.));
		div()
			.size(hit_target)
			.flex()
			.items_center()
			.justify_center()
			.child(
				div()
					.id(self.id)
					.relative()
					.size(self.diameter)
					.flex()
					.items_center()
					.justify_center()
					.role(Role::Button)
					.aria_label(self.label)
					.opacity(frame.press_opacity())
					.when(self.surface > 0., |button| {
						button.child(surface.opacity(self.surface))
					})
					.child(
						div()
							.relative()
							.child(self.icon.render(px(glyph), theme.palette.primary_text)),
					)
					.when(enabled, |button| {
						button.control_state(&state, glows).keyboard_focus(
							&focus,
							FocusShape::Rounded(radius),
							&theme,
							window,
						)
					})
					.when(!enabled, |button| {
						button
							.aria_disabled(true)
							.cursor_default()
							.opacity(control::DISABLED_FOREGROUND_OPACITY)
					})
					.when_some(self.on_click.filter(|_| enabled), |button, on_click| {
						button.on_click(on_click)
					})
					.scaled(frame.scale),
			)
	}
}

/// The still accent halo behind the primary operation, with one arrival pulse.
#[derive(IntoElement)]
pub struct Halo {
	child: AnyElement,
	generation: usize,
	theme: Option<Theme>,
}

/// Wraps the primary action in the accent halo. It is the only ambient effect on the action.
pub fn halo(child: impl IntoElement) -> Halo {
	Halo {
		child: child.into_any_element(),
		generation: 0,
		theme: None,
	}
}

impl Halo {
	/// Changing the generation plays the arrival pulse again.
	pub fn generation(mut self, generation: usize) -> Self {
		self.generation = generation;
		self
	}
}

impl RenderOnce for Halo {
	fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let accent = theme.accent;
		let resting = 0.5;
		let halo_radius = theme.style.radii.capsule_radius(theme.px(control::HEIGHT_LARGE));
		let glow = move |intensity: f32| {
			let spread = px(-(2. + 6. * intensity));
			div()
				.absolute()
				.top(spread)
				.bottom(spread)
				.left(spread)
				.right(spread)
				.rounded(halo_radius)
				.bg(accent)
				.filter(vec![Filter::Blur(px(12. + 8. * intensity))])
				.opacity(0.18 + 0.22 * intensity)
		};
		let reduced = theme.reduce_transparency();
		let pulses = theme.decorative_motion(window);
		div()
			.relative()
			.when(!reduced, |this| {
				if pulses {
					this.child(glow(resting).with_animation(
						("calcite-halo", self.generation),
						Animation::new(Duration::from_secs_f32(cycle::BREATHING)).with_easing(ease_in_out),
						move |element, t| {
							let intensity = resting + (1. - resting) * (std::f32::consts::PI * t).sin();
							let spread = px(-(2. + 6. * intensity));
							element
								.top(spread)
								.bottom(spread)
								.left(spread)
								.right(spread)
								.filter(vec![Filter::Blur(px(12. + 8. * intensity))])
								.opacity(0.18 + 0.22 * intensity)
						},
					))
				} else {
					this.child(glow(resting))
				}
			})
			.child(self.child)
	}
}

/// Components that accept a theme override with `.theme(theme)`.
pub trait ThemeOverride: Sized {
	fn theme(self, theme: Theme) -> Self;
}

/// Implements [`ThemeOverride`] for components with a `theme: Option<Theme>` field.
macro_rules! theme_override {
	($($component:ty),* $(,)?) => {
		$(impl $crate::controls::ThemeOverride for $component {
			fn theme(mut self, theme: $crate::theme::Theme) -> Self {
				self.theme = Some(theme);
				self
			}
		})*
	};
}

pub(crate) use theme_override;

theme_override!(Button, IconButton, Halo);

#[cfg(test)]
mod tests {
	use std::mem::discriminant;

	use super::*;
	use crate::{palette::Palette, surface::Paint, tokens::rgba};

	const STEPS: usize = 40;

	#[test]
	fn all_buttons_are_capsules() {
		for size in [Size::Compact, Size::Regular, Size::Large] {
			assert!(size.corner(&Theme::default()) >= size.height() / 2.);
		}
	}

	#[test]
	fn a_prominent_label_reads_on_its_fill_in_both_palettes() {
		for palette in [Palette::default(), Palette::LIGHT] {
			let theme = Theme::new(palette);
			for tone in [Tone::Neutral, Tone::Accent, Tone::Warning, Tone::Danger] {
				let fill = tone.prominent_fill(&theme);
				assert_eq!(fill.alpha, 1.);
				assert!(tokens::contrast_ratio(tokens::on_color(fill), fill) >= 3.);
			}
			assert_eq!(Tone::Neutral.prominent_fill(&theme), with_alpha(theme.accent, 1.));
		}
	}

	/// A palette whose control tint is chromatic (chroma 0.12), like the preview's Ember.
	fn warm() -> Palette {
		Palette {
			control: rgba(0.78, 0.72, 0.66, 1.),
			quiet_control: rgba(0.64, 0.58, 0.52, 1.),
			..Palette::default()
		}
	}

	type Resolved = (Paint, Rgba, Rgba, Rgba);

	/// Everything a button of `tone` draws from the theme: paint kind, fill, border, label.
	fn resolved(tone: Tone, theme: &Theme) -> Resolved {
		let surface = Surface::resolve(tone.surface(theme), tone.chromatic(theme), true, theme);
		let fill = match surface.paint {
			Paint::Flat(fill) | Paint::Plain(fill) => fill,
			Paint::Glass(glass) => glass.fill(),
		};
		(surface.paint, fill, surface.border, tone.label(theme))
	}

	fn channels(color: Rgba) -> [f32; 4] {
		[color.red, color.green, color.blue, color.alpha]
	}

	/// Samples `from.mix(to, t)` and checks that the paint kind never changes and every color
	/// channel moves in one direction by at most 1.5 times the even step.
	fn assert_smooth(tone: Tone, from: &Theme, to: &Theme) {
		let samples: Vec<Resolved> = (0..=STEPS)
			.map(|i| resolved(tone, &from.mix(to, i as f32 / STEPS as f32)))
			.collect();
		for pair in samples.windows(2) {
			assert_eq!(
				discriminant(&pair[0].0),
				discriminant(&pair[1].0),
				"{tone:?}: paint kind flipped mid-crossfade"
			);
		}
		let picks: [fn(&Resolved) -> Rgba; 3] = [|s| s.1, |s| s.2, |s| s.3];
		for (index, pick) in picks.into_iter().enumerate() {
			for channel in 0..4 {
				let start = channels(pick(&samples[0]))[channel];
				let end = channels(pick(&samples[STEPS]))[channel];
				let budget = (end - start).abs() / STEPS as f32 * 1.5 + 1e-4;
				for pair in samples.windows(2) {
					let step = channels(pick(&pair[1]))[channel] - channels(pick(&pair[0]))[channel];
					assert!(
						step.abs() <= budget && step * (end - start) >= -1e-6,
						"{tone:?}: color {index} channel {channel} jumped by {step} (budget {budget})"
					);
				}
			}
		}
	}

	#[test]
	fn neutral_ignores_accent_changes() {
		let base = Theme::default();
		let a = base.with_accent(Some(rgba(0.96, 0.62, 0.22, 1.)));
		let b = base.with_accent(Some(rgba(0.62, 0.50, 0.98, 1.)));
		let resting = resolved(Tone::Neutral, &a);
		for i in 0..=STEPS {
			let mid = resolved(Tone::Neutral, &a.mix(&b, i as f32 / STEPS as f32));
			assert_eq!(discriminant(&mid.0), discriminant(&resting.0));
			assert_eq!((mid.1, mid.2, mid.3), (resting.1, resting.2, resting.3));
		}
	}

	#[test]
	fn neutral_crossfades_smoothly_across_palettes() {
		let from = Theme::default();
		let to = Theme::default().with_palette(warm());
		assert_smooth(Tone::Neutral, &from, &to);
		assert_smooth(Tone::Neutral, &to, &from);
	}

	#[test]
	fn neutral_keeps_one_paint_kind_for_any_palette() {
		for palette in [Palette::default(), warm()] {
			let theme = Theme::default().with_palette(palette);
			assert!(matches!(resolved(Tone::Neutral, &theme).0, Paint::Flat(_)));
		}
	}

	#[test]
	fn accent_tones_crossfade_smoothly_between_accents() {
		let from = Theme::default().with_accent(Some(rgba(0.96, 0.62, 0.22, 1.)));
		let to = Theme::default().with_accent(Some(rgba(0.94, 0.36, 0.52, 1.)));
		assert_smooth(Tone::Accent, &from, &to);
		assert_smooth(Tone::Warning, &from, &to);
		assert_smooth(Tone::Danger, &from, &to);
	}
}
