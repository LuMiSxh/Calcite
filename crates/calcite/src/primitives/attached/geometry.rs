// SPDX-License-Identifier: MPL-2.0

//! Where the surface sits: the placement, the alignment, and the window fit.

use crate::morph::Direction;

/// Which side of the trigger the body grows toward.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
	Down,
	Up,
	/// Down, unless the final body would leave the window and up fits better.
	Auto,
}

/// How the body lines up with the trigger along the trigger's edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
	/// Left edges aligned.
	Start,
	Center,
	/// Right edges aligned.
	End,
	/// The body is exactly as wide as the trigger.
	Stretch,
}

/// The body's top left corner relative to the trigger's, before any window shift. `body` is the
/// final size ([`final_width`] already applied).
pub(super) fn body_origin(
	direction: Direction,
	align: Align,
	offset: f32,
	trigger: (f32, f32),
	body: (f32, f32),
) -> (f32, f32) {
	let left = match align {
		Align::Start | Align::Stretch => 0.,
		Align::Center => (trigger.0 - body.0) / 2.,
		Align::End => trigger.0 - body.0,
	};
	let top = match direction {
		Direction::Down => trigger.1 + offset,
		Direction::Up => -offset - body.1,
	};
	(left, top)
}

/// The width the body ends at: the trigger's for [`Align::Stretch`], otherwise its own.
pub fn final_width(align: Align, trigger_width: f32, body_width: f32) -> f32 {
	match align {
		Align::Stretch => trigger_width,
		_ => body_width,
	}
}

/// How far the body must move, `(dx, dy)`, to stay `margin` inside the window. `origin` is the
/// trigger's top left in window coordinates, `body` the final size (already capped to the window),
/// `viewport` the window size. When the body is larger than the window allows, its top left edge
/// wins, so the start of the content stays visible. [`Align::Stretch`] bodies never shift
/// horizontally: they are the trigger's width and stay attached to it.
#[allow(clippy::too_many_arguments)]
pub fn body_shift(
	direction: Direction,
	align: Align,
	offset: f32,
	origin: (f32, f32),
	trigger: (f32, f32),
	body: (f32, f32),
	viewport: (f32, f32),
	margin: f32,
) -> (f32, f32) {
	let (left, top) = body_origin(direction, align, offset, trigger, body);
	let fit = |pos: f32, size: f32, limit: f32| {
		let hi = (limit - margin - size).max(margin);
		pos.clamp(margin, hi) - pos
	};
	let dx = if align == Align::Stretch {
		0.
	} else {
		fit(origin.0 + left, body.0, viewport.0)
	};
	(dx, fit(origin.1 + top, body.1, viewport.1))
}

/// Which way the body grows, from its final height and the window. Call it once per opening.
/// `Auto` prefers down; if the body does not fit below, it goes up when that fits or has more room.
pub fn resolve_direction(
	placement: Placement,
	trigger_top: f32,
	trigger_height: f32,
	body_height: f32,
	offset: f32,
	viewport_height: f32,
	margin: f32,
) -> Direction {
	match placement {
		Placement::Down => Direction::Down,
		Placement::Up => Direction::Up,
		Placement::Auto => {
			let below_end = trigger_top + trigger_height + offset + body_height;
			if below_end + margin <= viewport_height {
				return Direction::Down;
			}
			let above_start = trigger_top - offset - body_height;
			if above_start >= margin {
				return Direction::Up;
			}
			let room_below = viewport_height - (trigger_top + trigger_height);
			if trigger_top > room_below {
				Direction::Up
			} else {
				Direction::Down
			}
		}
	}
}
