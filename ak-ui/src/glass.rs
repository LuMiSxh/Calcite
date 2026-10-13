// SPDX-License-Identifier: MPL-2.0

//! macOS 27 glass treatment on gpui-ce, ported from `AdaptiveGlass.swift`.
//!
//! SwiftUI on macOS 27 stacks, from bottom to top:
//!
//! 1. Liquid Glass (`Glass.regular`, optionally tinted): blurred backdrop, a light lift,
//!    a specular rim, and a soft drop shadow.
//! 2. A backplate: black 0.68 for neutral surfaces, or the tint (at least 0.24) over
//!    black 0.44 for chromatic surfaces.
//! 3. The content.
//!
//! gpui-ce gives us the blurred backdrop (`backdrop_blur`). We composite layers 1 (lift)
//! and 2 into one translucent fill and draw the rim and shadow with box shadows, so a
//! surface costs one backdrop pass and no extra elements.

use gpui::{BoxShadow, Pixels, Rgba, Styled, linear_color_stop, linear_gradient, px};

use crate::tokens::{self, black, is_chromatic, over, scale_alpha, stack, white, with_alpha};

/// Blur radius of the backdrop behind glass. Liquid Glass keeps shapes readable behind
/// the surface; a larger radius turns into the older `NSVisualEffectView` material.
pub const BACKDROP_BLUR: Pixels = px(30.);

const NEUTRAL_BACKPLATE_OPACITY: f32 = 0.68;
const CHROMATIC_BACKPLATE_OPACITY: f32 = 0.44;
const MINIMUM_TINT_OPACITY: f32 = 0.24;
/// The brightening that `Glass.regular` adds over its blurred backdrop in dark mode.
const GLASS_LIFT: Rgba = white(0.07);
/// How strongly a tinted `Glass` colors its own lift.
const GLASS_TINT_STRENGTH: f32 = 0.12;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Treatment {
	/// macOS 27 Liquid Glass approximation.
	Glass,
	/// Accessibility: Reduce Transparency is on.
	ReducedTransparency,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Elevation {
	/// Flat surfaces inside another surface.
	None,
	/// Ordinary floating glass.
	Resting,
	/// An expanded HUD panel (`shadow(0.35, radius 12, y 5)` in SwiftUI).
	Raised,
}

#[derive(Clone, Copy, Debug)]
pub struct Glass {
	pub tint: Option<Rgba>,
	pub treatment: Treatment,
	/// Accessibility "Show borders": macOS 27 glass then draws its own edge.
	pub shows_border: bool,
	pub elevation: Elevation,
	/// Adds the hover lift of an interactive surface.
	pub hovered: bool,
}

impl Glass {
	pub fn new(tint: Option<Rgba>, treatment: Treatment) -> Self {
		Self {
			tint,
			treatment,
			shows_border: false,
			elevation: Elevation::Resting,
			hovered: false,
		}
	}

	pub fn elevation(mut self, elevation: Elevation) -> Self {
		self.elevation = elevation;
		self
	}

	pub fn hovered(mut self, hovered: bool) -> Self {
		self.hovered = hovered;
		self
	}

	/// The tint only counts as chromatic when it has visible hue; near-neutral HUD tints
	/// take the darker neutral backplate instead of turning the surface blue.
	fn chromatic_tint(&self) -> Option<Rgba> {
		self.tint
			.filter(|tint| is_chromatic(*tint))
			.map(|tint| with_alpha(tint, tint.alpha.clamp(MINIMUM_TINT_OPACITY, 1.)))
	}

	/// The single fill that replaces the lift and backplate layers.
	pub fn fill(&self) -> Rgba {
		match self.treatment {
			Treatment::ReducedTransparency => tokens::black(0.92),
			Treatment::Glass => {
				let mut fill = match self.chromatic_tint() {
					Some(tint) => {
						let lift = over(scale_alpha(with_alpha(tint, 1.), GLASS_TINT_STRENGTH), GLASS_LIFT);
						stack(&[lift, black(CHROMATIC_BACKPLATE_OPACITY), tint])
					}
					None => stack(&[GLASS_LIFT, black(NEUTRAL_BACKPLATE_OPACITY)]),
				};
				if self.hovered {
					fill = over(white(tokens::control::HUD_HOVER_SURFACE_OPACITY), fill);
				}
				fill
			}
		}
	}

	pub fn backdrop_blur(&self) -> Option<Pixels> {
		match self.treatment {
			Treatment::Glass => Some(BACKDROP_BLUR),
			Treatment::ReducedTransparency => None,
		}
	}

	/// Specular rim, inner edge glow, and drop shadow.
	pub fn shadows(&self) -> Vec<BoxShadow> {
		if self.treatment == Treatment::ReducedTransparency {
			return Vec::new();
		}
		let rim_tint = self.chromatic_tint().map(tokens::contrast_tint);
		let rim = |alpha: f32| match rim_tint {
			Some(tint) => over(scale_alpha(with_alpha(tint, 1.), alpha * 0.35), white(alpha)),
			None => white(alpha),
		};
		let mut shadows = vec![
			// Specular highlight along the upper edge, fading toward the lower right.
			BoxShadow::new(
				px(0.),
				px(1.),
				linear_gradient(
					160.,
					linear_color_stop(rim(0.36), 0.),
					linear_color_stop(rim(0.04), 0.65),
				),
			)
			.inset(),
			// Faint counter-highlight on the lower edge: the glass "thickness".
			BoxShadow::new(px(0.), px(-1.), rim(0.07)).inset(),
			// Soft light inside the rim, as the lens edge of Liquid Glass gathers light.
			BoxShadow::new(px(0.), px(0.), rim(0.05))
				.blur_radius(px(6.))
				.inset(),
		];
		match self.elevation {
			Elevation::None => {}
			Elevation::Resting => {
				shadows.push(BoxShadow::new(px(0.), px(4.), black(0.22)).blur_radius(px(14.)))
			}
			Elevation::Raised => {
				shadows.push(BoxShadow::new(px(0.), px(5.), black(0.35)).blur_radius(px(24.)))
			}
		}
		shadows
	}

	/// The accessibility edge. Ordinary macOS 27 glass leaves the edge to the caller.
	pub fn border(&self, semantic_edge: Option<Rgba>) -> Option<Rgba> {
		match self.treatment {
			Treatment::ReducedTransparency => Some(with_alpha(
				semantic_edge.or(self.tint).unwrap_or(white(1.)),
				if self.shows_border { 0.68 } else { 0.42 },
			)),
			Treatment::Glass if self.shows_border => {
				let tint = semantic_edge.or(self.tint).filter(|tint| is_chromatic(*tint));
				Some(with_alpha(tint.unwrap_or(tokens::QUIET_CONTROL_TINT), 0.72))
			}
			Treatment::Glass => None,
		}
	}
}

/// Applies a [`Glass`] to any styled element. Callers add their own semantic border.
pub trait GlassExt: Styled + Sized {
	fn glass(self, glass: Glass) -> Self {
		let mut this = self.bg(glass.fill()).shadow(glass.shadows());
		if let Some(radius) = glass.backdrop_blur() {
			this = this.backdrop_blur(radius);
		}
		if let Some(border) = glass.border(None) {
			this = this.border_1().border_color(border);
		}
		this
	}
}

impl<T: Styled> GlassExt for T {}
