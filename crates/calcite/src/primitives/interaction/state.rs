// SPDX-License-Identifier: MPL-2.0

//! Per-control state: hover, press, pointer, and the frame each control paints.

use std::time::{Duration, Instant};

use gpui::{Animated, App, Bounds, ElementId, Entity, Pixels, Point, Rgba, Window, px};

use crate::{
	glass::{DEFAULT_POINTER_RADIUS, pointer_radius_for},
	motion::{
		Curve,
		value::{SpringState, advance},
	},
	theme::ActiveTheme,
	tokens::{control, over, with_alpha},
};

use super::scale::unscale_about;
use super::*;

/// Whether decorative motion should hold still: the system's Reduce Motion setting or the
/// theme flag. Every animation that only dresses a change reads it before asking for frames.
pub fn reduce_motion(cx: &App) -> bool {
	cx.reduce_motion() || cx.theme().reduce_motion
}

/// How far past its laid-out bounds the pointer still counts as over a control: a hovered control
/// scales up a little, and its edge must not flicker.
///
/// Limitation: the bounds are recorded when the control paints. A view that is cached and then
/// scrolled moves the control without rendering it, so the bounds (and this check) lag until
/// the view renders again; hover edges themselves still come from the live hit test.
const HOVER_SLOP: Pixels = px(4.);

/// How long a Space or Return activation holds the press feedback.
pub(super) const KEY_PRESS_PULSE: Duration = Duration::from_millis(120);

impl ControlState {
	pub(super) fn new() -> Self {
		Self {
			hovered: false,
			pressed: false,
			key_pulse: None,
			bounds: None,
			pointer: None,
			scale: SpringState::at_rest(1.),
			scale_moving: false,
			scale_at: Instant::now(),
			light: Animated::new(0., Curve::Hover.transition(false)),
		}
	}

	/// Drops a hover or press the pointer's exit event never cleared. Returns whether anything
	/// changed.
	pub(crate) fn clear_hover(&mut self) -> bool {
		let changed = self.hovered || self.pressed;
		self.hovered = false;
		self.pressed = false;
		changed
	}

	/// Starts the press feedback of a keyboard activation.
	pub(super) fn pulse(&mut self) {
		self.key_pulse = Some(Instant::now());
	}

	/// The state of the control `id`, kept for as long as the control renders.
	pub fn of(id: &ElementId, window: &mut Window, cx: &mut App) -> Entity<Self> {
		window.use_keyed_state((id.clone(), "calcite-control"), cx, |_, _| Self::new())
	}

	/// Advances the springs toward the current targets and returns this frame's values.
	pub fn frame(
		state: &Entity<Self>,
		feedback: Feedback,
		enabled: bool,
		reduce_motion: bool,
		window: &mut Window,
		cx: &mut App,
	) -> ControlFrame {
		let now = Instant::now();
		let reduce_motion = reduce_motion || self::reduce_motion(cx);
		let (frame, is_moving) = state.update(cx, |state, _| {
			let (hover_scale, press_scale) = feedback.scales();
			// A pointer that left without an exit event (a fast flick, the window losing the
			// pointer) must not leave the hover lit or the press held: check the live pointer
			// against the bounds. A control that appeared under a still pointer never got a hover
			// edge, so this runs for a press too.
			if (state.hovered || state.pressed)
				&& (!window.is_window_active()
					|| state
						.bounds
						.is_some_and(|bounds| !bounds.dilate(HOVER_SLOP).contains(&window.mouse_position())))
			{
				state.clear_hover();
			}
			let pulsing = state
				.key_pulse
				.is_some_and(|start| now.saturating_duration_since(start) < KEY_PRESS_PULSE);
			if !pulsing {
				state.key_pulse = None;
			}
			let hovered = enabled && state.hovered;
			let pressed = enabled && (state.pressed || pulsing);
			// Reduce Motion keeps only the opacity dip of a press.
			let scale_target = if reduce_motion {
				1.
			} else if pressed {
				press_scale
			} else if hovered {
				hover_scale
			} else {
				1.
			};
			let dt = now.saturating_duration_since(state.scale_at).as_secs_f32();
			state.scale_at = now;
			if reduce_motion {
				// No spring under Reduce Motion: a scale left over from before snaps to rest.
				state.scale = SpringState::at_rest(scale_target);
			}
			// A spring that was idle has no meaningful elapsed time: its first frame is the retarget.
			let (next, scale_active) = advance(
				state.scale,
				scale_target,
				scale_spring(pressed),
				if state.scale_moving { dt } else { 0. },
			);
			state.scale = next;
			state.scale_moving = scale_active;
			let light_target = if hovered { 1. } else { 0. };
			if *state.light.value() != light_target {
				state
					.light
					.set(light_target, Curve::Hover.transition(reduce_motion), now);
			}
			let scale = next.value;
			let light = state.light.sample(now);
			// Reduce Motion pins the light to the center instead of tracking the pointer.
			// The control paints under a scale about its center, so the window-space pointer
			// goes through the inverse to land under the cursor.
			let pointer = match (reduce_motion, state.bounds) {
				(true, Some(bounds)) => Some(bounds.center()),
				(false, Some(bounds)) => state
					.pointer
					.map(|pointer| unscale_about(pointer, bounds.center(), scale)),
				(_, None) => state.pointer,
			};
			let pointer_radius = state
				.bounds
				.map_or(DEFAULT_POINTER_RADIUS, |bounds| pointer_radius_for(bounds.size));
			let frame = ControlFrame {
				hovered,
				pressed,
				scale,
				press: feedback.press_depth(scale),
				light: light.value.clamp(0., 1.),
				pointer,
				pointer_radius,
			};
			let decorative = (scale_active || light.is_active) && !reduce_motion;
			// The pulse must end with a frame even under Reduce Motion: the opacity dip is kept.
			(frame, decorative || pulsing)
		});
		if is_moving {
			window.request_animation_frame();
		}
		frame
	}
}

/// The state a control keeps across frames.
pub struct ControlState {
	pub(super) hovered: bool,
	pub(super) pressed: bool,
	/// When a keyboard activation last started; the press feedback lasts [`KEY_PRESS_PULSE`].
	key_pulse: Option<Instant>,
	pub(super) bounds: Option<Bounds<Pixels>>,
	pub(super) pointer: Option<Point<Pixels>>,
	/// Scale and its velocity: retargeting (press during a hover lift, release mid-press) keeps
	/// the velocity, so the path bends instead of restarting.
	scale: SpringState,
	scale_moving: bool,
	scale_at: Instant,
	light: Animated<f32>,
}

/// The fill of a row: the accent selection fill at `selected` (0..=1) with the shared hover
/// overlay on top, which gives way to the selection. The resting fill is transparent white,
/// not clear black, so a fill transition never passes through dark colors.
pub(crate) fn row_fill(ink: Rgba, accent: Rgba, selected: f32, light: f32) -> Rgba {
	let selected = selected.clamp(0., 1.);
	let selection = with_alpha(accent, control::SELECTED_ROW_FILL_OPACITY * selected);
	let hover = with_alpha(
		with_alpha(ink, 1.),
		control::HOVER_OPACITY * light.clamp(0., 1.) * (1. - selected),
	);
	let fill = over(hover, selection);
	if fill.alpha <= f32::EPSILON {
		with_alpha(ink, 0.)
	} else {
		fill
	}
}
