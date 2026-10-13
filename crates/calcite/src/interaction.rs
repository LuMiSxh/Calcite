// SPDX-License-Identifier: MPL-2.0

//! Per-control interaction state: hover, press, bounds, pointer, and the springs that turn
//! them into scale and light. Port of `ActionPressStyle` and `PointerSheen`.
//!
//! Scale never changes layout: components draw their surface on an absolute layer and move
//! its insets, like SwiftUI's `scaleEffect`. GPUI re-renders while a spring moves and stops
//! when it settles.

use std::time::Instant;

use gpui::{
	Animated, App, Bounds, ElementId, Entity, MouseButton, ParentElement, Pixels, Point, Size,
	StatefulInteractiveElement, Styled, Window, canvas, div, px,
};

use crate::motion::Curve;

/// Press and hover feedback (`ActionPressStyle.Intensity`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Feedback {
	/// Compact buttons and pills: 0.95 pressed, 1.025 hovered.
	Standard,
	/// Large tiles and cards: 0.97 pressed, 1.01 hovered.
	Subtle,
	/// List rows: 0.97 pressed, no hover lift.
	PressOnly,
}

impl Feedback {
	fn scales(self) -> (f32, f32) {
		match self {
			Self::Standard => (1.025, 0.95),
			Self::Subtle => (1.01, 0.97),
			Self::PressOnly => (1., 0.97),
		}
	}
}

/// What a control looks like in this frame.
#[derive(Clone, Copy, Debug)]
pub struct ControlFrame {
	pub hovered: bool,
	pub pressed: bool,
	/// Surface scale around the center.
	pub scale: f32,
	/// Pointer light and hover strength in 0..=1, faded with the hover curve.
	pub light: f32,
	/// Where the pointer light sits, in window coordinates.
	pub pointer: Option<Point<Pixels>>,
	/// The control's size from the previous frame.
	pub size: Option<Size<Pixels>>,
}

impl ControlFrame {
	/// Absolute insets that scale a full-size surface layer around its center.
	pub fn surface_insets(&self) -> (Pixels, Pixels) {
		let Some(size) = self.size else {
			return (px(0.), px(0.));
		};
		let grow = 1. - self.scale;
		(
			px(f32::from(size.width) * grow / 2.),
			px(f32::from(size.height) * grow / 2.),
		)
	}
}

/// The state a control keeps across frames.
pub struct ControlState {
	hovered: bool,
	pressed: bool,
	bounds: Option<Bounds<Pixels>>,
	pointer: Option<Point<Pixels>>,
	scale: Animated<f32>,
	light: Animated<f32>,
}

impl ControlState {
	fn new() -> Self {
		Self {
			hovered: false,
			pressed: false,
			bounds: None,
			pointer: None,
			scale: Animated::new(1., Curve::Release.transition(false)),
			light: Animated::new(0., Curve::Hover.transition(false)),
		}
	}

	/// The state of the control `id`, kept for as long as the control renders.
	pub fn of(id: &ElementId, window: &mut Window, cx: &mut App) -> Entity<Self> {
		window.use_keyed_state((id.clone(), "vitro-control"), cx, |_, _| Self::new())
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
		let (frame, is_moving) = state.update(cx, |state, _| {
			let (hover_scale, press_scale) = feedback.scales();
			let hovered = enabled && state.hovered;
			let pressed = enabled && state.pressed;
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
			let scale_curve = if pressed { Curve::Press } else { Curve::Release };
			if *state.scale.value() != scale_target {
				state
					.scale
					.set(scale_target, scale_curve.transition(reduce_motion), now);
			}
			let light_target = if hovered { 1. } else { 0. };
			if *state.light.value() != light_target {
				state
					.light
					.set(light_target, Curve::Hover.transition(reduce_motion), now);
			}
			let scale = state.scale.sample(now);
			let light = state.light.sample(now);
			// Reduce Motion pins the light to the center instead of tracking the pointer.
			let pointer = match (reduce_motion, state.bounds) {
				(true, Some(bounds)) => Some(bounds.center()),
				_ => state.pointer,
			};
			let frame = ControlFrame {
				hovered,
				pressed,
				scale: scale.value,
				light: light.value.clamp(0., 1.),
				pointer,
				size: state.bounds.map(|bounds| bounds.size),
			};
			(frame, scale.is_active || light.is_active)
		});
		if is_moving {
			window.request_animation_frame();
		}
		frame
	}
}

/// Wires a control's hover, press, pointer, and bounds into its [`ControlState`].
pub trait ControlInteraction: StatefulInteractiveElement + ParentElement + Sized {
	fn control_state(self, state: &Entity<ControlState>, tracks_pointer: bool) -> Self {
		let hover = state.clone();
		let press = state.clone();
		let release = state.clone();
		let pointer = state.clone();
		let probe = state.clone();
		let this = self
			.on_hover(move |hovered, _, cx| {
				hover.update(cx, |state, cx| {
					state.hovered = *hovered;
					if !*hovered {
						state.pressed = false;
					}
					cx.notify();
				});
			})
			.on_mouse_down(MouseButton::Left, move |_, _, cx| {
				press.update(cx, |state, cx| {
					state.pressed = true;
					cx.notify();
				});
			})
			.on_mouse_up(MouseButton::Left, move |_, _, cx| {
				release.update(cx, |state, cx| {
					state.pressed = false;
					cx.notify();
				});
			});
		let this = if tracks_pointer {
			this.on_mouse_move(move |event, _, cx| {
				pointer.update(cx, |state, cx| {
					state.pointer = Some(event.position);
					if state.hovered {
						cx.notify();
					}
				});
			})
		} else {
			this
		};
		// A zero-cost probe records the laid-out bounds for scale and the centered light.
		this.child(
			canvas(
				move |bounds, _, cx| probe.update(cx, |state, _| state.bounds = Some(bounds)),
				|_, _, _, _| {},
			)
			.absolute()
			.size_full(),
		)
	}
}

impl<E: StatefulInteractiveElement + ParentElement + Sized> ControlInteraction for E {}

/// A spring-smoothed value that follows `target`, keyed by `id` across frames.
pub fn spring_value(
	id: impl Into<ElementId>,
	target: f32,
	curve: Curve,
	reduce_motion: bool,
	window: &mut Window,
	cx: &mut App,
) -> f32 {
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
	if sample.is_active {
		window.request_animation_frame();
	}
	sample.value
}

/// An empty absolute layer that fills its parent; components paint surfaces on it.
pub fn layer(insets: (Pixels, Pixels)) -> gpui::Div {
	let (horizontal, vertical) = insets;
	div()
		.absolute()
		.top(vertical)
		.bottom(vertical)
		.left(horizontal)
		.right(horizontal)
}
