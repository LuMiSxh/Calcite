// SPDX-License-Identifier: MPL-2.0

//! Per-control interaction state: hover, press, bounds, pointer, and the springs that turn
//! them into scale and light. Port of `ActionPressStyle` and `PointerSheen`.
//!
//! Keyboard parity lives here too: [`KeyboardFocus`] (`KeyboardFocusIndicator`) makes a
//! control a tab stop with the accent focus ring, and [`init_keyboard`] routes Tab, Return
//! (the default action), and Escape (the topmost dismissible surface).
//!
//! Scale never changes layout: components draw their surface on an absolute layer and move
//! its insets, like SwiftUI's `scaleEffect`. GPUI re-renders while a spring moves and stops
//! when it settles.

use std::{rc::Rc, time::Instant};

use gpui::{
	Animated, AnyWindowHandle, App, Bounds, ElementId, Entity, FocusHandle, Global, InteractiveElement,
	KeyBinding, KeyDownEvent, MouseButton, ParentElement, Pixels, Point, Size, StatefulInteractiveElement,
	Styled, WeakEntity, Window, actions, canvas, div, px,
};

use crate::{motion::Curve, theme::Theme, tokens};

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
	let state = window.use_keyed_state(id, cx, |_, _| {
		Animated::new(0., Curve::Present.transition(reduce_motion))
	});
	let target = if shown { 1. } else { 0. };
	let curve = if shown { Curve::Present } else { Curve::Dismiss };
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
	(shown || sample.is_active).then_some(sample.value)
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

/// The outline a focus ring follows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FocusShape {
	Capsule,
	/// A rounded rectangle with the control's corner radius.
	Rounded(Pixels),
}

/// How far the ring sits outside the control (`padding(-3)` in the launcher).
const FOCUS_OUTSET: f32 = 3.;

/// The focus handle of the control `id`, a tab stop kept for as long as the control renders.
pub fn focus_handle(id: &ElementId, window: &mut Window, cx: &mut App) -> FocusHandle {
	// The tab order reads the handle's own flag: `.tab_index` on an element that tracks an
	// explicit handle does not reach it.
	window
		.use_keyed_state((id.clone(), "calcite-focus"), cx, |_, cx| {
			cx.focus_handle().tab_stop(true)
		})
		.read(cx)
		.clone()
}

/// Whether `handle` shows its ring: focused by the keyboard in the active window.
pub fn focus_visible(handle: &FocusHandle, window: &Window) -> bool {
	handle.is_focused(window) && window.last_input_was_keyboard() && window.is_window_active()
}

/// The 2 pt accent ring around a focused control. Its parent must be `relative`.
pub fn focus_ring(shape: FocusShape, theme: &Theme) -> gpui::Div {
	let outset = px(-FOCUS_OUTSET);
	let ring = div()
		.absolute()
		.top(outset)
		.bottom(outset)
		.left(outset)
		.right(outset)
		.border_2()
		.border_color(tokens::contrast_tint(theme.accent));
	match shape {
		FocusShape::Capsule => ring.rounded_full(),
		FocusShape::Rounded(corner) => ring.rounded(corner + px(FOCUS_OUTSET)),
	}
}

/// Port of `KeyboardFocusIndicator`: a tab stop that shows the accent ring while focused by
/// the keyboard. Space and Return then click it through GPUI's keyboard click.
pub trait KeyboardFocus: InteractiveElement + ParentElement + Sized {
	fn keyboard_focus(self, handle: &FocusHandle, shape: FocusShape, theme: &Theme, window: &Window) -> Self {
		let ring = focus_visible(handle, window).then(|| focus_ring(shape, theme));
		self.track_focus(handle)
			// Like macOS buttons, a click does not take focus, so Return keeps reaching the
			// default action and a focused text field keeps its focus.
			.capture_any_mouse_down(|_, window, _| window.prevent_default())
			.children(ring)
	}
}

impl<E: InteractiveElement + ParentElement + Sized> KeyboardFocus for E {}

/// What a key target answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KeyRole {
	/// Return when no control that consumes Return has focus (`.defaultAction`).
	DefaultAction,
	/// Escape (`.cancelAction`): closes the topmost menu or modal.
	Dismiss,
	/// A focusable surface that passes Return on to the default action, like a modal
	/// container or a single-line text field.
	PassReturn,
}

pub(crate) type KeyHandler = Rc<dyn Fn(&mut Window, &mut App) + 'static>;

/// One registered target, kept as keyed state so it lives only while its control renders.
struct KeySlot {
	handler: Option<KeyHandler>,
	focus: Option<FocusHandle>,
}

#[derive(Default)]
struct KeyTargets(Vec<(AnyWindowHandle, KeyRole, WeakEntity<KeySlot>)>);

impl Global for KeyTargets {}

struct KeyboardInstalled;

impl Global for KeyboardInstalled {}

