// SPDX-License-Identifier: MPL-2.0

use super::geometry::body_origin;
use super::*;
use crate::tokens::{black, white};
use crate::{
	morph::MAX_OVERSHOOT,
	motion::{Curve, tokens::MORPH_CLOSE},
};

const TRIGGER: (f32, f32) = (80., 24.);
const BODY: (f32, f32) = (180., 120.);

fn frame_at(t: f32, direction: Direction, align: Align, offset: f32) -> Surface {
	surface_frame(t, direction, align, offset, TRIGGER, BODY, 12., 16.)
}

#[test]
fn endpoints_are_exact() {
	let closed = frame_at(0., Direction::Down, Align::Start, 0.);
	assert_eq!((closed.left, closed.top), (0., 0.));
	assert_eq!((closed.width, closed.height, closed.radius), (80., 24., 12.));
	assert_eq!((closed.content, closed.shadow, closed.unit), (0., 0., 0.));
	let open = frame_at(1., Direction::Down, Align::Start, 0.);
	assert_eq!((open.width, open.height, open.radius), (180., 144., 16.));
	assert_eq!((open.content, open.shadow, open.slide), (1., 1., 0.));
	let half = frame_at(0.5, Direction::Down, Align::Start, 0.);
	assert!((half.radius - 14.).abs() < 1e-5);
}

#[test]
fn at_rest_the_surface_is_exactly_the_trigger_for_every_placement() {
	for direction in [Direction::Down, Direction::Up] {
		for align in [Align::Start, Align::Center, Align::End, Align::Stretch] {
			for offset in [-24., 0., 4.] {
				let s = frame_at(0., direction, align, offset);
				assert_eq!(
					(s.left, s.top, s.width, s.height, s.radius),
					(0., 0., TRIGGER.0, TRIGGER.1, 12.),
					"{direction:?} {align:?} offset {offset}"
				);
				// A hair below 0 is a hair outside the trigger: continuous at the crossing.
				let s = frame_at(-1e-6, direction, align, offset);
				assert!(s.left < 1e-3 && s.top < 1e-3 && (s.width - TRIGGER.0).abs() < 1e-3);
				assert!((s.height - TRIGGER.1).abs() < 1e-3 && (s.radius - 12.).abs() < 1e-3);
			}
		}
	}
	// A capsule trigger stays a capsule: radius is half the height.
	let capsule = surface_frame(
		0.,
		Direction::Down,
		Align::End,
		-28.,
		(28., 28.),
		(260., 300.),
		14.,
		20.,
	);
	assert_eq!((capsule.width, capsule.height, capsule.radius), (28., 28., 14.));
}

