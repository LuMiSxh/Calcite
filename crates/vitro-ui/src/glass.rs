// SPDX-License-Identifier: MPL-2.0

//! Liquid Glass on gpui-ce's patched `Filter::Glass` backdrop filter.
//!
//! Liquid Glass is not a blur. Its look comes from the bezel: a curved edge that bends the
//! backdrop along the shape normal, splits red and blue slightly, catches a thin Fresnel
//! rim, and throws a specular highlight on the side that faces the light. The flat face in
//! the middle shows the softly blurred, more saturated backdrop through a fill. Larger
//! surfaces read thicker: more lensing and a deeper shadow.
//!
//! The shader does all of that in the composite pass of the backdrop blur (one extra
//! texture sample pair per pixel, no extra render pass). This module only chooses the
//! parameters for each system look:
//!
//! - **macOS 26**: clear Liquid Glass. The tint colors the glass; there is no backplate.
//! - **macOS 27**: the transparency slider. *Default* matches the launcher's macOS 27
//!   branch (black 0.68 backplate, or the tint at least 0.24 over black 0.44). *Clear* lets
//!   more backdrop through; *Tinted* is nearly opaque.
//! - **Reduce Transparency**: no backdrop at all, an opaque plate and a visible edge.

use gpui::{BoxShadow, Filter, GlassFilter, Pixels, Point, Rgba, Styled, px};

use crate::tokens::{self, black, is_chromatic, over, scale_alpha, stack, white, with_alpha};

/// The slider positions of macOS 27's Liquid Glass transparency setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Transparency {
	Clear,
	Default,
	Tinted,
}

/// Which system look the glass imitates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GlassStyle {
	MacOS26,
	MacOS27(Transparency),
	ReducedTransparency,
}

impl GlassStyle {
	pub const ALL: [Self; 5] = [
		Self::MacOS26,
		Self::MacOS27(Transparency::Clear),
		Self::MacOS27(Transparency::Default),
		Self::MacOS27(Transparency::Tinted),
		Self::ReducedTransparency,
	];

	pub fn label(self) -> &'static str {
		match self {
			Self::MacOS26 => "macOS 26",
			Self::MacOS27(Transparency::Clear) => "macOS 27 · Clear",
			Self::MacOS27(Transparency::Default) => "macOS 27 · Default",
			Self::MacOS27(Transparency::Tinted) => "macOS 27 · Tinted",
			Self::ReducedTransparency => "Reduce Transparency",
		}
	}

	pub fn next(self) -> Self {
		let index = Self::ALL.iter().position(|style| *style == self).unwrap_or(0);
		Self::ALL[(index + 1) % Self::ALL.len()]
	}
}

/// How far a surface floats above its backdrop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Elevation {
	/// Inside another surface: no shadow.
	None,
	/// Ordinary floating glass.
	Resting,
	/// An expanded HUD panel: thicker glass and a deeper shadow.
	Raised,
}

/// Backplate opacities behind the glass: (neutral, chromatic, minimum tint).
fn backplate(style: GlassStyle) -> (f32, f32, f32) {
	match style {
		GlassStyle::MacOS26 => (0., 0., 0.),
		GlassStyle::MacOS27(Transparency::Clear) => (0.38, 0.26, 0.20),
		GlassStyle::MacOS27(Transparency::Default) => (0.68, 0.44, 0.24),
		GlassStyle::MacOS27(Transparency::Tinted) => (0.86, 0.64, 0.34),
		GlassStyle::ReducedTransparency => (0.92, 0.92, 1.),
	}
}

/// Optical constants of one look, before elevation scaling.
struct Optics {
	blur: f32,
	bezel: f32,
	refraction: f32,
	dispersion: f32,
	saturation: f32,
	rim: f32,
	specular: f32,
	/// The brightening of `Glass.regular` over its backdrop in dark mode.
	lift: f32,
}

