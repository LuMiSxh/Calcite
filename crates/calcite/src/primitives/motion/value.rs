// SPDX-License-Identifier: MPL-2.0

//! A spring-smoothed value that keeps its velocity when the target changes mid-motion.

use std::time::Instant;

use gpui::{App, ElementId, SpringState as Physics, Window};

use super::{secondary, spring::Spring};

/// The longest step the simulation takes at once; a stalled frame must not catapult the value.
pub const MAX_DT: f32 = 1. / 30.;
/// A value this close to its target (in the animated unit) counts as arrived...
pub const SETTLE_DISTANCE: f32 = 0.001;
/// ...once it is also moving slower than this (units per second).
pub const SETTLE_SPEED: f32 = 0.01;

/// Position and velocity of a spring-driven value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpringState {
	pub value: f32,
	/// Change per second.
	pub velocity: f32,
}

impl SpringState {
	pub const fn at_rest(value: f32) -> Self {
		Self { value, velocity: 0. }
	}

	/// Whether the value has arrived: close to `target` and no longer moving.
	pub fn is_settled(self, target: f32) -> bool {
		(self.value - target).abs() < SETTLE_DISTANCE && self.velocity.abs() < SETTLE_SPEED
	}
}

/// One frame of a spring value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpringSample {
	pub value: f32,
	/// Change per second; feeds secondary motion such as stretch.
	pub velocity: f32,
	/// Whether the value is still moving, so the caller must ask for another frame.
	pub active: bool,
}

impl SpringSample {
	pub const fn at_rest(value: f32) -> Self {
		Self {
			value,
			velocity: 0.,
			active: false,
		}
	}
}

/// Advances `state` towards `target` by `dt` seconds (clamped to [`MAX_DT`]). The velocity carries
/// over, so a retarget in mid-motion bends the path instead of restarting from rest. Once settled
/// the state snaps onto the target with zero velocity.
pub fn advance(state: SpringState, target: f32, spring: Spring, dt: f32) -> (SpringState, bool) {
	if state.is_settled(target) {
		return (SpringState::at_rest(target), false);
	}
	let dt = if dt.is_nan() { 0. } else { dt.clamp(0., MAX_DT) };
	let next = spring.to_config().step(
		Physics {
			position: state.value,
			velocity: state.velocity,
		},
		target,
		dt,
	);
	let next = SpringState {
		value: next.position,
		velocity: next.velocity,
	};
	if next.is_settled(target) {
		(SpringState::at_rest(target), false)
	} else {
		(next, true)
	}
}

struct Cell {
	state: SpringState,
	last: Instant,
	/// Whether the previous frame ended in motion.
	moving: bool,
}

/// A spring-smoothed value that follows `target`, keyed by `id` across frames. Retargeting keeps
/// the velocity. It asks for the next frame while moving. Reduce Motion jumps to the target.
pub fn spring(
	id: impl Into<ElementId>,
	target: f32,
	spring: Spring,
	reduce_motion: bool,
	window: &mut Window,
	cx: &mut App,
) -> SpringSample {
	spring_from(id, target, target, spring, reduce_motion, window, cx)
}

/// The last target a [`landing`] saw, and when it changed.
struct Landing {
	target: f32,
	since: Option<Instant>,
}

/// The rim light a view shows as `target` lands. The light starts when `target` changes, waits out
/// `spring`'s settle time, then fades over [`secondary::landing_pulse`]. Zero at rest and under
/// Reduce Motion. It asks for frames only while the pulse is pending or running.
pub fn landing(
	id: impl Into<ElementId>,
	target: f32,
	spring: Spring,
	reduce_motion: bool,
	window: &mut Window,
	cx: &mut App,
) -> f32 {
	if reduce_motion || crate::interaction::reduce_motion(cx) {
		return 0.;
	}
	let cell = window.use_keyed_state(id, cx, |_, _| Landing { target, since: None });
	let now = Instant::now();
	let elapsed = cell.update(cx, |cell, _| {
		if cell.target != target {
			cell.target = target;
			cell.since = Some(now);
		}
		cell.since
			.map(|since| now.saturating_duration_since(since).as_secs_f32())
	});
	let Some(elapsed) = elapsed else {
		return 0.;
	};
	let after = elapsed - spring.settle_time();
	if after < 0. {
		window.request_animation_frame();
		return 0.;
	}
	if after < secondary::LANDING_DURATION {
		window.request_animation_frame();
	}
	secondary::landing_pulse(after, false)
}

/// Pins the spring `id` at `value` at rest, for a value that something else (a drag) drives for a
/// while. When [`spring`] resumes it moves on from `value` with no velocity.
pub fn hold(id: impl Into<ElementId>, value: f32, window: &mut Window, cx: &mut App) {
	let cell = window.use_keyed_state(id, cx, |_, _| Cell {
		state: SpringState::at_rest(value),
		last: Instant::now(),
		moving: false,
	});
	cell.update(cx, |cell, _| {
		cell.state = SpringState::at_rest(value);
		cell.last = Instant::now();
		cell.moving = false;
	});
}

