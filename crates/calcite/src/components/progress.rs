// SPDX-License-Identifier: MPL-2.0

//! `CapsuleProgressOutline`: transfer progress drawn around a capsule without changing
//! the capsule's layout, with the leading glow, the travelling sweep, and the
//! indeterminate glint.

use std::f32::consts::PI;

use gpui::{
	App, Bounds, IntoElement, PathBuilder, Pixels, Point, Rgba, Styled, Window, canvas, div, point,
	prelude::*, px,
};

use crate::{
	continuous::ContinuousMotion,
	motion::cycle,
	tokens::{scale_alpha, white},
};

const LINE_WIDTH: f32 = 2.;
const SAMPLE_SPACING: f32 = 2.;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OutlineMode {
	/// Known fraction in 0..=1.
	Determinate(f32),
	/// Work without measurable progress: a full outline with an orbiting glint.
	Indeterminate,
}

/// Perimeter of a capsule inset by half the line width. Distance 0 is the middle of the
/// left cap; the path runs clockwise over the top, so progress reads left to right.
#[derive(Clone, Copy)]
struct CapsulePath {
	origin: Point<Pixels>,
	width: f32,
	height: f32,
}

impl CapsulePath {
	fn new(bounds: Bounds<Pixels>) -> Self {
		let inset = LINE_WIDTH / 2.;
		Self {
			origin: point(bounds.origin.x + px(inset), bounds.origin.y + px(inset)),
			width: (f32::from(bounds.size.width) - LINE_WIDTH).max(0.),
			height: (f32::from(bounds.size.height) - LINE_WIDTH).max(0.),
		}
	}

	fn radius(&self) -> f32 {
		(self.height / 2.).min(self.width / 2.)
	}

	fn straight(&self) -> f32 {
		(self.width - 2. * self.radius()).max(0.)
	}

	fn length(&self) -> f32 {
		2. * self.straight() + 2. * PI * self.radius()
	}

	fn point_at(&self, distance: f32) -> Point<Pixels> {
		let r = self.radius();
		let straight = self.straight();
		let quarter = PI * r / 2.;
		let half = PI * r;
		let left_center = (r, r);
		let right_center = (r + straight, r);
		let mut d = distance.rem_euclid(self.length().max(f32::EPSILON));
		let arc = |center: (f32, f32), angle: f32| (center.0 + r * angle.cos(), center.1 + r * angle.sin());

		let (x, y) = if d < quarter {
			arc(left_center, PI + d / r)
		} else {
			d -= quarter;
			if d < straight {
				(r + d, 0.)
			} else {
				d -= straight;
				if d < half {
					arc(right_center, -PI / 2. + d / r)
				} else {
					d -= half;
					if d < straight {
						(r + straight - d, 2. * r)
					} else {
						d -= straight;
						arc(left_center, PI / 2. + d / r)
					}
				}
			}
		};
		point(self.origin.x + px(x), self.origin.y + px(y))
	}

	/// Strokes the stretch between two fractions of the perimeter.
	fn stroke(&self, from: f32, to: f32, width: f32, color: Rgba, window: &mut Window) {
		let length = self.length();
		let (start, end) = (from.clamp(0., 1.) * length, to.clamp(0., 1.) * length);
		if end - start < 0.5 || color.alpha <= 0. {
			return;
		}
		let mut builder = PathBuilder::stroke(px(width));
		builder.move_to(self.point_at(start));
		let mut distance = start + SAMPLE_SPACING;
		while distance < end {
			builder.line_to(self.point_at(distance));
			distance += SAMPLE_SPACING;
		}
		builder.line_to(self.point_at(end));
		if let Ok(path) = builder.build() {
			window.paint_path(path, color);
		}
	}
}

/// Phase of the sweep and glint loops in 0..1; updated by the parent animation.
#[derive(IntoElement)]
struct OutlineCanvas {
	mode: OutlineMode,
	tint: Rgba,
	track: Rgba,
	is_glint_active: bool,
	phase: f32,
}

