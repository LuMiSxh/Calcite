// SPDX-License-Identifier: MPL-2.0

//! A full-bleed cover image with a fallback that the app supplies.
//!
//! ```ignore
//! root.child(
//!     cover_image(Some(path))
//!         .fallback(app_backdrop())
//!         .corner(chrome::corner_radius(window)),
//! )
//! ```

use std::path::PathBuf;

use gpui::{
	Animation, AnimationExt, AnyElement, App, ElementId, Filter, IntoElement, ObjectFit, ParentElement,
	Pixels, RenderOnce, SharedString, Styled, StyledImage, Window, div, img, px,
};

use crate::{
	controls::theme_override,
	motion::Curve,
	theme::{ActiveTheme, Theme},
};
/// A cover-fit image that fills its parent. Without a path it shows the fallback, if one is set.
#[derive(IntoElement)]
pub struct CoverImage {
	path: Option<PathBuf>,
	fallback: Option<AnyElement>,
	corner: Pixels,
	reveal: bool,
	theme: Option<Theme>,
}

/// A cover-fit image of `path` that fills its parent. `None` shows the fallback.
pub fn cover_image(path: Option<PathBuf>) -> CoverImage {
	CoverImage {
		path,
		fallback: None,
		corner: px(0.),
		reveal: false,
		theme: None,
	}
}

impl CoverImage {
	/// The element shown while there is no image.
	pub fn fallback(mut self, fallback: impl IntoElement) -> Self {
		self.fallback = Some(fallback.into_any_element());
		self
	}

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

impl RenderOnce for CoverImage {
	fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let identity = match &self.path {
			Some(path) => path.to_string_lossy().into_owned(),
			None => "fallback".to_owned(),
		};
		let backdrop = div()
			.absolute()
			.size_full()
			.overflow_hidden()
			.rounded(self.corner)
			.bg(theme.palette.backdrop);
		let content = match (self.path, self.fallback) {
			(Some(path), _) => backdrop.child(
				img(path)
					.size_full()
					.rounded(self.corner)
					.object_fit(ObjectFit::Cover),
			),
			(None, Some(fallback)) => backdrop.child(fallback),
			(None, None) => backdrop,
		};
		if !self.reveal {
			return content.into_any_element();
		}
		let reduce_motion = theme.reduce_motion;
		let id = ElementId::from(SharedString::from(format!("calcite-artwork-{}", identity)));
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
theme_override!(CoverImage);
