// SPDX-License-Identifier: MPL-2.0

//! Actions: [`button`] (`CapsuleActionButton`), [`icon_button`] (`IconActionButton`), and
//! [`halo`] (`AccentHalo`).
//!
//! ```ignore
//! button("play", "Play").accent().large().icon(Icon::Play).on_click(cx.listener(|this, _, _, cx| ..))
//! icon_button("settings", Icon::Gear, "Settings").on_click(..)
//! ```

use std::time::Duration;

use gpui::{
	Animation, AnimationExt, AnyElement, App, ClickEvent, ElementId, Filter, FontWeight, IntoElement,
	ParentElement, Pixels, Rgba, Role, SharedString, Styled, Window, div, ease_in_out, prelude::*, px,
};

use crate::{
	glass::{Elevation, Glass, flat_surface},
	icons::IconRef,
	interaction::{ControlFrame, ControlInteraction, ControlState, Feedback, layer},
	motion::cycle,
	theme::{ActiveTheme, Theme},
	tokens::{self, control, is_chromatic, over, scale_alpha, white, with_alpha},
};

pub(crate) type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// The semantic tone of an action.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tone {
	/// The theme's dynamic accent: the current primary operation.
	Accent,
	/// A fixed chromatic color.
	Color(Rgba),
	/// Secondary actions: quiet neutral surface.
	Neutral,
	Warning,
	Danger,
}

impl Tone {
	fn foreground(self, theme: &Theme) -> Rgba {
		match self {
			Self::Accent => theme.accent,
			Self::Color(color) => color,
			Self::Neutral => tokens::CONTROL_TINT,
			Self::Warning => tokens::WARNING,
			Self::Danger => tokens::DANGER_FOREGROUND,
		}
	}

	fn surface(self, theme: &Theme) -> Rgba {
		match self {
			Self::Danger => tokens::DANGER,
			_ => self.foreground(theme),
		}
	}