fn optics(style: GlassStyle) -> Optics {
	match style {
		GlassStyle::MacOS26 => Optics {
			blur: 9.,
			bezel: 18.,
			refraction: 11.,
			dispersion: 0.18,
			saturation: 1.45,
			rim: 0.62,
			specular: 0.55,
			lift: 0.05,
		},
		GlassStyle::MacOS27(Transparency::Clear) => Optics {
			blur: 10.,
			bezel: 16.,
			refraction: 9.,
			dispersion: 0.14,
			saturation: 1.35,
			rim: 0.56,
			specular: 0.48,
			lift: 0.06,
		},
		GlassStyle::MacOS27(Transparency::Default) => Optics {
			blur: 13.,
			bezel: 15.,
			refraction: 7.,
			dispersion: 0.10,
			saturation: 1.3,
			rim: 0.50,
			specular: 0.40,
			lift: 0.07,
		},
		GlassStyle::MacOS27(Transparency::Tinted) => Optics {
			blur: 16.,
			bezel: 14.,
			refraction: 5.,
			dispersion: 0.06,
			saturation: 1.2,
			rim: 0.44,
			specular: 0.32,
			lift: 0.07,
		},
		GlassStyle::ReducedTransparency => Optics {
			blur: 0.,
			bezel: 0.,
			refraction: 0.,
			dispersion: 0.,
			saturation: 1.,
			rim: 0.,
			specular: 0.,
			lift: 0.,
		},
	}
}

/// Light from the upper left, like Apple's highlights.
const LIGHT_ANGLE: f32 = -2.25;
/// Radius and strength of the pointer light (`PointerSheen`: tint 0.30 at the center).
const POINTER_RADIUS: Pixels = px(72.);
const POINTER_STRENGTH: f32 = 0.30;

/// One glass surface.
#[derive(Clone, Copy, Debug)]
pub struct Glass {
	pub tint: Option<Rgba>,
	pub style: GlassStyle,
	pub elevation: Elevation,
	/// Accessibility "Show borders" / Increase Contrast: glass draws its own edge.
	pub shows_border: bool,
	/// Pointer position in window coordinates, for the hover light.
	pub pointer: Option<Point<Pixels>>,
	/// Color of the pointer light; usually the control's foreground.
	pub pointer_tint: Rgba,
	/// A fill composited above the glass fill, for hover and selection.
	pub overlay: Option<Rgba>,
}

impl Glass {
	pub fn new(tint: Option<Rgba>, style: GlassStyle) -> Self {
		Self {
			tint,
			style,
			elevation: Elevation::Resting,
			shows_border: false,
			pointer: None,
			pointer_tint: white(1.),
			overlay: None,
		}
	}

	pub fn elevation(mut self, elevation: Elevation) -> Self {
		self.elevation = elevation;
		self
	}

	pub fn pointer(mut self, pointer: Option<Point<Pixels>>, tint: Rgba) -> Self {
		self.pointer = pointer;
		self.pointer_tint = tint;
		self
	}

	pub fn overlay(mut self, overlay: Rgba) -> Self {
		self.overlay = Some(overlay);
		self
	}

	fn chromatic_tint(&self, minimum: f32) -> Option<Rgba> {
		self.tint
			.filter(|tint| is_chromatic(*tint))
			.map(|tint| with_alpha(tint, tint.alpha.clamp(minimum, 1.)))
	}

	/// The fill over the refracted backdrop: glass lift, tint, and backplate in one color.
	pub fn fill(&self) -> Rgba {
		let (neutral, chromatic, minimum) = backplate(self.style);
		let lift = white(optics(self.style).lift);
		let mut fill = match self.style {
			GlassStyle::ReducedTransparency => black(0.92),
			// macOS 26 tints the glass itself, also with the near-black HUD tint.
			GlassStyle::MacOS26 => match self.tint {
				Some(tint) if is_chromatic(tint) => {
					stack(&[lift, with_alpha(tint, (tint.alpha * 1.8).clamp(0.22, 0.6))])
				}
				Some(tint) => stack(&[lift, scale_alpha(tint, 0.62)]),
				None => lift,
			},
			GlassStyle::MacOS27(_) => match self.chromatic_tint(minimum) {
				Some(tint) => stack(&[lift, black(chromatic), tint]),
				None => stack(&[lift, black(neutral)]),
			},
		};
		if let Some(overlay) = self.overlay {
			fill = over(overlay, fill);
		}
		fill
	}