impl RenderOnce for OutlineCanvas {
	fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
		canvas(
			|_, _, _| {},
			move |bounds, _, window, _| {
				let path = CapsulePath::new(bounds);
				path.stroke(0., 1., LINE_WIDTH, self.track, window);
				match self.mode {
					OutlineMode::Indeterminate => {
						path.stroke(0., 1., LINE_WIDTH, scale_alpha(self.tint, 0.35), window);
						let head = (self.phase * cycle::INDETERMINATE_ORBIT_LAPS).fract();
						let tail = head - cycle::INDETERMINATE_GLINT_LENGTH;
						for (from, to) in wrapped(tail, head) {
							path.stroke(from, to, LINE_WIDTH * 2.5, scale_alpha(self.tint, 0.35), window);
							path.stroke(from, to, LINE_WIDTH, self.tint, window);
						}
					}
					OutlineMode::Determinate(fraction) => {
						let fraction = fraction.clamp(0., 1.);
						path.stroke(0., fraction, LINE_WIDTH, self.tint, window);
						if self.is_glint_active && fraction > 0. {
							// Soft glow at the leading edge of the transferred fraction.
							let tail = (fraction - 0.035_f32.min(fraction)).max(0.);
							path.stroke(
								tail,
								fraction,
								LINE_WIDTH * 4.,
								scale_alpha(self.tint, 0.18),
								window,
							);
							path.stroke(
								tail,
								fraction,
								LINE_WIDTH * 2.5,
								scale_alpha(self.tint, 0.35),
								window,
							);
							sweep(&path, fraction, self.phase, window);
						}
					}
				}
			},
		)
		.absolute()
		.top_0()
		.left_0()
		.size_full()
	}
}

/// The light that travels from the start to the progress head, then rests.
fn sweep(path: &CapsulePath, fraction: f32, phase: f32, window: &mut Window) {
	let total = cycle::progress_sweep_total();
	let time = phase * total;
	let fade_in = cycle::PROGRESS_SWEEP_FADE_IN;
	let travel = cycle::PROGRESS_SWEEP_TRAVEL;
	let fade_out = cycle::PROGRESS_SWEEP_FADE_OUT;
	let (position, alpha) = if time < fade_in {
		(0., time / fade_in)
	} else if time < fade_in + travel {
		let t = (time - fade_in) / travel;
		(t * t * (3. - 2. * t), 1.)
	} else if time < fade_in + travel + fade_out {
		(1., 1. - (time - fade_in - travel) / fade_out)
	} else {
		return;
	};
	let head = position * fraction;
	let tail = (head - 0.08).max(0.);
	path.stroke(tail, head, LINE_WIDTH, white(0.55 * alpha), window);
}

fn wrapped(from: f32, to: f32) -> Vec<(f32, f32)> {
	if from >= 0. {
		vec![(from, to)]
	} else {
		vec![(1. + from, 1.), (0., to)]
	}
}

/// Outline overlay for a capsule. `motion` is the app's own gate for this outline; the
/// loops also follow [`ContinuousMotion`]: they hold their phase while the window is in the
/// background and stop under Reduce Motion, where the glow and sweep are hidden as well.
pub fn progress_outline(
	mode: OutlineMode,
	theme: &crate::theme::Theme,
	is_glint_active: bool,
	motion: bool,
	window: &mut Window,
	cx: &mut App,
) -> impl IntoElement {
	let animates =
		motion && !theme.reduce_motion && (is_glint_active || matches!(mode, OutlineMode::Indeterminate));
	let (key, period) = match mode {
		OutlineMode::Indeterminate => ("calcite-outline-breathing", cycle::BREATHING),
		OutlineMode::Determinate(_) => ("calcite-progress-sweep", cycle::progress_sweep_total()),
	};
	let phase = if animates {
		ContinuousMotion::phase(key, period, theme, window, cx)
	} else {
		0.
	};
	let canvas = OutlineCanvas {
		mode,
		tint: theme.accent,
		track: crate::tokens::CLEAR,
		is_glint_active: is_glint_active && !theme.reduce_motion,
		phase,
	};
	div().absolute().top_0().left_0().size_full().child(canvas)
}
