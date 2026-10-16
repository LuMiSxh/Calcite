// SPDX-License-Identifier: MPL-2.0

//! Paint effects from the patched gpui-ce: a radial gradient fill, an alpha-shaped drop shadow for
//! any element, and a blurred shadow for text.
//!
//! Wrap the content of an element with [`DropShadowExt::drop_shadow`] to give it a shadow that
//! follows the alpha of what it paints (an icon, an image, text), unlike `box-shadow`, which
//! follows the box. The element paints into an offscreen layer, so use it on the element that
//! has the content, not on a whole page.

use gpui::{
	App, Background, DropShadowFilter, Filter, IntoElement, ParentElement, Pixels, Point, RenderOnce, Rgba,
	SharedString, Styled, Window, div, linear_color_stop, px,
};

use crate::tokens::rgba;

/// A radial gradient fill: `from` at `center`, `to` at `radius`, and `to` beyond.
///
/// `center` is relative to the painted box (`point(0.5, 0.5)` is the middle). `radius` is a
/// fraction of the shorter side of the box, so `0.5` reaches the edge of a square.
pub fn radial_gradient(center: Point<f32>, radius: f32, from: Rgba, to: Rgba) -> Background {
	gpui::radial_gradient(
		center,
		radius,
		linear_color_stop(from, 0.),
		linear_color_stop(to, 1.),
	)
}

/// A drop shadow that follows the alpha of the element's painted content.
pub trait DropShadowExt: Styled + Sized {
	/// Draws a copy of the content under it, shifted by `offset`, blurred by `blur`, and tinted
	/// with `color`. Its alpha scales the shadow. The content stays sharp.
	///
	/// A `blur` on the same element does not blur the content: the larger radius blurs the shadow.
	fn drop_shadow(mut self, offset: Point<Pixels>, blur: impl Into<Pixels>, color: Rgba) -> Self {
		let shadow = DropShadowFilter::new(
			offset,
			blur.into(),
			[color.red, color.green, color.blue, color.alpha],
		);
		self.style()
			.filter
			.get_or_insert_with(Vec::new)
			.push(Filter::DropShadow(shadow));
		self
	}
}

impl<T: Styled> DropShadowExt for T {}

/// Text with a blurred shadow, from a blurred copy of the text. It takes the font and the color
/// of the surrounding text. The copy is one line long, so it works best for a single line.
pub fn text_shadow(text: impl Into<SharedString>) -> TextShadow {
	TextShadow {
		text: text.into(),
		offset: Point { x: px(0.), y: px(1.) },
		blur: px(3.),
		color: rgba(0., 0., 0., 0.45),
	}
}

/// The element of [`text_shadow`].
#[derive(IntoElement)]
pub struct TextShadow {
	text: SharedString,
	offset: Point<Pixels>,
	blur: Pixels,
	color: Rgba,
}

impl TextShadow {
	/// How far the shadow sits from the text. The default is 1 px down.
	pub fn offset(mut self, offset: Point<Pixels>) -> Self {
		self.offset = offset;
		self
	}

	/// The blur radius of the shadow. The default is 3 px.
	pub fn blur(mut self, blur: impl Into<Pixels>) -> Self {
		self.blur = blur.into();
		self
	}

	/// The color of the shadow, with its alpha. The default is 45% black.
	pub fn color(mut self, color: Rgba) -> Self {
		self.color = color;
		self
	}
}

impl RenderOnce for TextShadow {
	fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
		let shadow = div()
			.absolute()
			.left(self.offset.x)
			.top(self.offset.y)
			.whitespace_nowrap()
			.text_color(self.color)
			.blur(self.blur)
			.child(self.text.clone());
		div()
			.relative()
			.child(shadow)
			.child(div().whitespace_nowrap().child(self.text))
	}
}

#[cfg(test)]
mod tests {
	use gpui::{BackgroundKind, StyleRefinement, point};

	use super::*;

	struct Probe(StyleRefinement);

	impl Styled for Probe {
		fn style(&mut self) -> &mut StyleRefinement {
			&mut self.0
		}
	}

	#[test]
	fn drop_shadow_adds_a_content_filter_with_the_color_channels() {
		let probe = Probe(StyleRefinement::default()).drop_shadow(
			point(px(2.), px(3.)),
			px(4.),
			rgba(0.1, 0.2, 0.3, 0.4),
		);
		let filters = probe.0.filter.expect("a drop shadow sets a filter chain");
		assert_eq!(
			filters,
			[Filter::DropShadow(DropShadowFilter::new(
				point(px(2.), px(3.)),
				px(4.),
				[0.1, 0.2, 0.3, 0.4]
			))]
		);
	}

	#[test]
	fn radial_gradient_runs_from_the_first_stop_to_the_second() {
		let from = rgba(1., 0., 0., 1.);
		let to = rgba(0., 0., 1., 1.);
		let BackgroundKind::RadialGradient {
			center,
			radius,
			stops,
		} = radial_gradient(point(0.25, 0.75), 0.5, from, to).kind()
		else {
			panic!("radial_gradient is a radial gradient");
		};
		assert_eq!((center, radius), (point(0.25, 0.75), 0.5));
		assert_eq!((stops[0].percentage, stops[1].percentage), (0., 1.));
	}
}
