// SPDX-License-Identifier: MPL-2.0

//! Effects: a radial gradient fill, a drop shadow that follows the alpha of the content, and a
//! blurred text shadow.

use calcite::{
	prelude::*,
	tokens::{radius, scale_alpha, white},
};
use gpui::{IntoElement, ParentElement, Styled, div, px};

use super::states::{cell, stack, state_row};

pub fn effects_section(theme: &Theme) -> impl IntoElement {
	let accent = theme.accent;
	let swatch = |fill| div().w(px(150.)).h(px(90.)).rounded(radius::CONTROL).bg(fill);
	stack(vec![
		state_row(
			"Radial gradient",
			"center relative to the box, radius as a fraction of the shorter side",
			"radial_gradient(point(0.5, 0.5), 0.6, from, to)",
			vec![
				cell(
					"center, radius 0.6",
					swatch(radial_gradient(
						point(0.5, 0.5),
						0.6,
						scale_alpha(accent, 0.95),
						scale_alpha(accent, 0.),
					)),
				),
				cell(
					"top left, radius 1.1",
					swatch(radial_gradient(
						point(0.15, 0.2),
						1.1,
						white(0.55),
						scale_alpha(accent, 0.25),
					)),
				),
			],
		),
		state_row(
			"Drop shadow",
			"follows the alpha of the content, not the box",
			"div().drop_shadow(point(px(0.), px(4.)), px(8.), color).child(content)",
			vec![
				cell(
					"icon",
					div()
						.p(px(12.))
						.drop_shadow(point(px(0.), px(4.)), px(8.), scale_alpha(accent, 0.9))
						.child(icon(Icon::Gear).size(px(48.)).text_color(white(1.))),
				),
				cell(
					"text",
					div()
						.p(px(12.))
						.drop_shadow(point(px(3.), px(3.)), px(0.), scale_alpha(accent, 0.8))
						.text_size(px(28.))
						.text_color(white(1.))
						.child("Calcite"),
				),
			],
		),
		state_row(
			"Text shadow",
			"a blurred copy of the text, behind it",
			"text_shadow(text).offset(point(px(0.), px(2.))).blur(px(4.)).color(color)",
			vec![
				cell(
					"default",
					div()
						.p(px(8.))
						.text_size(px(28.))
						.text_color(white(1.))
						.child(text_shadow("Calcite")),
				),
				cell(
					"glow",
					div().p(px(8.)).text_size(px(28.)).text_color(white(1.)).child(
						text_shadow("Calcite")
							.offset(point(px(0.), px(0.)))
							.blur(px(6.))
							.color(scale_alpha(accent, 1.)),
					),
				),
			],
		),
	])
}
