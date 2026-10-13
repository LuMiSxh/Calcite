// SPDX-License-Identifier: MPL-2.0

//! Shared actions: `CapsuleActionButton`, `IconActionButton`, and the accent halo.

use std::time::Duration;

use gpui::{
	Animation, AnimationExt, App, ClickEvent, ElementId, Filter, FontWeight, IntoElement, ParentElement,
	Pixels, Point, Rgba, SharedString, StyleRefinement, Styled, Window, div, ease_in_out, prelude::*, px,
};

use crate::{
	Theme,
	glass::{Elevation, Glass, flat_surface},
	icons::Icon,
	motion::{Curve, cycle},
	tokens::{self, control, is_chromatic, over, scale_alpha, white, with_alpha},
};

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tone {
	Accent(Rgba),
	Neutral,
	Warning,
	Danger,
}

impl Tone {
	fn foreground(self) -> Rgba {
		match self {
			Self::Accent(color) => color,
			Self::Neutral => tokens::CONTROL_TINT,
			Self::Warning => tokens::WARNING,
			Self::Danger => tokens::DANGER_FOREGROUND,
		}
	}

	fn surface(self) -> Rgba {
		match self {
			Self::Accent(color) => color,
			Self::Neutral => tokens::CONTROL_TINT,
			Self::Warning => tokens::WARNING,
			Self::Danger => tokens::DANGER,
		}
	}

	fn disabled_foreground(self) -> Rgba {
		match self {
			Self::Neutral => white(control::DISABLED_NEUTRAL_FOREGROUND_OPACITY),
			_ => with_alpha(self.foreground(), control::DISABLED_FOREGROUND_OPACITY),
		}
	}
}

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

	/// The resting style and the hover style. The hover style carries the pointer light.
	fn styles(&self, pointer: Option<Point<Pixels>>, pointer_tint: Rgba) -> (Painter, Painter) {
		match self.paint {
			Paint::Glass(glass) => (
				Painter::Filter(glass.filter()),
				Painter::Filter(
					glass
						.overlay(self.hover_overlay)
						.pointer(pointer, pointer_tint)
						.filter(),
				),
			),
			Paint::Flat(fill) => (
				Painter::Filter(Some(flat_surface(fill, None, pointer_tint))),
				Painter::Filter(Some(flat_surface(
					over(self.hover_overlay, fill),
					pointer,
					pointer_tint,
				))),
			),
			Paint::Plain(fill) => (Painter::Fill(fill), Painter::Fill(over(self.hover_overlay, fill))),
		}
	}
}

#[derive(Clone)]
enum Painter {
	Filter(Option<gpui::GlassFilter>),
	Fill(Rgba),
}

impl Painter {
	fn apply<S: Styled>(self, styled: S) -> S {
		match self {
			Self::Filter(Some(filter)) => styled.backdrop_filter(vec![Filter::Glass(filter)]),
			Self::Filter(None) => styled,
			Self::Fill(fill) => styled.bg(fill),
		}
	}

	fn refine(self, style: StyleRefinement) -> StyleRefinement {
		self.apply(style)
	}
}

/// A text action whose visible capsule and hit target share the same bounds. Standard and
/// large actions carry the pointer light (`PointerSheen`).
#[derive(IntoElement)]
pub struct CapsuleButton {
	id: ElementId,
	title: SharedString,
	icon: Option<Icon>,
	tone: Tone,
	size: Size,
	enabled: bool,
	theme: Theme,
	on_click: Option<ClickHandler>,
}

impl CapsuleButton {
	pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>, tone: Tone, theme: Theme) -> Self {
		Self {
			id: id.into(),
			title: title.into(),
			icon: None,
			tone,
			size: Size::Regular,
			enabled: true,
			theme,
			on_click: None,
		}
	}

	pub fn icon(mut self, icon: Icon) -> Self {
		self.icon = Some(icon);
		self
	}

	pub fn size(mut self, size: Size) -> Self {
		self.size = size;
		self
	}

	pub fn enabled(mut self, enabled: bool) -> Self {
		self.enabled = enabled;
		self
	}

	pub fn on_click(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
		self.on_click = Some(Box::new(handler));
		self
	}
}

