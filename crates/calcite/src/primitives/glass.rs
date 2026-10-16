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

use gpui::{BoxShadow, Filter, GlassFilter, Pixels, Point, Rgba, Size, Styled, px};

use crate::palette::Palette;
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
	// Tuned against macOS 27 screenshots (Dock, Music, Photos at the three slider stops):
	// the lens is a narrow band at the very edge, the face stays flat and frosted, the rim
	// is a hairline, and colour fringing is barely visible. macOS 26 keeps a little more.
	match style {
		GlassStyle::MacOS26 => Optics {
			blur: 9.,
			bezel: 12.,
			refraction: 8.,
			dispersion: 0.08,
			saturation: 1.35,
			rim: 0.45,
			specular: 0.34,
			lift: 0.05,
		},
		GlassStyle::MacOS27(Transparency::Clear) => Optics {
			blur: 5.,
			bezel: 8.,
			refraction: 6.,
			dispersion: 0.03,
			saturation: 1.15,
			rim: 0.30,
			specular: 0.18,
			lift: 0.05,
		},
		GlassStyle::MacOS27(Transparency::Default) => Optics {
			blur: 16.,
			bezel: 8.,
			refraction: 4.,
			dispersion: 0.02,
			saturation: 1.15,
			rim: 0.28,
			specular: 0.16,
			lift: 0.09,
		},
		GlassStyle::MacOS27(Transparency::Tinted) => Optics {
			blur: 26.,
			bezel: 7.,
			refraction: 2.5,
			dispersion: 0.,
			saturation: 1.05,
			rim: 0.24,
			specular: 0.12,
			lift: 0.12,
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
/// Strength of the pointer light (`PointerSheen`: tint 0.30 at the center).
const POINTER_STRENGTH: f32 = 0.30;
/// Pointer light radius when a surface's size is not known yet: its first frame, or a filter
/// built before layout.
pub(crate) const DEFAULT_POINTER_RADIUS: Pixels = px(40.);
/// The pointer light grows with its surface: this fraction of the short side, between the
/// minimum and maximum below.
const POINTER_RADIUS_SCALE: f32 = 0.9;
const POINTER_RADIUS_MIN: f32 = 28.;
const POINTER_RADIUS_MAX: f32 = 56.;

/// Radius of the pointer light on a surface of `size`: 0.9 of its short side, clamped to
/// 28..=56 pt. A 36 pt button gets 32 pt, a 28 pt pill 28 pt, and a 200 x 120 pt panel 56 pt.
pub fn pointer_radius_for(size: Size<Pixels>) -> Pixels {
	let short = f32::from(size.width).min(f32::from(size.height));
	px((short * POINTER_RADIUS_SCALE).clamp(POINTER_RADIUS_MIN, POINTER_RADIUS_MAX))
}

/// Lens depth of glass inside another surface, relative to resting glass.
const NESTED_THICKNESS: f32 = 0.35;

/// Strength of the tinted glass layer that `Glass.regular.tint(_:)` lays over the backplate.
const TINTED_GLASS_OPACITY: f32 = 0.24;
/// How far that layer is lifted toward white: system tinted glass reads lighter than the tint.
const TINTED_GLASS_LIFT: f32 = 0.22;

/// The luminous layer of system tinted glass. Without it a chromatic control over a dark
/// surface sinks into the backplate and reads as an outline instead of glass.
fn tinted_glass(tint: Rgba) -> Rgba {
	with_alpha(
		tokens::mix(with_alpha(tint, 1.), white(1.), TINTED_GLASS_LIFT),
		TINTED_GLASS_OPACITY,
	)
}

/// One glass surface.
#[derive(Clone, Copy, Debug)]
pub struct Glass {
	pub tint: Option<Rgba>,
	pub style: GlassStyle,
	/// Multiplies blur, bezel, refraction, and tint and backplate opacity. 1 is the system
	/// look, 0 is flat. Reduce Transparency ignores it.
	pub strength: f32,
	pub elevation: Elevation,
	/// Accessibility "Show borders" / Increase Contrast: glass draws its own edge.
	pub shows_border: bool,
	/// Pointer position in window coordinates, for the hover light.
	pub pointer: Option<Point<Pixels>>,
	/// Color of the pointer light; usually the control's foreground. Its alpha scales the light, so 0 hides it.
	pub pointer_tint: Rgba,
	/// Radius of the pointer light; `None` uses the default. Components pass
	/// [`pointer_radius_for`] of their size.
	pub pointer_radius: Option<Pixels>,
	/// A fill composited above the glass fill, for hover and selection.
	pub overlay: Option<Rgba>,
	/// The neutral edge the macOS 27 border falls back to: the palette's quiet control tint.
	pub quiet_edge: Rgba,
	/// The neutral of fills and edges (the palette's ink) and of the backplate behind the glass.
	pub ink: Rgba,
	pub shade: Rgba,
	/// Scales the drop shadow: 1 on a dark palette, less on a light one.
	pub shadow_scale: f32,
}

impl Glass {
	pub fn new(tint: Option<Rgba>, style: GlassStyle) -> Self {
		Self {
			tint,
			style,
			strength: 1.,
			elevation: Elevation::Resting,
			shows_border: false,
			pointer: None,
			pointer_tint: white(1.),
			pointer_radius: None,
			overlay: None,
			quiet_edge: Palette::default().quiet_control,
			ink: white(1.),
			shade: black(1.),
			shadow_scale: 1.,
		}
	}

	/// Takes the neutral colors (the quiet edge, ink, and shade) from `palette`. Glass drawn
	/// from a theme gets them from [`Theme::surface`](crate::theme::Theme::surface).
	pub fn palette(mut self, palette: &Palette) -> Self {
		self.quiet_edge = palette.quiet_control;
		self.ink = palette.ink;
		self.shade = palette.shade;
		self
	}

	pub fn elevation(mut self, elevation: Elevation) -> Self {
		self.elevation = elevation;
		self
	}

	pub fn strength(mut self, strength: f32) -> Self {
		self.strength = strength.max(0.);
		self
	}

	pub fn pointer(mut self, pointer: Option<Point<Pixels>>, tint: Rgba) -> Self {
		self.pointer = pointer;
		self.pointer_tint = tint;
		self
	}

	/// The pointer light's radius, usually [`pointer_radius_for`] of the surface's size.
	pub fn pointer_radius(mut self, radius: Pixels) -> Self {
		self.pointer_radius = Some(radius);
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
		let strength = self.strength;
		let scaled = |color: Rgba| scale_alpha(color, strength);
		let mut fill = match self.style {
			GlassStyle::ReducedTransparency => with_alpha(self.shade, 0.92),
			// macOS 26 tints the glass itself, also with the near-black HUD tint.
			GlassStyle::MacOS26 => match self.tint {
				Some(tint) if is_chromatic(tint) => stack(&[
					lift,
					scaled(with_alpha(tint, (tint.alpha * 1.8).clamp(0.22, 0.6))),
					scaled(tinted_glass(tint)),
				]),
				Some(tint) => stack(&[lift, scaled(scale_alpha(tint, 0.62))]),
				None => lift,
			},
			GlassStyle::MacOS27(_) => match self.chromatic_tint(minimum) {
				Some(tint) => stack(&[
					lift,
					scaled(with_alpha(self.shade, chromatic)),
					scaled(tint),
					scaled(tinted_glass(tint)),
				]),
				None => stack(&[lift, scaled(with_alpha(self.shade, neutral))]),
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
		let strength = self.strength;
		let thickness = match self.elevation {
			Elevation::Raised => 1.3,
			Elevation::Resting => 1.,
			Elevation::None => NESTED_THICKNESS,
		};
		// Glass inside another surface (controls in a panel or a bar) reads almost flat in
		// the launcher: a tinted fill and its border, with only a trace of rim and light.
		let (rim, specular, dispersion) = match self.elevation {
			Elevation::None => (optics.rim * 0.5, optics.specular * 0.4, 0.),
			_ => (optics.rim, optics.specular, optics.dispersion),
		};
		let fill = self.fill();
		let pointer_color = scale_alpha(tokens::contrast_tint(self.pointer_tint), POINTER_STRENGTH);
		Some(GlassFilter {
			blur: px(optics.blur * thickness * strength),
			bezel: px(optics.bezel * thickness * strength),
			refraction: px(optics.refraction * thickness * strength),
			dispersion,
			saturation: optics.saturation,
			brightness: 1.,
			fill: [fill.red, fill.green, fill.blue, fill.alpha],
			rim,
			specular,
			light_angle: LIGHT_ANGLE,
			pointer: self.pointer.map(|point| [f32::from(point.x), f32::from(point.y)]),
			pointer_radius: self.pointer_radius.unwrap_or(DEFAULT_POINTER_RADIUS),
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
				vec![BoxShadow::new(px(0.), px(4.), black(0.20 * self.shadow_scale)).blur_radius(px(16.))]
			}
			Elevation::Raised => {
				vec![BoxShadow::new(px(0.), px(8.), black(0.34 * self.shadow_scale)).blur_radius(px(28.))]
			}
		}
	}

	/// The hairline edge of a HUD surface: the glass's own edge when it draws one, else
	/// `hairline` at the HUD border opacity. Grouped glass uses it for its single outline.
	pub fn edge(&self, hairline: Rgba) -> Rgba {
		self.border()
			.unwrap_or_else(|| scale_alpha(hairline, tokens::control::HUD_BORDER_OPACITY))
	}

	/// The edge the glass draws itself: always under Reduce Transparency, and on macOS 27
	/// when the user asks for borders. Other edges belong to the caller.
	pub fn border(&self) -> Option<Rgba> {
		let edge = self.tint.unwrap_or(self.ink);
		match self.style {
			GlassStyle::ReducedTransparency => {
				Some(with_alpha(edge, if self.shows_border { 0.68 } else { 0.42 }))
			}
			GlassStyle::MacOS27(_) if self.shows_border => {
				let tint = self.tint.filter(|tint| is_chromatic(*tint));
				Some(with_alpha(tint.unwrap_or(self.quiet_edge), 0.72))
			}
			_ => None,
		}
	}
}

/// A flat, non-refracting surface that still carries the pointer light: the quiet neutral
/// fill of secondary controls.
pub fn flat_surface(
	fill: Rgba,
	pointer: Option<Point<Pixels>>,
	pointer_tint: Rgba,
	pointer_radius: Pixels,
) -> GlassFilter {
	let pointer_color = scale_alpha(pointer_tint, POINTER_STRENGTH);
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
		pointer_radius,
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

	/// The HUD glass of the control bar and pills, with its hairline edge.
	fn glass_hud(self, theme: &crate::theme::Theme) -> Self {
		self.glass(theme.hud_surface())
			.border_1()
			.border_color(theme.hud_border())
	}

	/// [`GlassExt::glass_hud`] at an elevation.
	fn glass_hud_at(self, theme: &crate::theme::Theme, elevation: Elevation) -> Self {
		self.glass(theme.hud_surface().elevation(elevation))
			.border_1()
			.border_color(theme.hud_border())
	}

	/// Chromatic glass in `tint`, with the semantic edge of an accent surface.
	fn glass_tinted(self, theme: &crate::theme::Theme, tint: Rgba) -> Self {
		let chromatic = tokens::Chromatic::enabled(tint);
		self.glass(theme.surface(Some(chromatic.fill)))
			.border_1()
			.border_color(chromatic.border)
	}
}

impl<T: Styled> GlassExt for T {}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn macos_27_default_matches_the_launcher_backplate() {
		let glass = Glass::new(Some(black(0.52)), GlassStyle::MacOS27(Transparency::Default));
		let lift = optics(GlassStyle::MacOS27(Transparency::Default)).lift;
		let expected = stack(&[white(lift), black(0.68)]);
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

	#[test]
	fn strength_scales_optics_and_backplate() {
		let full = Glass::new(None, GlassStyle::MacOS27(Transparency::Default));
		let half = full.strength(0.5);
		let flat = full.strength(0.);
		let blur = |glass: &Glass| f32::from(glass.filter().unwrap().blur);
		assert!((blur(&half) - blur(&full) / 2.).abs() < 1e-4);
		assert_eq!(blur(&flat), 0.);
		assert!(half.fill().alpha < full.fill().alpha);
		assert!(flat.fill().alpha < half.fill().alpha);
	}

	#[test]
	fn strength_does_not_touch_reduced_transparency() {
		let reduced = Glass::new(None, GlassStyle::ReducedTransparency);
		assert_eq!(reduced.strength(0.).fill(), reduced.fill());
	}

	fn sized(width: f32, height: f32) -> Size<Pixels> {
		Size {
			width: px(width),
			height: px(height),
		}
	}

	fn radius(width: f32, height: f32) -> f32 {
		f32::from(pointer_radius_for(sized(width, height)))
	}

	#[test]
	fn pointer_radius_matches_the_design_cases() {
		assert!((radius(96., 36.) - 32.4).abs() < 1e-3);
		assert_eq!(radius(72., 28.), 28.);
		assert_eq!(radius(200., 120.), 56.);
	}

	#[test]
	fn pointer_radius_clamps_to_its_range() {
		assert_eq!(radius(4., 4.), POINTER_RADIUS_MIN);
		assert_eq!(radius(1000., 1000.), POINTER_RADIUS_MAX);
	}

	#[test]
	fn pointer_radius_follows_the_short_side_and_never_shrinks() {
		assert_eq!(radius(200., 36.), radius(36., 200.));
		for step in 0..60 {
			let side = step as f32 * 5.;
			assert!(radius(side + 5., 36.) >= radius(side, 36.));
			assert!(radius(200., side + 5.) >= radius(200., side));
		}
	}

	#[test]
	fn filter_uses_the_default_radius_until_one_is_set() {
		let base = Glass::new(None, GlassStyle::MacOS27(Transparency::Default));
		let filter_radius = |glass: &Glass| f32::from(glass.filter().unwrap().pointer_radius);
		assert_eq!(filter_radius(&base), 40.);
		assert_eq!(filter_radius(&base.pointer_radius(px(30.))), 30.);
	}
}