#[test]
fn undershoot_bumps_outside_the_trigger_by_at_most_the_cap_and_overshoot_is_capped() {
	for direction in [Direction::Down, Direction::Up] {
		for align in [Align::Start, Align::Center, Align::End, Align::Stretch] {
			let mut last_bump = 0.;
			for step in 1..=400 {
				let t = -(step as f32) * 0.005;
				let s = frame_at(t, direction, align, 0.);
				let bump = -s.left;
				// Uniform on all four edges, outside the trigger, never past the cap.
				assert!((-s.top - bump).abs() < 1e-4, "{direction:?} {align:?} t {t}");
				assert!((s.left + s.width - TRIGGER.0 - bump).abs() < 1e-4);
				assert!((s.top + s.height - TRIGGER.1 - bump).abs() < 1e-4);
				assert!(bump > 0. && bump <= CLOSE_BUMP_PX + 1e-4, "bump {bump}");
				assert!(s.width >= TRIGGER.0 && s.height >= TRIGGER.1);
				assert!(s.radius <= s.width.min(s.height) / 2. + 1e-4);
				// Deeper rebound, bigger bump.
				assert!(bump >= last_bump - 1e-5);
				last_bump = bump;
				assert_eq!((s.unit, s.shadow, s.content), (0., 0., 0.));
			}
			assert!(
				last_bump > CLOSE_BUMP_PX - 0.1,
				"a deep rebound nearly reaches the cap"
			);
		}
	}
	// The radius grows with the rect: a capsule stays a capsule.
	let capsule = |t: f32| {
		surface_frame(
			t,
			Direction::Down,
			Align::End,
			-28.,
			(28., 28.),
			(260., 300.),
			14.,
			20.,
		)
	};
	for t in [-0.001, -0.02, -0.08, -0.5] {
		let s = capsule(t);
		assert!(
			(s.radius - s.width.min(s.height) / 2.).abs() < 1e-4,
			"t {t}: {s:?}"
		);
	}
	let s = frame_at(1.04, Direction::Down, Align::Start, 0.);
	assert!(s.width > 180. && s.height > 144.);
	let capped = frame_at(9., Direction::Down, Align::Start, 0.);
	// 2 px past the final edge, not a ratio of the growth.
	assert!((capped.height - 146.).abs() < 1e-3, "{}", capped.height);
	assert!((capped.width - 182.).abs() < 1e-3, "{}", capped.width);
	assert_eq!(capped.radius, 16.);
	// A big body overshoots by the same few px as a small one.
	let big = surface_frame(
		9.,
		Direction::Down,
		Align::End,
		0.,
		(28., 28.),
		(260., 300.),
		14.,
		20.,
	);
	assert!((big.height - 330.).abs() < 1e-3 && (big.width - 262.).abs() < 1e-3);
	// A tiny body keeps the ratio cap of the morph module.
	let tiny = surface_frame(
		9.,
		Direction::Down,
		Align::Start,
		0.,
		(80., 24.),
		(80., 10.),
		12.,
		4.,
	);
	assert!((tiny.height - (24. + 10. * MAX_OVERSHOOT)).abs() < 1e-3);
}

#[test]
fn radius_never_exceeds_half_the_short_side() {
	for step in 0..=120 {
		let s = surface_frame(
			step as f32 / 100.,
			Direction::Down,
			Align::Start,
			0.,
			TRIGGER,
			BODY,
			12.,
			80.,
		);
		assert!(s.radius <= s.width.min(s.height) / 2. + 1e-4);
	}
}

#[test]
fn body_attaches_to_the_chosen_edge_with_its_offset() {
	let down = frame_at(1., Direction::Down, Align::Start, 2.);
	assert_eq!((down.body_top, down.top, down.height), (26., 0., 146.));
	let up = frame_at(1., Direction::Up, Align::Start, 2.);
	assert_eq!(up.body_top + 120., -2.);
	assert_eq!((up.top, up.top + up.height), (-122., 24.));
	let overlap = frame_at(1., Direction::Down, Align::Start, -24.);
	assert_eq!((overlap.top, overlap.body_top, overlap.height), (0., 0., 120.));
	// The edge on the trigger stays put while the surface grows.
	for t in [0.2, 0.6, 1.05] {
		let up = frame_at(t, Direction::Up, Align::Start, 1.);
		assert!((up.top + up.height - 24.).abs() < 1e-4);
		let down = frame_at(t, Direction::Down, Align::Start, 1.);
		assert_eq!(down.top, 0.);
	}
}

#[test]
fn alignment_anchors_an_edge_while_growing() {
	for t in [0., 0.3, 1., 1.05] {
		let start = frame_at(t, Direction::Down, Align::Start, 0.);
		assert_eq!(start.left, 0.);
		let end = frame_at(t, Direction::Down, Align::End, 0.);
		assert!((end.left + end.width - TRIGGER.0).abs() < 1e-4);
		let center = frame_at(t, Direction::Down, Align::Center, 0.);
		assert!((center.left + center.width / 2. - TRIGGER.0 / 2.).abs() < 1e-4);
	}
	let stretch = frame_at(1., Direction::Down, Align::Stretch, 0.);
	assert_eq!((stretch.left, stretch.width), (0., 80.));
	assert_eq!(final_width(Align::Stretch, 80., 180.), 80.);
	assert_eq!(final_width(Align::End, 80., 180.), 180.);
}