/// Like [`spring`], but a state created now starts at `initial` and moves to `target`.
pub fn spring_from(
	id: impl Into<ElementId>,
	initial: f32,
	target: f32,
	spring: Spring,
	reduce_motion: bool,
	window: &mut Window,
	cx: &mut App,
) -> SpringSample {
	// The system setting counts as well as the caller's flag: the value snaps and asks for no frames.
	let reduce_motion = reduce_motion || crate::interaction::reduce_motion(cx);
	let cell = window.use_keyed_state(id, cx, |_, _| Cell {
		state: SpringState::at_rest(initial),
		last: Instant::now(),
		moving: false,
	});
	let now = Instant::now();
	let sample = cell.update(cx, |cell, _| {
		if reduce_motion {
			cell.state = SpringState::at_rest(target);
			cell.last = now;
			cell.moving = false;
			return SpringSample::at_rest(target);
		}
		let dt = now.saturating_duration_since(cell.last).as_secs_f32();
		cell.last = now;
		// A spring that was idle has no meaningful elapsed time: its first frame is the retarget.
		let (next, active) = advance(cell.state, target, spring, if cell.moving { dt } else { 0. });
		cell.state = next;
		cell.moving = active;
		SpringSample {
			value: next.value,
			velocity: next.velocity,
			active,
		}
	});
	if sample.active {
		window.request_animation_frame();
	}
	sample
}

#[cfg(test)]
mod tests {
	use super::*;

	const SPRING: Spring = Spring::new(0.4, 0.3);

	fn run(mut state: SpringState, target: f32, spring: Spring, seconds: f32) -> SpringState {
		let frames = (seconds * 120.) as usize;
		for _ in 0..frames {
			state = advance(state, target, spring, 1. / 120.).0;
		}
		state
	}

	#[test]
	fn converges_to_the_target_and_reports_rest() {
		let mut state = SpringState::at_rest(0.);
		let mut active = true;
		for _ in 0..600 {
			(state, active) = advance(state, 1., SPRING, 1. / 60.);
			if !active {
				break;
			}
		}
		assert!(!active, "still moving: {state:?}");
		assert_eq!(state, SpringState::at_rest(1.));
	}

	#[test]
	fn a_value_at_its_target_is_inactive() {
		let (state, active) = advance(SpringState::at_rest(0.5), 0.5, SPRING, 1. / 60.);
		assert!(!active);
		assert_eq!(state, SpringState::at_rest(0.5));
	}

	#[test]
	fn overshoot_matches_the_closed_form() {
		let mut state = SpringState::at_rest(0.);
		let mut peak = 0.0_f32;
		// 1/60 s frames stay under the dt clamp, so this is the real playback.
		for _ in 0..240 {
			state = advance(state, 1., SPRING, 1. / 60.).0;
			peak = peak.max(state.value);
		}
		assert!((peak - 1. - SPRING.overshoot()).abs() < 0.01, "peak {peak}");
	}

	#[test]
	fn critically_damped_never_overshoots() {
		let mut state = SpringState::at_rest(0.);
		for _ in 0..240 {
			state = advance(state, 1., Spring::smooth(0.3), 1. / 60.).0;
			assert!(state.value <= 1. + 1e-4, "{state:?}");
		}
	}

	#[test]
	fn retargeting_keeps_the_velocity() {
		let mid = run(SpringState::at_rest(0.), 1., SPRING, 0.12);
		assert!(mid.velocity > 1., "should be mid-flight: {mid:?}");
		let (after, active) = advance(mid, -1., SPRING, 0.);
		assert!(active);
		assert_eq!(after.velocity, mid.velocity);
		assert!((after.value - mid.value).abs() < 1e-5);
		// The path bends rather than restarting from rest: the value keeps rising a little.
		let (later, _) = advance(mid, -1., SPRING, 1. / 60.);
		assert!(later.value > mid.value, "{later:?} vs {mid:?}");
	}

	#[test]
	fn the_time_step_is_clamped() {
		let start = SpringState::at_rest(0.);
		let huge = advance(start, 1., SPRING, 5.).0;
		let clamped = advance(start, 1., SPRING, MAX_DT).0;
		assert_eq!(huge, clamped);
		assert_eq!(advance(start, 1., SPRING, -1.).0, start);
		assert_eq!(advance(start, 1., SPRING, f32::NAN).0, start);
	}

	#[test]
	fn settling_needs_both_position_and_rest() {
		assert!(
			SpringState {
				value: 1.,
				velocity: 0.
			}
			.is_settled(1.)
		);
		assert!(
			SpringState {
				value: 1.0005,
				velocity: 0.005
			}
			.is_settled(1.)
		);
		assert!(
			!SpringState {
				value: 1.01,
				velocity: 0.
			}
			.is_settled(1.)
		);
		// At the target but still moving fast: about to overshoot, not arrived.
		assert!(
			!SpringState {
				value: 1.,
				velocity: 0.5
			}
			.is_settled(1.)
		);
	}
}