/// Registers the key target of the control `id` for this frame. Targets rendered later sit
/// on top, so an open menu closes before the modal under it.
pub(crate) fn register_key_target(
	id: &ElementId,
	role: KeyRole,
	handler: Option<KeyHandler>,
	focus: Option<FocusHandle>,
	window: &mut Window,
	cx: &mut App,
) {
	let key = match role {
		KeyRole::DefaultAction => "calcite-default-action",
		KeyRole::Dismiss => "calcite-dismiss",
		KeyRole::PassReturn => "calcite-pass-return",
	};
	let slot = window.use_keyed_state((id.clone(), key), cx, |_, _| KeySlot {
		handler: None,
		focus: None,
	});
	slot.update(cx, |slot, _| {
		slot.handler = handler;
		slot.focus = focus;
	});
	let weak = slot.downgrade();
	let window_handle = window.window_handle();
	let targets = &mut cx.default_global::<KeyTargets>().0;
	targets.retain(|(_, _, slot)| slot.entity_id() != weak.entity_id() && slot.upgrade().is_some());
	targets.push((window_handle, role, weak));
}

fn live_slots(role: KeyRole, window: &Window, cx: &App) -> Vec<Entity<KeySlot>> {
	let window_handle = window.window_handle();
	cx.try_global::<KeyTargets>()
		.map(|targets| {
			targets
				.0
				.iter()
				.filter(|(handle, slot_role, _)| *handle == window_handle && *slot_role == role)
				.filter_map(|(_, _, slot)| slot.upgrade())
				.collect()
		})
		.unwrap_or_default()
}

/// The handler Return or Escape runs in `window`, if any.
fn key_handler(key: &str, window: &Window, cx: &App) -> Option<KeyHandler> {
	let role = match key {
		"escape" => KeyRole::Dismiss,
		"enter" => {
			// A focused control that is not a passive surface consumes Return itself.
			if let Some(focused) = window.focused(cx) {
				let passes = live_slots(KeyRole::PassReturn, window, cx)
					.into_iter()
					.chain(live_slots(KeyRole::Dismiss, window, cx))
					.any(|slot| slot.read(cx).focus.as_ref() == Some(&focused));
				if !passes {
					return None;
				}
			}
			KeyRole::DefaultAction
		}
		_ => return None,
	};
	live_slots(role, window, cx)
		.into_iter()
		.rev()
		.find_map(|slot| slot.read(cx).handler.clone())
}

actions!(
	calcite,
	[
		/// Moves keyboard focus to the next control.
		FocusNext,
		/// Moves keyboard focus to the previous control.
		FocusPrevious,
	]
);

/// Binds Tab and Shift-Tab to move focus, and routes Return to the default action and
/// Escape to the topmost menu or modal. Call it once; later calls do nothing.
pub fn init_keyboard(cx: &mut App) {
	if cx.has_global::<KeyboardInstalled>() {
		return;
	}
	cx.set_global(KeyboardInstalled);
	// Unscoped bindings lose to a deeper key context, so a text editor keeps its Tab.
	cx.bind_keys([
		KeyBinding::new("tab", FocusNext, None),
		KeyBinding::new("shift-tab", FocusPrevious, None),
	]);
	// Global action listeners get no window; move focus once the dispatch returns it.
	cx.on_action(|_: &FocusNext, cx| {
		if let Some(window) = cx.active_window() {
			cx.defer(move |cx| {
				window.update(cx, |_, window, cx| window.focus_next(cx)).ok();
			});
		}
	});
	cx.on_action(|_: &FocusPrevious, cx| {
		if let Some(window) = cx.active_window() {
			cx.defer(move |cx| {
				window.update(cx, |_, window, cx| window.focus_prev(cx)).ok();
			});
		}
	});
	// An interceptor sees the window and runs before key bindings, so Return and Escape
	// work even when no control has focus.
	cx.intercept_keystrokes(|event, window, cx| {
		let stroke = &event.keystroke;
		if stroke.modifiers.modified() {
			return;
		}
		if let Some(handler) = key_handler(&stroke.key, window, cx) {
			cx.stop_propagation();
			handler(window, cx);
		}
	})
	.detach();
}

/// The step that arrow keys move a value: `step`, or 5 % of the range.
pub(crate) fn key_step(range: &std::ops::RangeInclusive<f32>, step: Option<f32>) -> f32 {
	step.filter(|step| *step > 0.)
		.unwrap_or_else(|| (range.end() - range.start()).abs() * 0.05)
}

/// The direction of an unmodified arrow key: -1 for left and down, 1 for right and up.
pub(crate) fn arrow_direction(event: &KeyDownEvent) -> Option<i32> {
	if event.keystroke.modifiers.modified() {
		return None;
	}
	match event.keystroke.key.as_str() {
		"left" | "down" => Some(-1),
		"right" | "up" => Some(1),
		_ => None,
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn key_step_defaults_to_five_percent_of_the_range() {
		assert!((key_step(&(0.0..=200.0), None) - 10.).abs() < 1e-6);
		assert_eq!(key_step(&(0.0..=1.0), Some(0.25)), 0.25);
	}
}