#[test]
fn content_arrives_in_the_second_half_and_leaves_in_the_first() {
	assert_eq!(frame_at(0.5, Direction::Down, Align::Start, 0.).content, 0.);
	assert_eq!(frame_at(0.3, Direction::Down, Align::Start, 0.).content, 0.);
	let late = frame_at(0.75, Direction::Down, Align::Start, 0.);
	assert!((late.content - 0.5).abs() < 1e-6);
	assert!(late.slide > 0.);
}

#[test]
fn trigger_rim_never_overlaps_the_surface_with_glass() {
	// Including the bump: the surface alone is the glass, so crossing t = 0 switches nothing.
	for t in [-0.3, -0.02, -1e-6, 0., 0.0005, 0.001, 0.5, 1., 1.06] {
		assert_eq!(trigger_rim(t, true), 0., "t = {t}");
	}
}

#[test]
fn the_bump_always_covers_the_trigger_with_the_surface_glass() {
	// With glass, either the surface exists and the trigger paints no glass, or the reverse;
	// the surface contains the trigger's rect on every moving frame (no hole) and sticks out
	// by at most the cap.
	let (_, open) = run(motion::SpringState::at_rest(0.), true, 600);
	let (frames, _) = run(open, false, 600);
	assert!(frames.iter().any(|(t, _)| *t < 0.), "the close never rebounded");
	// The last frame is the settled one: exactly 0, where the surface is removed.
	let (settled, moving) = frames.split_last().expect("frames");
	assert!(!settled.1 && settled.0 == 0.);
	for (t, active) in moving {
		let present = body_present(*t, false, *active);
		assert!(present, "surface dropped at t = {t} while still moving");
		assert_eq!(trigger_rim(*t, true), 0.);
		let s = surface_frame(
			*t,
			Direction::Down,
			Align::End,
			-28.,
			(28., 28.),
			(260., 300.),
			14.,
			20.,
		);
		let c = CLOSE_BUMP_PX + 1e-4;
		assert!(s.left <= 1e-4 && s.top <= 1e-4);
		assert!(s.left + s.width >= 28. - 1e-4 && s.top + s.height >= 28. - 1e-4);
		if *t < 0. {
			assert!(s.left >= -c && s.top >= -c);
			assert!(s.left + s.width <= 28. + c && s.top + s.height <= 28. + c);
		}
	}
	// The bump is visible: several px, not a rounding error.
	let widest = frames
		.iter()
		.map(|(t, _)| {
			surface_frame(
				*t,
				Direction::Down,
				Align::End,
				-28.,
				(28., 28.),
				(260., 300.),
				14.,
				20.,
			)
			.left
		})
		.fold(0., f32::min);
	assert!(widest < -2., "bump only {} px", -widest);
}

#[test]
fn the_close_spring_rebounds_further_than_the_calm_one() {
	assert!(MORPH_CLOSE.overshoot() > 0.07);
}

#[test]
fn the_handover_at_the_end_of_a_close_has_no_gap_or_overlap() {
	let (_, open) = run(motion::SpringState::at_rest(0.), true, 600);
	let (frames, _) = run(open, false, 600);
	// Each rendered frame: the surface exists exactly while the body is present, and the
	// trigger's rim is 0 exactly then, 1 otherwise.
	let coverage = |t: f32, active: bool| {
		let present = body_present(t, false, active);
		let rim = if present { trigger_rim(t, true) } else { 1. };
		(present as u8 as f32) + rim
	};
	for (t, active) in &frames {
		assert_eq!(coverage(*t, *active), 1., "t = {t}");
	}
	// The frame after the last animated one: the surface is gone, the trigger paints all.
	assert_eq!(coverage(0., false), 1.);
	// And the surface that is removed is the trigger's exact rect and radius.
	let last = surface_frame(
		frames.last().unwrap().0,
		Direction::Down,
		Align::End,
		-28.,
		(28., 28.),
		(260., 300.),
		14.,
		20.,
	);
	assert_eq!((last.width, last.height, last.radius), (28., 28., 14.));
}

