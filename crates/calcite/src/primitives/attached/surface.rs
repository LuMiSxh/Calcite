// SPDX-License-Identifier: MPL-2.0

//! The morphing surface: its look, its frame at each `t`, and the close rebound.

use gpui::{GlassFilter, Rgba};

use crate::{
	glass::Glass,
	morph::{self, Direction, MAX_OVERSHOOT},
	motion::{Curve, Spring, tokens::MORPH_CLOSE},
};

use super::geometry::body_origin;
use super::{Align, CLOSE_BUMP_PX, CONTENT_SLIDE, MAX_OVERSHOOT_PX, final_width};

impl Look {
	pub fn of(glass: &Glass, edge: Rgba) -> Self {
		Self {
			filter: glass.filter(),
			fill: glass.fill(),
			edge,
		}
	}

	/// The look between `self` (at `t = 0`) and `to` (at `t = 1`).
	pub fn mix(&self, to: &Look, t: f32) -> Look {
		Look {
			filter: match (&self.filter, &to.filter) {
				(Some(from), Some(to)) => Some(morph::mix_filter(from, to, t)),
				_ => None,
			},
			fill: morph::gradient(self.fill, to.fill, t),
			edge: morph::gradient(self.edge, to.edge, t),
		}
	}
}

/// The look of one end of the morph: material, flat fill, and outline.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
	/// `None` under Reduce Transparency.
	pub filter: Option<GlassFilter>,
	pub fill: Rgba,
	pub edge: Rgba,
}

/// The surface at one instant, relative to the trigger's top left corner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Surface {
	pub left: f32,
	pub top: f32,
	pub width: f32,
	pub height: f32,
	pub radius: f32,
	/// The body's final rect (where its content lays out), relative to the trigger.
	pub body_left: f32,
	pub body_top: f32,
	pub body_width: f32,
	/// Opacity of the body's content: 0 in the first half of `t`, rising to 1 in the second.
	pub content: f32,
	/// How far the content is still displaced, in px.
	pub slide: f32,
	/// Strength of the surface's drop shadow, 0 at rest.
	pub shadow: f32,
	/// The clamped progress, for the colour gradient.
	pub unit: f32,
}

fn lerp(from: f32, to: f32, t: f32) -> f32 {
	from + (to - from) * t
}

/// The largest spring value that still moves an edge `span` px: 1 plus the overshoot
/// allowance, which is [`MAX_OVERSHOOT_PX`] but never more than the ratio [`MAX_OVERSHOOT`].
fn max_grow(span: f32) -> f32 {
	if span <= 0. {
		return 1.;
	}
	1. + (MAX_OVERSHOOT_PX / span).min(MAX_OVERSHOOT - 1.)
}

/// The spring that drives the body: [`Curve::Present`] while opening, [`MORPH_CLOSE`] while
/// closing. [`motion::spring`](fn@motion::spring) keeps the velocity when the target flips.
pub fn body_spring(open: bool) -> Spring {
	if open {
		Curve::Present.spring().expect("Present is a spring")
	} else {
		MORPH_CLOSE
	}
}

/// How far the surface sits outside the trigger when the spring is `depth` below 0: the
/// reflected overshoot (`depth` times the longest edge travel) saturates smoothly at
/// [`CLOSE_BUMP_PX`], so a large body rebounds visibly and a small one proportionally.
fn close_bump(depth: f32, span: f32) -> f32 {
	let cap = CLOSE_BUMP_PX;
	if span <= 0. || depth <= 0. {
		return 0.;
	}
	cap * (depth * span / cap).tanh()
}

/// The surface at spring value `t`. At 0 it is exactly the trigger's rect, with the trigger's
/// radius; at 1 it covers the trigger and `body` (the trigger's width when stretched), the body
/// placed `offset` away from the trigger's edge (positive is a gap, negative overlaps the
/// trigger). Overshoot moves an edge at most [`MAX_OVERSHOOT_PX`] past its final place;
/// undershoot (`t < 0`) is reflected outward: the rect swells uniformly past the trigger by at
/// most [`CLOSE_BUMP_PX`] per edge, with the radius growing along. It is never smaller than the
/// trigger.
#[allow(clippy::too_many_arguments)]
pub fn surface_frame(
	t: f32,
	direction: Direction,
	align: Align,
	offset: f32,
	trigger: (f32, f32),
	body: (f32, f32),
	trigger_radius: f32,
	body_radius: f32,
) -> Surface {
	surface_frame_shifted(
		t,
		direction,
		align,
		offset,
		trigger,
		body,
		trigger_radius,
		body_radius,
		(0., 0.),
	)
}

/// [`surface_frame`] with the body's final rect moved by `shift` (see [`body_shift`]).
#[allow(clippy::too_many_arguments)]
pub fn surface_frame_shifted(
	t: f32,
	direction: Direction,
	align: Align,
	offset: f32,
	trigger: (f32, f32),
	body: (f32, f32),
	trigger_radius: f32,
	body_radius: f32,
	shift: (f32, f32),
) -> Surface {
	let unit = t.clamp(0., 1.);
	let (tw, th) = trigger;
	let bw = final_width(align, tw, body.0);
	let bh = body.1.max(0.);
	let (body_left, body_top) = body_origin(direction, align, offset, trigger, (bw, bh));
	let (body_left, body_top) = (body_left + shift.0, body_top + shift.1);
	// The final surface is the trigger and the body together. Each edge travels from the
	// trigger's edge to its final place; an edge that does not move never moves.
	let edge = |from: f32, to: f32| lerp(from, to, t.clamp(0., max_grow((to - from).abs())));
	let (to_left, to_top) = (body_left.min(0.), body_top.min(0.));
	let (to_right, to_bottom) = ((body_left + bw).max(tw), (body_top + bh).max(th));
	let bump = if t < 0. {
		let span = [-to_left, -to_top, to_right - tw, to_bottom - th]
			.into_iter()
			.fold(0., f32::max);
		close_bump(-t, span)
	} else {
		0.
	};
	let left = edge(0., to_left) - bump;
	let top = edge(0., to_top) - bump;
	let right = edge(tw, to_right) + bump;
	let bottom = edge(th, to_bottom) + bump;
	let (width, height) = (right - left, bottom - top);
	let content = ((unit - 0.5) / 0.5).clamp(0., 1.);
	Surface {
		left,
		top,
		width,
		height,
		radius: (lerp(trigger_radius, body_radius, unit) + bump)
			.min(width.min(height) / 2.)
			.max(0.),
		body_left,
		body_top,
		body_width: bw,
		content,
		slide: (1. - content) * CONTENT_SLIDE,
		shadow: unit,
		unit,
	}
}

/// How much of its own glass and border the trigger paints while the surface exists (when it does
/// not, the trigger always paints it all). With glass the surface owns the look from its first to
/// its last frame, so this is 0 throughout: the two are never drawn together, and the frame the
/// surface leaves the trigger takes over a look identical to it. Without glass (Reduce
/// Transparency) it crossfades with the surface.
pub fn trigger_rim(t: f32, glass: bool) -> f32 {
	if glass { 0. } else { 1. - t.clamp(0., 1.) }
}

/// Whether the body is still part of the tree: until the spring has settled closed.
/// `active` is the spring's own verdict (still moving), which also keeps the body for the very
/// last frames, so it leaves exactly when the spring snaps onto 0 with no velocity.
pub fn body_present(t: f32, open: bool, active: bool) -> bool {
	open || active || t > 0.
}
