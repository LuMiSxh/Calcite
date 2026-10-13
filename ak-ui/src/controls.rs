// SPDX-License-Identifier: MPL-2.0

//! Shared actions: `CapsuleActionButton`, `IconActionButton`, and the accent halo.

use std::time::Duration;

use gpui::{
	Animation, AnimationExt, App, ClickEvent, ElementId, Filter, FontWeight, IntoElement, ParentElement,
	Pixels, Rgba, SharedString, Styled, Window, div, ease_in_out, prelude::*, px,
};

use crate::{
	Appearance,
	glass::{Elevation, Glass, GlassExt},
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

/// Surface colors for one control state, resolved from `adaptiveControlSurface`.
struct SurfaceColors {
	glass: Option<Glass>,
	fill: Rgba,
	hover_fill: Rgba,
	border: Rgba,
}

fn control_surface(tint: Rgba, enabled: bool, appearance: &Appearance) -> SurfaceColors {
	let chromatic = is_chromatic(tint);
	if !enabled {
		let fill = if appearance.reduce_transparency {
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
		return SurfaceColors {
			glass: None,
			fill,
			hover_fill: fill,
			border,
		};
	}
	if chromatic {
		// A dynamic accent gets a chromatic Liquid Glass surface.
		let glass = Glass::new(
			Some(with_alpha(tint, control::ENABLED_CHROMATIC_SURFACE_OPACITY)),
			appearance.treatment(),
		)
		.elevation(Elevation::None);
		let fill = glass.fill();
		return SurfaceColors {
			glass: Some(glass),
			fill,
			hover_fill: over(with_alpha(tint, 0.05), fill),
			border: with_alpha(tint, control::ENABLED_CHROMATIC_BORDER_OPACITY),
		};
	}
	// Secondary actions use a quiet filled neutral surface, not glass.
	let fill = if appearance.reduce_transparency {
		control::OPAQUE_NEUTRAL_SURFACE
	} else {
		white(control::ENABLED_NEUTRAL_SURFACE_OPACITY)
	};
	SurfaceColors {
		glass: None,
		fill,
		hover_fill: over(white(0.04), fill),
		border: with_alpha(tint, control::ENABLED_NEUTRAL_BORDER_OPACITY),
	}
}

/// A text action whose visible capsule and hit target share the same bounds.
#[derive(IntoElement)]
pub struct CapsuleButton {
	id: ElementId,
	title: SharedString,
	icon: Option<Icon>,
	tone: Tone,
	size: Size,
	enabled: bool,
	appearance: Appearance,
	on_click: Option<ClickHandler>,
}

impl CapsuleButton {
	pub fn new(
		id: impl Into<ElementId>,
		title: impl Into<SharedString>,
		tone: Tone,
		appearance: Appearance,
	) -> Self {
		Self {
			id: id.into(),
			title: title.into(),
			icon: None,
			tone,
			size: Size::Regular,
			enabled: true,
			appearance,
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
	fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
		let reduce_motion = self.appearance.reduce_motion;
		let surface = control_surface(self.tone.surface(), self.enabled, &self.appearance);
		let foreground = if self.enabled {
			tokens::contrast_tint(self.tone.foreground())
		} else {
			self.tone.disabled_foreground()
		};
		let (horizontal, vertical) = self.size.padding();
		let text_size = self.size.text_size();

		let mut button = div()
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
			.border_color(surface.border);
		button = match surface.glass {
			Some(glass) => button.glass(glass),
			None => button.bg(surface.fill),
		};
		if let Some(icon) = self.icon {
			button = button.child(icon.render(text_size, foreground));
		}
		button = button.child(self.title);

		if !self.enabled {
			return button.cursor_default();
		}
		let hover_fill = surface.hover_fill;
		let on_click = self.on_click;
		button
			.cursor_pointer()
			.transitions(move |transitions| {
				transitions
					.bg(Curve::Hover.transition(reduce_motion))
					.opacity(Curve::Release.transition(reduce_motion))
			})
			.hover(move |style| style.bg(hover_fill))
			// gpui-ce has no element scale yet; the press reads as a dim instead of 0.95×.
			.active(|style| style.opacity(0.78))
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
	appearance: Appearance,
	on_click: Option<ClickHandler>,
}

impl IconButton {
	pub fn hud(id: impl Into<ElementId>, icon: Icon, appearance: Appearance) -> Self {
		Self {
			id: id.into(),
			icon,
			diameter: px(40.),
			foreground: tokens::PRIMARY_TEXT,
			appearance,
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
		let reduce_motion = self.appearance.reduce_motion;
		let glass = Glass::new(Some(self.appearance.hud_tint), self.appearance.treatment());
		let hover_fill = glass.hovered(true).fill();
		// `IconActionButton` scales the glyph with the button: 17 pt at 36 pt, clamped 15–20.
		let glyph = (f32::from(self.diameter) / 36. * 17.).clamp(15., 20.);
		let on_click = self.on_click;
		div().size(px(44.)).flex().items_center().justify_center().child(
			div()
				.id(self.id)
				.size(self.diameter)
				.rounded_full()
				.flex()
				.items_center()
				.justify_center()
				.glass(glass)
				.border_1()
				.border_color(scale_alpha(tokens::HAIRLINE, control::HUD_BORDER_OPACITY))
				.child(self.icon.render(px(glyph), self.foreground))
				.cursor_pointer()
				.transitions(move |transitions| {
					transitions
						.bg(Curve::Hover.transition(reduce_motion))
						.opacity(Curve::Release.transition(reduce_motion))
				})
				.hover(move |style| style.bg(hover_fill))
				.active(|style| style.opacity(0.78))
				.when_some(on_click, |button, on_click| button.on_click(on_click)),
		)
	}
}

/// The still accent halo behind the primary operation, with one arrival pulse.
/// `generation` restarts the pulse; the halo is the only ambient effect on the action.
pub fn accent_halo(
	accent: Rgba,
	generation: usize,
	appearance: &Appearance,
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
	let pulse = if appearance.reduce_motion || appearance.reduce_transparency {
		None
	} else {
		Some(halo(resting).with_animation(
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
		))
	};
	div()
		.relative()
		.when(!appearance.reduce_transparency, |this| match pulse {
			Some(pulse) => this.child(pulse),
			None => this.child(halo(resting)),
		})
		.child(child)
}