#[test]
fn trigger_rim_crossfades_without_glass() {
	assert_eq!(trigger_rim(0., false), 1.);
	assert_eq!(trigger_rim(1., false), 0.);
	assert_eq!(trigger_rim(1.06, false), 0.);
	assert!((trigger_rim(0.25, false) - 0.75).abs() < 1e-6);
}

#[test]
fn the_body_stays_until_settled_closed() {
	// Gone only when closed, at 0 and no longer moving.
	assert!(!body_present(0., false, false));
	assert!(body_present(0.3, false, true));
	assert!(body_present(0.0004, false, true));
	assert!(body_present(0., true, false));
	assert!(body_present(0., false, true));
}

#[test]
fn forced_placements_ignore_the_window() {
	assert_eq!(
		resolve_direction(Placement::Down, 700., 24., 500., 0., 800., 8.),
		Direction::Down
	);
	assert_eq!(
		resolve_direction(Placement::Up, 10., 24., 500., 0., 800., 8.),
		Direction::Up
	);
}

#[test]
fn auto_flips_up_only_when_down_does_not_fit() {
	let auto = |top, body_h, vp| resolve_direction(Placement::Auto, top, 24., body_h, 0., vp, 8.);
	assert_eq!(auto(100., 120., 800.), Direction::Down);
	assert_eq!(auto(700., 120., 800.), Direction::Up);
	// Neither fits, more room above.
	assert_eq!(auto(300., 500., 600.), Direction::Up);
	// Neither fits, more room below.
	assert_eq!(auto(40., 500., 400.), Direction::Down);
	// Exactly fitting counts as fitting.
	assert_eq!(auto(100., 800. - 124. - 8., 800.), Direction::Down);
	// The offset is part of the reach.
	assert_eq!(
		resolve_direction(Placement::Auto, 640., 24., 100., 0., 800., 8.),
		Direction::Down
	);
	assert_eq!(
		resolve_direction(Placement::Auto, 640., 24., 100., 50., 800., 8.),
		Direction::Up
	);
}

/// The body's final rect in window coordinates after the same steps `render` takes:
/// resolve the side, cap the size to the window, shift inside it.
fn placed(
	placement: Placement,
	align: Align,
	origin: (f32, f32),
	viewport: (f32, f32),
	body: (f32, f32),
) -> (Direction, [f32; 4]) {
	let margin = 8.;
	let offset = 4.;
	let direction = resolve_direction(placement, origin.1, TRIGGER.1, body.1, offset, viewport.1, margin);
	let cap = |size: f32, limit: f32| size.min((limit - 2. * margin).max(0.));
	let body = (cap(body.0, viewport.0), cap(body.1, viewport.1));
	let body = (final_width(align, TRIGGER.0, body.0), body.1);
	let (dx, dy) = body_shift(direction, align, offset, origin, TRIGGER, body, viewport, margin);
	let (left, top) = body_origin(direction, align, offset, TRIGGER, body);
	let (x, y) = (origin.0 + left + dx, origin.1 + top + dy);
	(direction, [x, y, x + body.0, y + body.1])
}

#[test]
fn the_body_stays_inside_the_window_for_triggers_at_every_edge_and_corner() {
	let vp = (400., 300.);
	let xs = [0., 160., vp.0 - TRIGGER.0];
	let ys = [0., 140., vp.1 - TRIGGER.1];
	for placement in [Placement::Auto, Placement::Down, Placement::Up] {
		for align in [Align::Start, Align::Center, Align::End] {
			for x in xs {
				for y in ys {
					let (_, [l, t, r, b]) = placed(placement, align, (x, y), vp, BODY);
					let ctx = format!("{placement:?} {align:?} at ({x}, {y}): {l} {t} {r} {b}");
					assert!(l >= 8. - 1e-3 && t >= 8. - 1e-3, "{ctx}");
					assert!(r <= vp.0 - 8. + 1e-3 && b <= vp.1 - 8. + 1e-3, "{ctx}");
				}
			}
		}
	}
}

