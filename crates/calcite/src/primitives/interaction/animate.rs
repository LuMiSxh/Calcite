// SPDX-License-Identifier: MPL-2.0

//! Springs and presence: the values that animate a control in and out.

use std::time::Instant;

use gpui::{Animated, App, ElementId, Styled, Window, div, px};

use crate::motion::{self, Curve};

/// A spring-smoothed value that follows `target`, keyed by `id` across frames. Spring curves keep
/// their velocity when the target changes mid-motion (see [`motion::spring()`]); the timed curves
/// (`Crossfade`, `Select`, `Progress`) ease from the current value instead.
pub fn spring_value(
	id: impl Into<ElementId>,
	target: f32,
	curve: Curve,
	reduce_motion: bool,
	window: &mut Window,
	cx: &mut App,
) -> f32 {
	let reduce_motion = reduce_motion || super::reduce_motion(cx);
	if let Some(spring) = curve.spring() {
		return motion::spring(id, target, spring, reduce_motion, window, cx).value;
	}
	let state = window.use_keyed_state(id, cx, |_, _| {
		Animated::new(target, curve.transition(reduce_motion))
	});
	let now = Instant::now();
	let sample = state.update(cx, |value, _| {
		if *value.value() != target {
			value.set(target, curve.transition(reduce_motion), now);
		}
		value.sample(now)
	});
	if sample.is_active && !reduce_motion {
		window.request_animation_frame();
	}
	sample.value
}

/// The presence of something that appears and disappears, such as a menu or a modal:
/// 0 hidden, 1 shown. It enters on [`Curve::Present`] and leaves on [`Curve::Dismiss`].
/// Returns `None` once it is fully gone, so the caller stops drawing it; until then the
/// caller keeps drawing it while it fades out. Call it on every render, shown or not.
pub fn presence(
	id: impl Into<ElementId>,
	shown: bool,
	reduce_motion: bool,
	window: &mut Window,
	cx: &mut App,
) -> Option<f32> {
	let target = if shown { 1. } else { 0. };
	let reduce_motion = reduce_motion || super::reduce_motion(cx);
	let curve = if shown { Curve::Present } else { Curve::Dismiss };
	let spring = curve.spring().unwrap_or(motion::tokens::SMOOTH);
	let sample = motion::spring_from(id, 0., target, spring, reduce_motion, window, cx);
	(shown || sample.active).then_some(sample.value)
}

/// An empty absolute layer that fills its parent; components paint surfaces on it.
pub fn layer() -> gpui::Div {
	div()
		.absolute()
		.top(px(0.))
		.bottom(px(0.))
		.left(px(0.))
		.right(px(0.))
}
