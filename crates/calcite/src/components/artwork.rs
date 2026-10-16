// SPDX-License-Identifier: MPL-2.0

//! The launcher's full-bleed backdrop: the artwork image or the fallback motif
//! (`LauncherArtworkView`), and the dark field behind the wordmark and status text
//! (`LauncherReadabilityField`).
//!
//! ```ignore
//! root.child(artwork_layer(ArtworkSource::Image(path)).corner(chrome::corner_radius(window)))
//!     .child(readability_field(theme.reduce_transparency()))
//! ```

use std::path::PathBuf;

use gpui::{
	Animation, AnimationExt, AnyElement, App, ElementId, Filter, FontWeight, IntoElement, ObjectFit,
	ParentElement, Pixels, RenderOnce, SharedString, Styled, StyledImage, Window, div, img, px, relative,
	svg,
};

use crate::{
	controls::theme_override,
	motion::Curve,
	theme::{ActiveTheme, Theme},
	tokens::{black, scale_alpha, white},
};

/// Asset path of the accent slash in the fallback artwork; `Assets::default` serves it.
pub(crate) const SLASH_PATH: &str = "calcite/artwork/slash.svg";
pub(crate) const SLASH_SVG: &[u8] = include_bytes!("../../assets/artwork/slash.svg");

/// What the backdrop shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArtworkSource {
	/// The near-black backdrop with the large "A" and the accent slash.
	Fallback,
	Image(PathBuf),
}

impl ArtworkSource {
	/// Names the artwork, so a change of artwork plays the crossfade again.
	fn identity(&self) -> SharedString {
		match self {
			Self::Fallback => "fallback".into(),
			Self::Image(path) => path.to_string_lossy().into_owned().into(),
		}
	}
}

/// The backdrop of the window. Fill the window with it and draw the interface above.
#[derive(IntoElement)]
pub struct ArtworkLayer {
	source: ArtworkSource,
	corner: Pixels,
	reveal: bool,
	theme: Option<Theme>,
}

/// The artwork or the fallback motif, covering its parent.
pub fn artwork_layer(source: ArtworkSource) -> ArtworkLayer {
	ArtworkLayer {
		source,
		corner: px(0.),
		reveal: false,
		theme: None,
	}
}

impl ArtworkLayer {
	/// Rounds the backdrop inside a client-drawn window frame
	/// ([`chrome::corner_radius`](crate::chrome::corner_radius)).
	pub fn corner(mut self, corner: Pixels) -> Self {
		self.corner = corner;
		self
	}

	/// Fades the artwork in with the cross-fade curve, blurred and clearing. Leave it off for the
	/// first artwork the window shows: it appears at once, and only later changes fade.
	pub fn reveal(mut self, reveal: bool) -> Self {
		self.reveal = reveal;
		self
	}
}

impl RenderOnce for ArtworkLayer {
	fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let backdrop = div()
			.absolute()
			.size_full()
			.overflow_hidden()
			.rounded(self.corner)
			.bg(theme.palette.backdrop);
		let content = match &self.source {
			ArtworkSource::Image(path) => backdrop.child(
				img(path.clone())
					.size_full()
					.rounded(self.corner)
					.object_fit(ObjectFit::Cover),
			),
			ArtworkSource::Fallback => backdrop.child(fallback_motif(&theme)),
		};
		if !self.reveal {
			return content.into_any_element();
		}
		let reduce_motion = theme.reduce_motion;
		let id = ElementId::from(SharedString::from(format!(
			"calcite-artwork-{}",
			self.source.identity()
		)));
		content
			.with_animation(
				id,
				Animation::new(Curve::Crossfade.timed(reduce_motion)),
				move |layer, t| {
					let layer = layer.opacity(t);
					let blur = if reduce_motion { 0. } else { REVEAL_BLUR * (1. - t) };
					if blur > 0.01 {
						layer.filter(vec![Filter::Blur(px(blur))])
					} else {
						layer
					}
				},
			)
			.into_any_element()
	}
}

/// The blur radius a revealing artwork starts with.
const REVEAL_BLUR: f32 = 12.;
/// Point size of the fallback "A".
const FALLBACK_LETTER: f32 = 440.;
/// Side of the box that holds the fallback slash.
const SLASH_SIZE: f32 = 620.;

/// The fallback artwork: a faint "A" and an accent slash, centered.
fn fallback_motif(theme: &Theme) -> AnyElement {
	div()
		.absolute()
		.size_full()
		.flex()
		.items_center()
		.justify_center()
		.child(
			div()
				.font_family(theme.sans())
				.text_size(px(FALLBACK_LETTER))
				.line_height(relative(1.2))
				.font_weight(FontWeight::BLACK)
				.text_color(white(0.07))
				.child("A"),
		)
		.child(
			div()
				.absolute()
				.size_full()
				.flex()
				.items_center()
				.justify_center()
				.child(
					svg()
						.path(SLASH_PATH)
						.size(px(SLASH_SIZE))
						.flex_none()
						.text_color(scale_alpha(theme.accent, 0.4)),
				),
		)
		.into_any_element()
}

/// How far the readability field reaches from the top-leading corner.
const FIELD_REACH: f32 = 450.;
/// Room around the field so the blur is not clipped at the edge of its box.
const FIELD_MARGIN: f32 = 90.;
const FIELD_BLUR: f32 = 36.;
/// Radius and opacity of the discs that stand in for the radial gradient. The gradient runs black,
/// 68 % black, 20 % black, clear over 450 pt, so it falls about 20 % per 90 pt. Each disc is a
/// band of that fall at its middle (10 %, 30 %, 50 %, 70 %, 90 %); the opacities compound to
/// those totals, and the blur joins the bands.
const FIELD_DISCS: [(f32, f32); 5] = [
	(450., 0.1),
	(360., 0.2222),
	(270., 0.2857),
	(180., 0.4),
	(90., 0.6667),
];

/// The dark field at the top-leading corner that keeps the wordmark and the status text legible
/// over any artwork. gpui has no radial gradient, so blurred discs approximate it.
pub fn readability_field(reduce_transparency: bool) -> impl IntoElement {
	let extent = FIELD_REACH + FIELD_MARGIN;
	div()
		.absolute()
		.left(px(-extent))
		.top(px(-extent))
		.size(px(extent * 2.))
		.filter(vec![Filter::Blur(px(FIELD_BLUR))])
		.opacity(if reduce_transparency { 0.96 } else { 0.86 })
		.children(FIELD_DISCS.map(|(radius, alpha)| {
			div()
				.absolute()
				.left(px(extent - radius))
				.top(px(extent - radius))
				.size(px(radius * 2.))
				.rounded_full()
				.bg(black(alpha))
		}))
}

theme_override!(ArtworkLayer);