#[test]
fn the_body_shifts_horizontally_at_the_left_and_right_edges() {
	let vp = (400., 300.);
	// Start-aligned at the right edge would overflow: it moves left by exactly the overflow.
	let (_, [l, _, r, _]) = placed(Placement::Down, Align::Start, (vp.0 - TRIGGER.0, 100.), vp, BODY);
	assert_eq!(r, vp.0 - 8.);
	assert_eq!(l, r - BODY.0);
	// End-aligned at the left edge would overflow: it moves right.
	let (_, [l, _, r, _]) = placed(Placement::Down, Align::End, (0., 100.), vp, BODY);
	assert_eq!(l, 8.);
	assert_eq!(r, 8. + BODY.0);
	// A body that already fits does not move.
	let (_, [l, ..]) = placed(Placement::Down, Align::Start, (100., 100.), vp, BODY);
	assert_eq!(l, 100.);
	// A stretched body stays exactly as wide as, and attached to, the trigger.
	assert_eq!(
		body_shift(
			Direction::Down,
			Align::Stretch,
			0.,
			(390., 10.),
			TRIGGER,
			TRIGGER,
			vp,
			8.
		)
		.0,
		0.
	);
}

#[test]
fn a_window_shorter_than_the_body_caps_it_inside_the_margins() {
	let vp = (400., 100.);
	for placement in [Placement::Auto, Placement::Down, Placement::Up] {
		for y in [0., 40., 76.] {
			let (_, [_, t, _, b]) = placed(placement, Align::Start, (100., y), vp, BODY);
			assert_eq!((t, b), (8., vp.1 - 8.), "{placement:?} at {y}");
		}
	}
}

#[test]
fn auto_picks_the_side_with_more_room_and_clamps_when_neither_fits() {
	let vp = (400., 300.);
	// Trigger low in the window: more room above.
	let (d, [_, t, _, b]) = placed(Placement::Auto, Align::Start, (100., 240.), vp, BODY);
	assert_eq!(d, Direction::Up);
	assert!(t >= 8. && b <= 240.);
	// Trigger high in the window: more room below.
	let (d, [_, t, _, b]) = placed(Placement::Auto, Align::Start, (100., 20.), vp, BODY);
	assert_eq!(d, Direction::Down);
	assert!(t >= 20. + TRIGGER.1 && b <= vp.1 - 8.);
	// Neither side fits a 250 px body in 300 px: the side with more room wins and is clamped in.
	let tall = (180., 250.);
	let (d, [_, t, _, b]) = placed(Placement::Auto, Align::Start, (100., 200.), vp, tall);
	assert_eq!(d, Direction::Up);
	assert!(t >= 8. && b <= vp.1 - 8.);
}

#[test]
fn opening_keeps_present_and_closing_uses_the_rebounding_spring() {
	assert_eq!(body_spring(true), Curve::Present.spring().unwrap());
	assert_eq!(body_spring(false), MORPH_CLOSE);
	assert!(MORPH_CLOSE.response <= body_spring(true).response);
}

/// Steps the body spring the way `render` does, one frame at a time, and returns each
/// frame's `(t, active)`.
fn run(mut state: motion::SpringState, open: bool, frames: usize) -> (Vec<(f32, bool)>, motion::SpringState) {
	let target = if open { 1. } else { 0. };
	let mut out = Vec::new();
	for _ in 0..frames {
		let (next, active) = motion::value::advance(state, target, body_spring(open), 1. / 120.);
		state = next;
		out.push((state.value, active));
		if !active {
			break;
		}
	}
	(out, state)
}

