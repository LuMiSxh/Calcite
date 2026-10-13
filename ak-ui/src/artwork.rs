// SPDX-License-Identifier: MPL-2.0

//! Full-bleed hero artwork, the readability field, and the wordmark.
//!
//! The repository must not ship official artwork, so the default backdrop is procedural.
//! Pass a local image to see the glass over real artwork.

use std::path::PathBuf;

use gpui::{
	Filter, FontWeight, IntoElement, ObjectFit, ParentElement, Pixels, Rgba, Styled, StyledImage, div, img,
	linear_color_stop, linear_gradient, px,
};

use crate::{
	icons::Icon,
	tokens::{self, black, rgba, scale_alpha, white},
};

#[derive(Clone, Debug, PartialEq)]
pub enum Artwork {
	Procedural,
	Image(PathBuf),
}

fn blob(left: f32, top: f32, size: f32, color: Rgba, blur: f32) -> impl IntoElement {
	div()
		.absolute()
		.left(px(left))
		.top(px(top))
		.size(px(size))
		.rounded_full()
		.bg(color)
		.filter(vec![Filter::Blur(px(blur))])
}

/// A bright, detailed scene so that the blur and the backplates are easy to judge.
fn procedural(accent: Rgba) -> impl IntoElement {
	let tile = |left: f32, top: f32, width: f32, height: f32, color: Rgba| {
		div()
			.absolute()
			.left(px(left))
			.top(px(top))
			.w(px(width))
			.h(px(height))
			.bg(color)
	};
	div()
		.absolute()
		.size_full()
		.overflow_hidden()
		.bg(linear_gradient(
			165.,
			linear_color_stop(rgba(0.07, 0.10, 0.16, 1.), 0.),
			linear_color_stop(tokens::ARTWORK_FALLBACK, 1.),
		))
		.child(blob(520., -120., 620., scale_alpha(accent, 0.55), 120.))
		.child(blob(-140., 300., 520., rgba(0.96, 0.52, 0.18, 0.45), 110.))
		.child(blob(300., 260., 360., rgba(0.82, 0.22, 0.52, 0.40), 90.))
		.child(blob(760., 380., 300., rgba(0.30, 0.86, 0.62, 0.30), 80.))
		// Sharp geometry under the HUD shows how much detail the glass keeps.
		.child(tile(0., 520., 2400., 3., white(0.55)))
		.child(tile(0., 560., 2400., 1., white(0.35)))
		.child(tile(0., 600., 2400., 6., scale_alpha(accent, 0.9)))
		.children((0..24).map(|index| {
			let x = 40. + index as f32 * 64.;
			tile(
				x,
				470.,
				18.,
				160.,
				if index % 3 == 0 { white(0.6) } else { white(0.18) },
			)
		}))
		// The fallback artwork motif: a giant "A" and an accent slash.
		.child(
			div()
				.absolute()
				.left(px(380.))
				.top(px(-80.))
				.text_size(px(440.))
				.font_weight(FontWeight::BLACK)
				.text_color(white(0.07))
				.child("A"),
		)
		.child(
			div()
				.absolute()
				.left(px(300.))
				.top(px(40.))
				.child(Icon::ArtworkSlash.render(px(620.), scale_alpha(accent, 0.4))),
		)
}

pub fn artwork_layer(artwork: &Artwork, accent: Rgba) -> impl IntoElement {
	match artwork {
		Artwork::Procedural => procedural(accent).into_any_element(),
		Artwork::Image(path) => div()
			.absolute()
			.size_full()
			.bg(tokens::ARTWORK_FALLBACK)
			.child(img(path.clone()).size_full().object_fit(ObjectFit::Cover))
			.into_any_element(),
	}
}

/// The small top-leading contrast field behind the wordmark and status text. SwiftUI uses
/// a radial gradient; a blurred black ellipse gives the same falloff.
pub fn readability_field(reduce_transparency: bool) -> impl IntoElement {
	div()
		.absolute()
		.left(px(-330.))
		.top(px(-300.))
		.w(px(1010.))
		.h(px(780.))
		.rounded_full()
		.bg(black(1.))
		.filter(vec![Filter::Blur(px(if reduce_transparency {
			60.
		} else {
			120.
		}))])
		.opacity(if reduce_transparency { 0.96 } else { 0.86 })
}

/// Fallback wordmark. The real app shows the official logo from the publisher's launcher
/// configuration; it is never bundled.
pub fn wordmark(size: Pixels) -> impl IntoElement {
	div()
		.font_family(SERIF_FAMILY)
		.text_size(size)
		.font_weight(FontWeight::SEMIBOLD)
		.text_color(white(0.94))
		.child("ARKNIGHTS")
}

#[cfg(target_os = "macos")]
const SERIF_FAMILY: &str = "Times New Roman";
#[cfg(not(target_os = "macos"))]
const SERIF_FAMILY: &str = "serif";