	fn disabled_foreground(self, theme: &Theme) -> Rgba {
		match self {
			Self::Neutral => white(control::DISABLED_NEUTRAL_FOREGROUND_OPACITY),
			_ => with_alpha(self.foreground(theme), control::DISABLED_FOREGROUND_OPACITY),
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
	fn padding(self) -> (Pixels, Pixels) {
		match self {
			Self::Regular => (px(12.), px(6.)),
			Self::Large => (px(16.), px(8.)),
			Self::Compact => (px(10.), px(5.)),
		}
	}

	fn text_size(self) -> Pixels {
		match self {
			Self::Compact => tokens::text::CAPTION,
			_ => tokens::text::BODY,
		}
	}
}

/// How one control state paints its surface (`adaptiveControlSurface`).
#[derive(Clone, Copy)]
enum Paint {
	/// A dynamic accent: chromatic Liquid Glass.
	Glass(Glass),
	/// Secondary actions: a quiet translucent neutral fill, no lensing.
	Flat(Rgba),
	/// Reduce Transparency or disabled: an opaque or plain fill.
	Plain(Rgba),
}

#[derive(Clone, Copy)]
struct Surface {
	paint: Paint,
	hover_overlay: Rgba,
	border: Rgba,
}

impl Surface {
	fn resolve(tint: Rgba, enabled: bool, theme: &Theme) -> Self {
		let chromatic = is_chromatic(tint);
		let reduced = theme.reduce_transparency();
		if !enabled {
			let fill = if reduced {
				control::OPAQUE_DISABLED_SURFACE
			} else if chromatic {
				white(control::DISABLED_CHROMATIC_SURFACE_OPACITY)
			} else {
				white(control::DISABLED_NEUTRAL_SURFACE_OPACITY)
			};
			let border = if chromatic {
				with_alpha(tint, control::DISABLED_CHROMATIC_BORDER_OPACITY)
			} else {
				white(control::DISABLED_NEUTRAL_BORDER_OPACITY)
			};
			return Self {
				paint: Paint::Plain(fill),
				hover_overlay: tokens::CLEAR,
				border,
			};
		}
		if chromatic && !reduced {
			let glass = theme
				.surface(Some(with_alpha(tint, control::ENABLED_CHROMATIC_SURFACE_OPACITY)))
				.elevation(Elevation::None);
			return Self {
				paint: Paint::Glass(glass),
				hover_overlay: with_alpha(tint, 0.05),
				border: with_alpha(tint, control::ENABLED_CHROMATIC_BORDER_OPACITY),
			};
		}
		let border_opacity = if chromatic {
			control::ENABLED_CHROMATIC_BORDER_OPACITY
		} else {
			control::ENABLED_NEUTRAL_BORDER_OPACITY
		};
		let paint = if reduced {
			Paint::Plain(control::OPAQUE_NEUTRAL_SURFACE)
		} else {
			Paint::Flat(white(control::ENABLED_NEUTRAL_SURFACE_OPACITY))
		};
		Self {
			paint,
			hover_overlay: white(0.04),
			border: with_alpha(tint, border_opacity),
		}
	}

	/// Paints the surface for this frame. The hover overlay and the pointer light fade in
	/// with `frame.light`.
	fn paint<S: Styled>(&self, styled: S, frame: &ControlFrame, pointer_tint: Option<Rgba>) -> S {
		let overlay = scale_alpha(self.hover_overlay, frame.light);
		let pointer =
			pointer_tint.and_then(|tint| frame.pointer.map(|point| (point, scale_alpha(tint, frame.light))));
		match self.paint {
			Paint::Glass(glass) => {
				let glass = match pointer {
					Some((point, tint)) => glass.pointer(Some(point), tint),
					None => glass,
				};
				let glass = glass.overlay(overlay);
				match glass.filter() {
					Some(filter) => styled.backdrop_filter(vec![Filter::Glass(filter)]),
					None => styled.bg(glass.fill()),
				}
			}
			Paint::Flat(fill) => {
				let (point, tint) = match pointer {
					Some((point, tint)) => (Some(point), tint),
					None => (None, tokens::CLEAR),
				};
				styled.backdrop_filter(vec![Filter::Glass(flat_surface(
					over(overlay, fill),
					point,
					tint,
				))])
			}
			Paint::Plain(fill) => styled.bg(over(overlay, fill)),
		}
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
	theme: Option<Theme>,
	on_click: Option<ClickHandler>,
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

	pub fn enabled(mut self, enabled: bool) -> Self {
		self.enabled = enabled;
		self
	}

	pub fn disabled(self, disabled: bool) -> Self {
		self.enabled(!disabled)
	}

	pub fn on_click(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
		self.on_click = Some(Box::new(handler));
		self
	}
}

impl RenderOnce for Button {
	fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let state = ControlState::of(&self.id, window, cx);
		let frame = ControlState::frame(
			&state,
			Feedback::Standard,
			self.enabled,
			theme.reduce_motion,
			window,
			cx,
		);
		let surface = Surface::resolve(self.tone.surface(&theme), self.enabled, &theme);
		let foreground = if self.enabled {
			tokens::contrast_tint(self.tone.foreground(&theme))
		} else {
			self.tone.disabled_foreground(&theme)
		};
		let (horizontal, vertical) = self.size.padding();
		let text_size = self.size.text_size();
		let has_sheen = self.enabled && self.size != Size::Compact && !theme.reduce_transparency();
		let pointer_tint = has_sheen.then(|| self.tone.foreground(&theme));

		let surface_layer = surface
			.paint(layer(frame.surface_insets()), &frame, pointer_tint)
			.rounded_full()
			.border_1()
			.border_color(surface.border);
		let content = div()
			.relative()
			.flex()
			.flex_row()
			.items_center()
			.gap(px(6.))
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
			.py(vertical)
			.text_size(text_size)
			.font_weight(FontWeight::SEMIBOLD)
			.text_color(foreground)
			.role(Role::Button)
			.aria_label(self.title)
			.child(surface_layer)
			.child(content);
		if !self.enabled {
			return button.cursor_default();
		}
		button
			.cursor_pointer()
			// `ActionPressStyle`: opacity 0.86 while pressed; the surface carries the scale.
			.opacity(if frame.pressed { 0.86 } else { 1. })
			.control_state(&state, has_sheen)
			.when_some(self.on_click, |button, on_click| button.on_click(on_click))
	}
}

/// A circular icon-only action, used for the Settings control over the artwork.
#[derive(IntoElement)]
pub struct IconButton {
	id: ElementId,
	icon: IconRef,
	label: SharedString,
	diameter: Pixels,
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
		theme: None,
		on_click: None,
	}
}

impl IconButton {
	pub fn diameter(mut self, diameter: Pixels) -> Self {
		self.diameter = diameter;
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
		let frame = ControlState::frame(&state, Feedback::Standard, true, theme.reduce_motion, window, cx);
		let glass = theme.hud_surface().overlay(scale_alpha(
			white(control::HUD_HOVER_SURFACE_OPACITY),
			frame.light,
		));
		let border = glass
			.border()
			.unwrap_or(scale_alpha(tokens::HAIRLINE, control::HUD_BORDER_OPACITY));
		let surface = layer(frame.surface_insets())
			.rounded_full()
			.shadow(glass.shadows())
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
					.cursor_pointer()
					.role(Role::Button)
					.aria_label(self.label)
					.opacity(if frame.pressed { 0.86 } else { 1. })
					.child(surface)
					.child(
						div()
							.relative()
							.child(self.icon.render(px(glyph), tokens::PRIMARY_TEXT)),
					)
					.control_state(&state, false)
					.when_some(self.on_click, |button, on_click| button.on_click(on_click)),
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
		let glow = move |intensity: f32| {
			let spread = px(-(2. + 6. * intensity));
			div()
				.absolute()
				.top(spread)
				.bottom(spread)
				.left(spread)
				.right(spread)
				.rounded_full()
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
						("vitro-halo", self.generation),
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