#[test]
fn closing_bumps_a_little_past_the_trigger_and_settles_without_a_snap() {
	// A big body on a small round trigger, like the shortcut legend.
	let frame = |t: f32| {
		surface_frame(
			t,
			Direction::Down,
			Align::End,
			-28.,
			(28., 28.),
			(260., 300.),
			14.,
			20.,
		)
	};
	let (_, open) = run(motion::SpringState::at_rest(0.), true, 600);
	let (frames, end) = run(open, false, 600);
	assert_eq!(end, motion::SpringState::at_rest(0.));
	assert!(!frames.last().unwrap().1);
	// Never smaller than the trigger, at most the cap past it, and the last frame is exactly its
	// rect and radius.
	let all: Vec<Surface> = frames.iter().map(|(t, _)| frame(*t)).collect();
	for (s, (t, _)) in all.iter().zip(&frames) {
		let c = CLOSE_BUMP_PX + 1e-4;
		assert!(s.width >= 28. - 1e-4 && s.height >= 28. - 1e-4 && s.left <= 1e-4 && s.top <= 1e-4);
		assert!(*t >= 0. || (s.left >= -c && s.top >= -c));
	}
	let last = all[all.len() - 1];
	assert_eq!(
		(last.left, last.top, last.width, last.height, last.radius),
		(0., 0., 28., 28., 14.)
	);
	let before = all[all.len() - 2];
	assert!(
		before.height - last.height < 0.35,
		"snap of {} px",
		before.height - last.height
	);
	// It falls monotonically to the trigger, bumps past it by at most the cap, and settles.
	let high_after = frames
		.iter()
		.zip(&all)
		.filter(|((t, _), _)| *t < 0.)
		.map(|(_, s)| s.height)
		.fold(0., f32::max);
	assert!(high_after > 28. + 1. && high_after <= 28. + 2. * CLOSE_BUMP_PX + 1e-4);
	let first_bump = frames.iter().position(|(t, _)| *t < 0.).expect("bumps");
	assert!(
		all[..first_bump]
			.windows(2)
			.all(|w| w[1].height <= w[0].height + 1e-3)
	);
}

#[test]
fn reversing_keeps_the_velocity() {
	let (frames, mid) = run(motion::SpringState::at_rest(0.), true, 20);
	assert!(mid.velocity > 0.);
	let (next, _) = motion::value::advance(mid, 0., body_spring(false), 1. / 120.);
	// The velocity bends the path instead of restarting it: still moving up one frame later.
	assert!(next.value > mid.value - 0.001);
	assert!(frames.len() == 20);
}

fn look(fill: Rgba, edge: Rgba, glass: bool) -> Look {
	let filter = glass.then(|| {
		crate::glass::Glass::new(None, crate::GlassStyle::MacOS26)
			.filter()
			.unwrap()
	});
	Look { filter, fill, edge }
}

#[test]
fn look_gradient_hits_both_ends() {
	let from = look(white(0.2), white(0.4), true);
	let mut to = look(black(0.8), black(0.6), true);
	to.filter = crate::glass::Glass::new(Some(white(1.)), crate::GlassStyle::MacOS26)
		.elevation(Elevation::Raised)
		.filter();
	assert_eq!(from.mix(&to, 0.), from);
	assert_eq!(from.mix(&to, 1.), to);
	assert_eq!(from.mix(&to, -1.), from);
	assert_eq!(from.mix(&to, 3.), to);
	// The group's material at t = 0 is exactly what the trigger painted, light angle included.
	let mixed = from.mix(&to, 0.).filter.unwrap();
	assert_eq!(mixed, from.filter.unwrap());
	let mid = from.mix(&to, 0.5);
	assert!((mid.fill.alpha - 0.5).abs() < 1e-6);
	assert!((mid.edge.alpha - 0.5).abs() < 1e-6);
}

#[test]
fn look_without_glass_has_no_filter() {
	let flat = look(white(0.2), white(0.4), false);
	let glass = look(black(0.8), black(0.6), true);
	assert_eq!(flat.mix(&glass, 0.5).filter, None);
	assert_eq!(glass.mix(&flat, 0.5).filter, None);
}