impl RenderOnce for CapsuleButton {
	fn render(self, window: &mut Window, _cx: &mut App) -> impl IntoElement {
		let theme = self.theme;
		let surface = Surface::resolve(self.tone.surface(), self.enabled, &theme);
		let foreground = if self.enabled {
			tokens::contrast_tint(self.tone.foreground())
		} else {
			self.tone.disabled_foreground()
		};
		let (horizontal, vertical) = self.size.padding();
		let text_size = self.size.text_size();
		let has_sheen = self.enabled && self.size != Size::Compact && !theme.reduce_transparency();
		let pointer = has_sheen.then(|| window.mouse_position());
		let (resting, hovered) = surface.styles(pointer, self.tone.foreground());

		let mut button = resting.apply(
			div()
				.id(self.id)
				.flex()
				.flex_row()
				.items_center()
				.gap(px(6.))
				.px(horizontal)
				.py(vertical)
				.rounded_full()
				.text_size(text_size)
				.font_weight(FontWeight::SEMIBOLD)
				.text_color(foreground)
				.border_1()
				.border_color(surface.border),
		);
		if let Some(icon) = self.icon {
			button = button.child(icon.render(text_size, foreground));
		}
		button = button.child(self.title);
		if !self.enabled {
			return button.cursor_default();
		}
		let reduce_motion = theme.reduce_motion;
		let on_click = self.on_click;
		button
			.cursor_pointer()
			.transitions(move |transitions| {
				transitions
					.bg(Curve::Hover.transition(reduce_motion))
					.opacity(Curve::Release.transition(reduce_motion))
			})
			.hover(move |style| hovered.clone().refine(style))
			// gpui-ce has no element scale yet; the press reads as a dim instead of 0.95×.
			.active(|style| style.opacity(0.82))
			// The pointer light follows the cursor while it is over the action.
			.when(has_sheen, |button| {
				button.on_mouse_move(|_, window, _| window.refresh())
			})
			.when_some(on_click, |button, on_click| button.on_click(on_click))
	}
}

/// A circular icon-only action, used for the Settings control over the artwork.
#[derive(IntoElement)]
pub struct IconButton {
	id: ElementId,
	icon: Icon,
	diameter: Pixels,
	foreground: Rgba,
	theme: Theme,
	on_click: Option<ClickHandler>,
}

impl IconButton {
	pub fn hud(id: impl Into<ElementId>, icon: Icon, theme: Theme) -> Self {
		Self {
			id: id.into(),
			icon,
			diameter: px(40.),
			foreground: tokens::PRIMARY_TEXT,
			theme,
			on_click: None,
		}
	}

	pub fn on_click(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
		self.on_click = Some(Box::new(handler));
		self
	}
}

impl RenderOnce for IconButton {
	fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
		let reduce_motion = self.theme.reduce_motion;
		let glass = self.theme.surface(Some(self.theme.hud_tint));
		let hover = glass.overlay(white(control::HUD_HOVER_SURFACE_OPACITY));
		let (resting, hovered) = match (glass.filter(), hover.filter()) {
			(Some(resting), Some(hovered)) => {
				(Painter::Filter(Some(resting)), Painter::Filter(Some(hovered)))
			}
			_ => (Painter::Fill(glass.fill()), Painter::Fill(hover.fill())),
		};
		// `IconActionButton` scales the glyph with the button: 17 pt at 36 pt, clamped 15–20.
		let glyph = (f32::from(self.diameter) / 36. * 17.).clamp(15., 20.);
		let on_click = self.on_click;
		div().size(px(44.)).flex().items_center().justify_center().child(
			resting
				.apply(
					div()
						.id(self.id)
						.size(self.diameter)
						.rounded_full()
						.flex()
						.items_center()
						.justify_center()
						.shadow(glass.shadows())
						.border_1()
						.border_color(
							glass
								.border()
								.unwrap_or(scale_alpha(tokens::HAIRLINE, control::HUD_BORDER_OPACITY)),
						),
				)
				.child(self.icon.render(px(glyph), self.foreground))
				.cursor_pointer()
				.transitions(move |transitions| transitions.opacity(Curve::Release.transition(reduce_motion)))
				.hover(move |style| hovered.clone().refine(style))
				.active(|style| style.opacity(0.82))
				.when_some(on_click, |button, on_click| button.on_click(on_click)),
		)
	}
}

/// The still accent halo behind the primary operation, with one arrival pulse.
/// `generation` restarts the pulse; the halo is the only ambient effect on the action.
pub fn accent_halo(
	accent: Rgba,
	generation: usize,
	theme: &Theme,
	child: impl IntoElement,
) -> impl IntoElement {
	let resting = 0.5;
	let halo = |intensity: f32| {
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
	let pulse = (!theme.reduce_motion && !reduced).then(|| {
		halo(resting).with_animation(
			("accent-halo", generation),
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
		)
	});
	div()
		.relative()
		.when(!reduced, |this| match pulse {
			Some(pulse) => this.child(pulse),
			None => this.child(halo(resting)),
		})
		.child(child)
}