	/// The glass filter, or `None` under Reduce Transparency.
	pub fn filter(&self) -> Option<GlassFilter> {
		if self.style == GlassStyle::ReducedTransparency {
			return None;
		}
		let optics = optics(self.style);
		let thickness = match self.elevation {
			Elevation::Raised => 1.3,
			_ => 1.,
		};
		let fill = self.fill();
		let pointer_color = with_alpha(tokens::contrast_tint(self.pointer_tint), POINTER_STRENGTH);
		Some(GlassFilter {
			blur: px(optics.blur * thickness),
			bezel: px(optics.bezel * thickness),
			refraction: px(optics.refraction * thickness),
			dispersion: optics.dispersion,
			saturation: optics.saturation,
			brightness: 1.,
			fill: [fill.red, fill.green, fill.blue, fill.alpha],
			rim: optics.rim,
			specular: optics.specular,
			light_angle: LIGHT_ANGLE,
			pointer: self.pointer.map(|point| [f32::from(point.x), f32::from(point.y)]),
			pointer_radius: POINTER_RADIUS,
			pointer_color: [
				pointer_color.red,
				pointer_color.green,
				pointer_color.blue,
				pointer_color.alpha,
			],
		})
	}

	/// The drop shadow. Liquid Glass casts a soft, wide shadow; a raised panel a deeper one.
	pub fn shadows(&self) -> Vec<BoxShadow> {
		if self.style == GlassStyle::ReducedTransparency {
			return Vec::new();
		}
		match self.elevation {
			Elevation::None => Vec::new(),
			Elevation::Resting => {
				vec![BoxShadow::new(px(0.), px(4.), black(0.20)).blur_radius(px(16.))]
			}
			Elevation::Raised => {
				vec![BoxShadow::new(px(0.), px(8.), black(0.34)).blur_radius(px(28.))]
			}
		}
	}

	/// The edge the glass draws itself: always under Reduce Transparency, and on macOS 27
	/// when the user asks for borders. Other edges belong to the caller.
	pub fn border(&self) -> Option<Rgba> {
		let edge = self.tint.unwrap_or(white(1.));
		match self.style {
			GlassStyle::ReducedTransparency => {
				Some(with_alpha(edge, if self.shows_border { 0.68 } else { 0.42 }))
			}
			GlassStyle::MacOS27(_) if self.shows_border => {
				let tint = self.tint.filter(|tint| is_chromatic(*tint));
				Some(with_alpha(tint.unwrap_or(tokens::QUIET_CONTROL_TINT), 0.72))
			}
			_ => None,
		}
	}
}

/// A flat, non-refracting surface that still carries the pointer light: the quiet neutral
/// fill of secondary controls.
pub fn flat_surface(fill: Rgba, pointer: Option<Point<Pixels>>, pointer_tint: Rgba) -> GlassFilter {
	let pointer_color = with_alpha(pointer_tint, POINTER_STRENGTH);
	GlassFilter {
		blur: px(1.),
		bezel: px(1.),
		refraction: px(0.),
		dispersion: 0.,
		saturation: 1.,
		brightness: 1.,
		fill: [fill.red, fill.green, fill.blue, fill.alpha],
		rim: 0.,
		specular: 0.,
		light_angle: LIGHT_ANGLE,
		pointer: pointer.map(|point| [f32::from(point.x), f32::from(point.y)]),
		pointer_radius: POINTER_RADIUS,
		pointer_color: [
			pointer_color.red,
			pointer_color.green,
			pointer_color.blue,
			pointer_color.alpha,
		],
	}
}

/// Applies a [`Glass`] to any styled element. Callers add their own semantic border.
pub trait GlassExt: Styled + Sized {
	fn glass(self, glass: Glass) -> Self {
		let mut this = self.shadow(glass.shadows());
		this = match glass.filter() {
			Some(filter) => this.backdrop_filter(vec![Filter::Glass(filter)]),
			None => this.bg(glass.fill()),
		};
		if let Some(border) = glass.border() {
			this = this.border_1().border_color(border);
		}
		this
	}
}

impl<T: Styled> GlassExt for T {}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn macos_27_default_matches_the_launcher_backplate() {
		let glass = Glass::new(Some(black(0.52)), GlassStyle::MacOS27(Transparency::Default));
		let expected = stack(&[white(0.07), black(0.68)]);
		assert!((glass.fill().alpha - expected.alpha).abs() < 1e-4);
	}

	#[test]
	fn reduce_transparency_has_no_backdrop() {
		assert!(
			Glass::new(None, GlassStyle::ReducedTransparency)
				.filter()
				.is_none()
		);
	}
}
