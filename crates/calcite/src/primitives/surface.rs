// SPDX-License-Identifier: MPL-2.0

//! The control surface: how a tone paints its glass, flat, or plain fill, and the compact capsule
//! shared by chips and menu pickers.

use gpui::{ElementId, Filter, FontWeight, Rgba, Styled, div, prelude::*, px};

use crate::{
	glass::{Elevation, Glass, flat_surface},
	interaction::ControlFrame,
	theme::Theme,
	tokens::{self, Chromatic, control, over, scale_alpha, with_alpha},
};

/// How one control state paints its surface (`adaptiveControlSurface`).
#[derive(Clone, Copy)]
pub(crate) enum Paint {
	/// A dynamic accent: chromatic Liquid Glass.
	Glass(Glass),
	/// Secondary actions: a quiet translucent neutral fill, no lensing.
	Flat(Rgba),
	/// Reduce Transparency or disabled: an opaque or plain fill.
	Plain(Rgba),
}

#[derive(Clone, Copy)]
pub(crate) struct Surface {
	pub(crate) paint: Paint,
	pub(crate) hover_overlay: Rgba,
	pub(crate) border: Rgba,
}

impl Surface {
	pub(crate) fn resolve(tint: Rgba, chromatic: bool, enabled: bool, theme: &Theme) -> Self {
		let reduced = theme.reduce_transparency();
		if !enabled {
			let fill = if reduced {
				theme.palette.opaque_disabled_surface
			} else if chromatic {
				Chromatic::disabled_with(tint, theme.palette.ink).fill
			} else {
				theme.ink(control::DISABLED_NEUTRAL_SURFACE_OPACITY)
			};
			let border = if chromatic {
				Chromatic::disabled_with(tint, theme.palette.ink).border
			} else {
				theme.ink(control::DISABLED_NEUTRAL_BORDER_OPACITY)
			};
			return Self {
				paint: Paint::Plain(fill),
				hover_overlay: tokens::CLEAR,
				border,
			};
		}
		if chromatic && !reduced {
			let accent = Chromatic::enabled(tint);
			let glass = theme.surface(Some(accent.fill)).elevation(Elevation::None);
			return Self {
				paint: Paint::Glass(glass),
				hover_overlay: with_alpha(tint, control::HOVER_OPACITY),
				border: accent.border,
			};
		}
		let border_opacity = if chromatic {
			control::ENABLED_CHROMATIC_BORDER_OPACITY
		} else {
			control::ENABLED_NEUTRAL_BORDER_OPACITY
		};
		let paint = if reduced {
			Paint::Plain(theme.palette.opaque_surface)
		} else {
			Paint::Flat(theme.ink(control::ENABLED_NEUTRAL_SURFACE_OPACITY))
		};
		Self {
			paint,
			hover_overlay: theme.ink(control::HOVER_OPACITY),
			border: with_alpha(tint, border_opacity),
		}
	}

	/// A solid fill with a label-colored hover wash and no border: the prominent button.
	pub(crate) fn solid(fill: Rgba, label: Rgba) -> Self {
		Self {
			paint: Paint::Plain(fill),
			hover_overlay: with_alpha(label, control::HOVER_OPACITY),
			border: tokens::CLEAR,
		}
	}

	/// Paints the surface for this frame. The hover overlay and the pointer light fade in
	/// with `frame.light`.
	pub(crate) fn paint<S: Styled>(&self, styled: S, frame: &ControlFrame, pointer_tint: Option<Rgba>) -> S {
		let overlay = scale_alpha(self.hover_overlay, frame.light);
		let pointer =
			pointer_tint.and_then(|tint| frame.pointer.map(|point| (point, with_alpha(tint, frame.light))));
		match self.paint {
			Paint::Glass(glass) => {
				let glass = match pointer {
					Some((point, tint)) => glass
						.pointer(Some(point), tint)
						.pointer_radius(frame.pointer_radius),
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
					frame.pointer_radius,
				))])
			}
			Paint::Plain(fill) => styled.bg(over(overlay, fill)),
		}
	}
}

/// The compact capsule of a chip and a menu picker: one padding, gap, and caption text size.
/// The caller adds the surface, border, and content.
pub(crate) fn chip_capsule(id: impl Into<ElementId>) -> gpui::Stateful<gpui::Div> {
	div()
		.id(id)
		.relative()
		.flex()
		.flex_row()
		.items_center()
		.gap(px(control::CHIP_GAP))
		.px(px(control::COMPACT_HORIZONTAL_PADDING))
		.py(px(control::COMPACT_VERTICAL_PADDING))
		.rounded_full()
		.text_size(tokens::text::CAPTION)
		.font_weight(FontWeight::SEMIBOLD)
}
