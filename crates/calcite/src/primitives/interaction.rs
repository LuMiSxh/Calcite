// SPDX-License-Identifier: MPL-2.0

//! Per-control interaction state: hover, press, bounds, pointer, and the springs that turn
//! them into scale and light. Port of `ActionPressStyle` and `PointerSheen`.
//!
//! Keyboard parity lives here too: [`KeyboardFocus`] (`KeyboardFocusIndicator`) makes a
//! control a tab stop with the accent focus ring, and [`init_keyboard`] routes Tab, Return
//! (the default action), and Escape (the topmost dismissible surface).
//!
//! Scale never changes layout: [`ScaleExt::scaled`] paints the whole control (surface,
//! label, icons) through a paint transform, like SwiftUI's `scaleEffect`. GPUI re-renders
//! while a spring moves and stops when it settles.

use std::{
	rc::Rc,
	time::{Duration, Instant},
};

use gpui::{
	Animated, AnyElement, AnyWindowHandle, App, Bounds, BoxShadow, Div, Element, ElementId, Entity, Filter,
	FocusHandle, Global, GlobalElementId, InspectorElementId, InteractiveElement, IntoElement, KeyBinding,
	KeyDownEvent, LayoutId, MouseButton, ParentElement, Pixels, Point, Rgba, Stateful,
	StatefulInteractiveElement, Styled, WeakEntity, Window, actions, canvas, div, point, px,
};

use crate::{
	glass::{DEFAULT_POINTER_RADIUS, flat_surface, pointer_radius_for},
	motion::{
		self, Curve, Spring,
		tokens::{JELLY, PRESS},
		value::{SpringState, advance},
	},
	theme::{ActiveTheme, Theme},
	tokens::{self, control, over, with_alpha},
};

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

	/// How deep a press is at `scale`: 0 at rest or hovered, 1 fully pressed (it may pass 1 while
	/// the release spring is pulled below the pressed scale).
	pub fn press_depth(self, scale: f32) -> f32 {
		let (_, pressed) = self.scales();
		((1. - scale) / (1. - pressed)).clamp(0., 1.)
	}
}

/// What a control looks like in this frame.
#[derive(Clone, Copy, Debug)]
pub struct ControlFrame {
	pub hovered: bool,
	pub pressed: bool,
	/// Scale of the whole control around its center; apply it with [`ScaleExt::scaled`].
	pub scale: f32,
	/// How deep the press is in 0..=1, following the scale spring; shrinks the shadow
	/// ([`ControlFrame::shadows`]).
	pub press: f32,
	/// Pointer light and hover strength in 0..=1, faded with the hover curve.
	pub light: f32,
	/// Where the pointer light sits, in the control's unscaled paint coordinates: the pointer
	/// is mapped through the inverse of the scale so the light lands under the cursor.
	pub pointer: Option<Point<Pixels>>,
	/// Radius of the pointer light: [`pointer_radius_for`] of the control's laid-out size, or
	/// the default until the first layout.
	pub pointer_radius: Pixels,
}

impl ControlFrame {
	/// The control's opacity this frame: dipped while pressed (`ActionPressStyle`).
	pub fn press_opacity(&self) -> f32 {
		if self.pressed {
			control::PRESSED_OPACITY
		} else {
			1.
		}
	}

	/// `shadows` for this frame: a pressed control sits closer to its backdrop, so the blur shrinks
	/// to 0.6x and the vertical offset to 0.5x at full press (see [`pressed_shadows`]).
	pub fn shadows(&self, shadows: Vec<BoxShadow>) -> Vec<BoxShadow> {
		pressed_shadows(shadows, self.press)
	}

	/// The hover overlay of `tint` (white on neutral surfaces, the accent on tinted ones): the
	/// shared hover strength faded with the hover curve.
	pub fn hover_fill(&self, tint: Rgba) -> Rgba {
		with_alpha(tint, control::HOVER_OPACITY * self.light)
	}
}

/// Blur radius multiplier of a fully pressed shadow.
const PRESSED_SHADOW_BLUR: f32 = 0.6;
/// Vertical offset multiplier of a fully pressed shadow.
const PRESSED_SHADOW_OFFSET: f32 = 0.5;

/// `shadows` pulled in by a press of depth `press` (0..=1): blur toward 0.6x, y offset toward 0.5x.
pub fn pressed_shadows(shadows: Vec<BoxShadow>, press: f32) -> Vec<BoxShadow> {
	let press = if press.is_nan() { 0. } else { press.clamp(0., 1.) };
	if press == 0. {
		return shadows;
	}
	let blur = 1. - (1. - PRESSED_SHADOW_BLUR) * press;
	let offset = 1. - (1. - PRESSED_SHADOW_OFFSET) * press;
	shadows
		.into_iter()
		.map(|shadow| BoxShadow {
			offset: point(shadow.offset.x, shadow.offset.y * offset),
			blur_radius: shadow.blur_radius * blur,
			..shadow
		})
		.collect()
}

/// The spring behind a control's scale: [`PRESS`] while held down, [`JELLY`] on release (and when
/// the hover lift ends), so letting go springs back with a visible wobble.
pub fn scale_spring(pressed: bool) -> Spring {
	if pressed { PRESS } else { JELLY }
}

/// Where a [`ScaleEffect`] keeps its fixed point.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScaleAnchor {
	#[default]
	Center,
	/// The middle of the bottom edge, where a HUD pill rises from the control bar.
	BottomCenter,
}

impl ScaleAnchor {
	fn origin(self, bounds: Bounds<Pixels>) -> Point<Pixels> {
		match self {
			Self::Center => bounds.center(),
			Self::BottomCenter => point(bounds.center().x, bounds.bottom()),
		}
	}
}

/// Whether a scale is close enough to 1 to skip the paint transform.
fn is_unscaled(scale: f32) -> bool {
	(scale - 1.).abs() < 1e-4
}

/// Where `p` lands when scaled by `scale` about `origin`.
#[cfg(test)]
fn scale_about(p: Point<Pixels>, origin: Point<Pixels>, scale: f32) -> Point<Pixels> {
	origin + (p - origin) * scale
}

/// The point that lands on `p` once scaled by `scale` about `origin`.
fn unscale_about(p: Point<Pixels>, origin: Point<Pixels>, scale: f32) -> Point<Pixels> {
	if scale.abs() < 1e-6 {
		return p;
	}
	origin + (p - origin) / scale
}

/// Paints its child scaled about an anchor without touching layout or hit-testing.
pub struct ScaleEffect {
	scale: f32,
	anchor: ScaleAnchor,
	child: AnyElement,
}

/// Scales everything `child` paints by `scale` about its center (SwiftUI `scaleEffect`).
pub fn scale_effect(scale: f32, child: impl IntoElement) -> ScaleEffect {
	ScaleEffect {
		scale,
		anchor: ScaleAnchor::Center,
		child: child.into_any_element(),
	}
}

impl ScaleEffect {
	pub fn anchor(mut self, anchor: ScaleAnchor) -> Self {
		self.anchor = anchor;
		self
	}
}

/// `element.scaled(frame.scale)`: the whole control scales, layout and hitboxes do not.
pub trait ScaleExt: IntoElement + Sized {
	fn scaled(self, scale: f32) -> ScaleEffect {
		scale_effect(scale, self)
	}
}

impl<E: IntoElement> ScaleExt for E {}

impl IntoElement for ScaleEffect {
	type Element = Self;

	fn into_element(self) -> Self::Element {
		self
	}
}

impl Element for ScaleEffect {
	type RequestLayoutState = ();
	type PrepaintState = ();

	fn id(&self) -> Option<ElementId> {
		None
	}

	fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
		None
	}

	fn request_layout(
		&mut self,
		_id: Option<&GlobalElementId>,
		_inspector_id: Option<&InspectorElementId>,
		window: &mut Window,
		cx: &mut App,
	) -> (LayoutId, Self::RequestLayoutState) {
		(self.child.request_layout(window, cx), ())
	}

	fn prepaint(
		&mut self,
		_id: Option<&GlobalElementId>,
		_inspector_id: Option<&InspectorElementId>,
		_bounds: Bounds<Pixels>,
		_request_layout: &mut Self::RequestLayoutState,
		window: &mut Window,
		cx: &mut App,
	) -> Self::PrepaintState {
		self.child.prepaint(window, cx);
	}

	fn paint(
		&mut self,
		_id: Option<&GlobalElementId>,
		_inspector_id: Option<&InspectorElementId>,
		bounds: Bounds<Pixels>,
		_request_layout: &mut Self::RequestLayoutState,
		_prepaint: &mut Self::PrepaintState,
		window: &mut Window,
		cx: &mut App,
	) {
		if is_unscaled(self.scale) {
			self.child.paint(window, cx);
		} else {
			let origin = self.anchor.origin(bounds);
			let child = &mut self.child;
			window.with_paint_transform(self.scale, origin, |window| child.paint(window, cx));
		}
	}
}

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
const KEY_PRESS_PULSE: Duration = Duration::from_millis(120);

/// The state a control keeps across frames.
pub struct ControlState {
	hovered: bool,
	pressed: bool,
	/// When a keyboard activation last started; the press feedback lasts [`KEY_PRESS_PULSE`].
	key_pulse: Option<Instant>,
	bounds: Option<Bounds<Pixels>>,
	pointer: Option<Point<Pixels>>,
	/// Scale and its velocity: retargeting (press during a hover lift, release mid-press) keeps
	/// the velocity, so the path bends instead of restarting.
	scale: SpringState,
	scale_moving: bool,
	scale_at: Instant,
	light: Animated<f32>,
}

impl ControlState {
	fn new() -> Self {
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
	fn pulse(&mut self) {
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

/// What every clickable row shares: its hover light, selection fill, and
/// keyboard focus ring. Build it per frame, read [`ClickableRow::fill`], then call
/// [`ClickableRow::wire`] on the row's element.
pub(crate) struct ClickableRow {
	state: Entity<ControlState>,
	focus: FocusHandle,
	pub frame: ControlFrame,
}

impl ClickableRow {
	pub(crate) fn new(id: &ElementId, theme: &Theme, window: &mut Window, cx: &mut App) -> Self {
		let state = ControlState::of(id, window, cx);
		let focus = focus_handle(id, window, cx);
		let frame = ControlState::frame(&state, Feedback::PressOnly, true, theme.reduce_motion, window, cx);
		Self { state, focus, frame }
	}

	/// Whether the row is a tab stop (the default). A roving group (a rail, a radio group) flags
	/// only its current entry, so Tab visits the group once and the arrow keys move inside it.
	pub(crate) fn tab_stop(mut self, tab_stop: bool) -> Self {
		self.focus = self.focus.tab_stop(tab_stop);
		self
	}

	/// The row's focus handle, for the group that routes the arrow keys.
	pub(crate) fn focus(&self) -> &FocusHandle {
		&self.focus
	}

	/// The row's background for a selection amount of `selected` (0..=1).
	pub(crate) fn fill(&self, theme: &Theme, selected: f32) -> Rgba {
		row_fill(theme.palette.ink, theme.accent, selected, self.frame.light)
	}

	/// Makes `row` respond: hover tracking and the focus ring of `shape`.
	pub(crate) fn wire(
		&self,
		row: Stateful<Div>,
		shape: FocusShape,
		theme: &Theme,
		window: &Window,
	) -> Stateful<Div> {
		row.control_state(&self.state, false)
			.keyboard_focus(&self.focus, shape, theme, window)
	}
}

/// Whether a control carries the pointer light. `setting` is the component's `.glow(bool)`
/// override and `None` follows [`Style::pointer_glow`](crate::style::Style::pointer_glow).
/// Reduce Transparency and compact controls never glow.
pub fn glow_enabled(theme: &Theme, setting: Option<bool>, compact: bool) -> bool {
	setting.unwrap_or(theme.style.pointer_glow) && !compact && !theme.reduce_transparency()
}

/// The pointer light as its own layer over a control's surface: transparent glass that only
/// adds the light. It fades with `frame.light`, so it rises on hover and falls away on
/// leave. `None` while the pointer is outside the control. The caller rounds it like the
/// surface and tracks the pointer with `control_state(&state, true)`.
pub fn glow_layer(frame: &ControlFrame, tint: Rgba) -> Option<gpui::Div> {
	let pointer = frame.pointer?;
	if frame.light <= 0. {
		return None;
	}
	Some(layer().backdrop_filter(vec![Filter::Glass(flat_surface(
		tokens::CLEAR,
		Some(pointer),
		with_alpha(tint, frame.light),
		frame.pointer_radius,
	))]))
}

/// Wires a control's hover, press, pointer, and bounds into its [`ControlState`].
pub trait ControlInteraction: StatefulInteractiveElement + ParentElement + Sized {
	/// When the pointer leaves this whole element, drops the hover and press of every state in
	/// `states`. A fast exit can skip a child's own hover-out event, which would leave it lit.
	fn clear_hover_on_leave(self, states: Vec<Entity<ControlState>>) -> Self {
		self.on_hover(move |hovered, _, cx| {
			if !*hovered {
				for state in &states {
					state.update(cx, |state, cx| {
						if state.clear_hover() {
							cx.notify();
						}
					});
				}
			}
		})
	}

	fn control_state(self, state: &Entity<ControlState>, tracks_pointer: bool) -> Self {
		let hover = state.clone();
		let press = state.clone();
		let release = state.clone();
		let release_out = state.clone();
		let pointer = state.clone();
		let probe = state.clone();
		let key = state.clone();
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
			})
			// Released outside after a drag off a control that never saw a hover edge.
			.on_mouse_up_out(MouseButton::Left, move |_, _, cx| {
				release_out.update(cx, |state, cx| {
					if state.pressed {
						state.pressed = false;
						cx.notify();
					}
				});
			})
			// Space and Return activate a focused control without the mouse: show the press.
			.on_key_down(move |event, window, cx| {
				let stroke = &event.keystroke;
				if event.is_held
					|| stroke.modifiers.modified()
					|| !matches!(stroke.key.as_str(), "space" | "enter")
					|| text_input_focused(window)
				{
					return;
				}
				key.update(cx, |state, cx| {
					state.pulse();
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
	let reduce_motion = reduce_motion || self::reduce_motion(cx);
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
	let reduce_motion = reduce_motion || self::reduce_motion(cx);
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

/// The outline a focus ring follows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FocusShape {
	Capsule,
	/// A rounded rectangle with the control's corner radius.
	Rounded(Pixels),
}

/// The ring's stroke width.
const FOCUS_RING_WIDTH: f32 = 2.;

/// The gap between the control's edge and the ring.
const FOCUS_RING_OFFSET: f32 = 2.;

/// How far the ring's box sits outside the control.
const FOCUS_OUTSET: f32 = FOCUS_RING_WIDTH + FOCUS_RING_OFFSET;

/// The opacity of the focus ring: soft enough to read as a halo, strong enough to find.
pub const FOCUS_RING_ALPHA: f32 = 0.6;

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

/// The soft accent ring around a focused control: 2 pt wide, 2 pt off the edge, at
/// [`FOCUS_RING_ALPHA`]. Its parent must be `relative`.
pub fn focus_ring(shape: FocusShape, theme: &Theme) -> gpui::Div {
	let outset = px(-FOCUS_OUTSET);
	let ring = div()
		.absolute()
		.top(outset)
		.bottom(outset)
		.left(outset)
		.right(outset)
		.border_2()
		.border_color(with_alpha(tokens::contrast_tint(theme.accent), FOCUS_RING_ALPHA));
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
	/// A modal's focus scope: Tab and Shift-Tab cycle inside it.
	TabScope,
}

impl KeyRole {
	/// The keyed-state name of a slot with this role.
	fn slot_key(self) -> &'static str {
		match self {
			KeyRole::DefaultAction => "calcite-default-action",
			KeyRole::Dismiss => "calcite-dismiss",
			KeyRole::PassReturn => "calcite-pass-return",
			KeyRole::TabScope => "calcite-tab-scope",
		}
	}
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
	let slot = window.use_keyed_state((id.clone(), role.slot_key()), cx, |_, _| KeySlot {
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

/// Removes the key target of the control `id` for `role`, so a surface that is leaving (or gone)
/// stops answering keys.
pub(crate) fn unregister_key_target(id: &ElementId, role: KeyRole, window: &mut Window, cx: &mut App) {
	let slot = window.use_keyed_state((id.clone(), role.slot_key()), cx, |_, _| KeySlot {
		handler: None,
		focus: None,
	});
	let gone = slot.entity_id();
	if cx.has_global::<KeyTargets>() {
		cx.global_mut::<KeyTargets>()
			.0
			.retain(|(_, _, slot)| slot.entity_id() != gone);
	}
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

/// How many tab stops [`focus_within`] steps through before it gives up.
const TAB_SCAN_LIMIT: usize = 256;

/// Moves focus one tab stop forward or back inside `scope`, wrapping at its ends: Tab from the
/// last control lands on the first rather than on the page behind a modal.
pub(crate) fn focus_within(scope: &FocusHandle, forward: bool, window: &mut Window, cx: &mut App) {
	for _ in 0..TAB_SCAN_LIMIT {
		if forward {
			window.focus_next(cx);
		} else {
			window.focus_prev(cx);
		}
		if scope.contains_focused(window, cx) {
			return;
		}
	}
	// Nothing inside takes focus: park it on the scope itself.
	window.focus(scope, cx);
}

/// Tab or Shift-Tab: one tab stop on, confined to the topmost modal when one is up. Text
/// fields that handle Tab themselves should call this instead of `Window::focus_next`.
pub fn tab_step(forward: bool, window: &mut Window, cx: &mut App) {
	let scope = live_slots(KeyRole::TabScope, window, cx)
		.into_iter()
		.rev()
		.find_map(|slot| slot.read(cx).focus.clone());
	match scope {
		Some(scope) => focus_within(&scope, forward, window, cx),
		None if forward => window.focus_next(cx),
		None => window.focus_prev(cx),
	}
}

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
				window.update(cx, |_, window, cx| tab_step(true, window, cx)).ok();
			});
		}
	});
	cx.on_action(|_: &FocusPrevious, cx| {
		if let Some(window) = cx.active_window() {
			cx.defer(move |cx| {
				window
					.update(cx, |_, window, cx| tab_step(false, window, cx))
					.ok();
			});
		}
	});
	// An interceptor sees the window and runs before key bindings, so Return and Escape
	// work even when no control has focus. Escape in a text input is the input's own: it
	// ends a composition and blurs the field before it can dismiss anything.
	cx.intercept_keystrokes(|event, window, cx| {
		let stroke = &event.keystroke;
		if stroke.modifiers.modified() {
			return;
		}
		if stroke.key == "escape" && text_input_focused(window) {
			return;
		}
		if let Some(handler) = key_handler(&stroke.key, window, cx) {
			cx.stop_propagation();
			handler(window, cx);
		}
	})
	.detach();
}

/// Whether a text input has focus. It owns Escape (ending a composition, then blurring) and
/// Space, so neither dismisses a surface nor presses a control.
pub(crate) fn text_input_focused(window: &Window) -> bool {
	window
		.context_stack()
		.iter()
		.any(|context| context.contains(crate::keymap::INPUT_CONTEXT))
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
	use gpui::{Context, Entity, IntoElement, Render, TestAppContext, VisualTestContext};
	use std::cell::{Cell, RefCell};

	fn frame(pressed: bool) -> ControlFrame {
		ControlFrame {
			hovered: false,
			pressed,
			scale: 1.,
			press: 0.,
			light: 0.5,
			pointer: None,
			pointer_radius: px(10.),
		}
	}

	#[test]
	fn clear_hover_drops_hover_and_press() {
		let mut state = ControlState::new();
		assert!(!state.clear_hover());
		state.hovered = true;
		state.pressed = true;
		assert!(state.clear_hover());
		assert!(!state.hovered && !state.pressed);
		assert!(!state.clear_hover());
	}

	#[test]
	fn press_uses_the_flat_spring_and_release_the_jelly() {
		assert_eq!(scale_spring(true), PRESS);
		assert_eq!(scale_spring(false), JELLY);
		assert_eq!(PRESS.overshoot(), 0.);
		assert!(JELLY.overshoot() > 0.1);
	}

	#[test]
	fn release_overshoots_the_rest_scale_and_press_does_not() {
		let run = |from: f32, to: f32, spring: Spring| {
			let mut state = SpringState::at_rest(from);
			let (mut lo, mut hi) = (from, from);
			for _ in 0..240 {
				state = advance(state, to, spring, 1. / 60.).0;
				lo = lo.min(state.value);
				hi = hi.max(state.value);
			}
			(lo, hi)
		};
		let (lo, _) = run(1., 0.95, PRESS);
		assert!(lo >= 0.95 - 1e-4, "press undershot to {lo}");
		let (_, hi) = run(0.95, 1., JELLY);
		assert!(hi > 1.0005, "release should spring past rest, peaked at {hi}");
	}

	#[test]
	fn press_depth_tracks_the_scale() {
		assert_eq!(Feedback::Standard.press_depth(1.), 0.);
		assert_eq!(Feedback::Standard.press_depth(1.025), 0.);
		assert!((Feedback::Standard.press_depth(0.975) - 0.5).abs() < 1e-5);
		assert_eq!(Feedback::Standard.press_depth(0.9), 1.);
		assert!((Feedback::PressOnly.press_depth(0.97) - 1.).abs() < 1e-5);
	}

	#[test]
	fn pressed_shadows_shrink_blur_and_offset() {
		use gpui::hsla;
		let shadow = BoxShadow::new(px(0.), px(8.), hsla(0., 0., 0., 0.3)).blur_radius(px(20.));
		let rest = pressed_shadows(vec![shadow.clone()], 0.);
		assert_eq!(f32::from(rest[0].blur_radius), 20.);
		let down = pressed_shadows(vec![shadow.clone()], 1.);
		assert!((f32::from(down[0].blur_radius) - 12.).abs() < 1e-4);
		assert!((f32::from(down[0].offset.y) - 4.).abs() < 1e-4);
		let half = pressed_shadows(vec![shadow], 0.5);
		assert!((f32::from(half[0].blur_radius) - 16.).abs() < 1e-4);
		assert!((f32::from(half[0].offset.y) - 6.).abs() < 1e-4);
	}

	#[test]
	fn press_opacity_dips_only_while_pressed() {
		assert_eq!(frame(false).press_opacity(), 1.);
		assert_eq!(frame(true).press_opacity(), control::PRESSED_OPACITY);
	}

	#[test]
	fn hover_fill_scales_the_shared_hover_token_with_the_light() {
		let fill = frame(false).hover_fill(tokens::white(1.));
		assert!((fill.alpha - control::HOVER_OPACITY * 0.5).abs() < 1e-6);
	}

	#[test]
	fn row_fill_rests_transparent_and_hovers_at_the_shared_strength() {
		let accent = tokens::rgba(0.2, 0.4, 0.9, 1.);
		let ink = tokens::white(1.);
		assert_eq!(row_fill(ink, accent, 0., 0.).alpha, 0.);
		assert!((row_fill(ink, accent, 0., 1.).alpha - control::HOVER_OPACITY).abs() < 1e-6);
		// A selected row shows the selection fill and ignores hover.
		let selected = row_fill(ink, accent, 1., 1.);
		assert!((selected.alpha - control::SELECTED_ROW_FILL_OPACITY).abs() < 1e-6);
		assert_eq!(selected, row_fill(ink, accent, 1., 0.));
	}

	#[test]
	fn glow_follows_the_theme_unless_overridden() {
		let off = Theme::default();
		let on = Theme::default().with_style(off.style.with_pointer_glow(true));
		assert!(!glow_enabled(&off, None, false));
		assert!(glow_enabled(&on, None, false));
		assert!(glow_enabled(&off, Some(true), false));
		assert!(!glow_enabled(&on, Some(false), false));
	}

	#[test]
	fn glow_skips_compact_controls_and_reduce_transparency() {
		let on = Theme::default().with_style(Theme::default().style.with_pointer_glow(true));
		assert!(!glow_enabled(&on, None, true));
		assert!(!glow_enabled(&on, Some(true), true));
		let reduced = on.with_glass(crate::glass::GlassStyle::ReducedTransparency);
		assert!(!glow_enabled(&reduced, Some(true), false));
	}

	#[test]
	fn scaling_keeps_the_origin_fixed_and_round_trips() {
		let origin = point(px(100.), px(40.));
		assert_eq!(scale_about(origin, origin, 0.95), origin);
		let scaled = scale_about(point(px(120.), px(40.)), origin, 0.5);
		assert_eq!(scaled, point(px(110.), px(40.)));
		let p = point(px(133.), px(77.));
		let back = unscale_about(scale_about(p, origin, 1.025), origin, 1.025);
		assert!((back.x - p.x).abs() < px(1e-3) && (back.y - p.y).abs() < px(1e-3));
	}

	#[test]
	fn anchors_pick_their_fixed_point() {
		let bounds = Bounds::new(point(px(10.), px(20.)), gpui::size(px(100.), px(40.)));
		assert_eq!(ScaleAnchor::Center.origin(bounds), point(px(60.), px(40.)));
		assert_eq!(ScaleAnchor::BottomCenter.origin(bounds), point(px(60.), px(60.)));
	}

	#[test]
	fn unit_scale_takes_the_fast_path() {
		assert!(is_unscaled(1.));
		assert!(!is_unscaled(0.95));
		assert!(!is_unscaled(1.025));
	}

	#[test]
	fn key_step_defaults_to_five_percent_of_the_range() {
		assert!((key_step(&(0.0..=200.0), None) - 10.).abs() < 1e-6);
		assert_eq!(key_step(&(0.0..=1.0), Some(0.25)), 0.25);
	}

	#[test]
	fn the_focus_ring_is_a_soft_two_point_halo() {
		assert_eq!(FOCUS_RING_WIDTH, 2.);
		assert_eq!(FOCUS_RING_OFFSET, 2.);
		assert_eq!(FOCUS_OUTSET, FOCUS_RING_WIDTH + FOCUS_RING_OFFSET);
		assert!((0.4..=0.8).contains(&FOCUS_RING_ALPHA));
	}

	/// A control at (50, 50), 100 square, that records the frame it was given. Its pointer
	/// events go through [`ControlInteraction::control_state`], the path every control uses.
	struct Probe {
		shown: bool,
		/// `(hovered, pressed)` of the last frame.
		seen: Rc<Cell<(bool, bool)>>,
		state: Rc<RefCell<Option<Entity<ControlState>>>>,
		focus: Rc<RefCell<Option<FocusHandle>>>,
	}

	impl Render for Probe {
		fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
			let root = div().size_full();
			if !self.shown {
				return root;
			}
			let id: ElementId = "probe".into();
			let state = ControlState::of(&id, window, cx);
			let focus = focus_handle(&id, window, cx);
			let frame = ControlState::frame(&state, Feedback::Standard, true, false, window, cx);
			self.seen.set((frame.hovered, frame.pressed));
			*self.state.borrow_mut() = Some(state.clone());
			*self.focus.borrow_mut() = Some(focus.clone());
			root.child(
				div()
					.id(id)
					.track_focus(&focus)
					.absolute()
					.left(px(50.))
					.top(px(50.))
					.size(px(100.))
					.control_state(&state, false),
			)
		}
	}

	struct Rig {
		view: Entity<Probe>,
		seen: Rc<Cell<(bool, bool)>>,
		state: Rc<RefCell<Option<Entity<ControlState>>>>,
		focus: Rc<RefCell<Option<FocusHandle>>>,
	}

	const INSIDE: Point<Pixels> = point(px(100.), px(100.));
	const OUTSIDE: Point<Pixels> = point(px(400.), px(400.));

	fn rig(shown: bool, cx: &mut TestAppContext) -> (Rig, &mut VisualTestContext) {
		cx.update(crate::init);
		let seen = Rc::new(Cell::new((false, false)));
		let state = Rc::new(RefCell::new(None));
		let focus = Rc::new(RefCell::new(None));
		let (view, vcx) = cx.add_window_view({
			let (seen, state, focus) = (seen.clone(), state.clone(), focus.clone());
			move |_, _| Probe {
				shown,
				seen,
				state,
				focus,
			}
		});
		vcx.update(|window, _| window.activate());
		let rig = Rig {
			view,
			seen,
			state,
			focus,
		};
		rig.render(vcx);
		(rig, vcx)
	}

	impl Rig {
		/// Renders the probe again, as a state notification does.
		fn render(&self, cx: &mut VisualTestContext) {
			self.view.update(cx, |_, cx| cx.notify());
			cx.update(|window, cx| window.draw(cx).clear(cx));
		}

		fn show(&self, cx: &mut VisualTestContext) {
			self.view.update(cx, |probe, _| probe.shown = true);
			self.render(cx);
		}

		fn state(&self) -> Entity<ControlState> {
			self.state.borrow().clone().expect("the probe rendered")
		}
	}

	fn mouse(cx: &mut VisualTestContext, at: Point<Pixels>, action: &str) {
		let none = gpui::Modifiers::none();
		match action {
			"move" => cx.simulate_mouse_move(at, None, none),
			"down" => cx.simulate_mouse_down(at, MouseButton::Left, none),
			_ => cx.simulate_mouse_up(at, MouseButton::Left, none),
		}
	}

	#[gpui::test]
	fn a_pointer_that_left_without_an_exit_event_drops_hover_and_press(cx: &mut TestAppContext) {
		let (rig, cx) = rig(true, cx);
		// The pointer is at the origin, outside the control; no exit event will ever arrive.
		rig.state().update(cx, |state, _| {
			state.hovered = true;
			state.pressed = true;
		});
		rig.render(cx);
		assert_eq!(rig.seen.get(), (false, false));
	}

	#[gpui::test]
	fn a_press_does_not_survive_the_window_losing_activation(cx: &mut TestAppContext) {
		let (rig, cx) = rig(true, cx);
		mouse(cx, INSIDE, "move");
		mouse(cx, INSIDE, "down");
		rig.render(cx);
		assert_eq!(rig.seen.get(), (true, true));
		cx.deactivate_window();
		rig.render(cx);
		assert_eq!(rig.seen.get(), (false, false));
	}

	#[gpui::test]
	fn pressing_then_leaving_releases_the_press(cx: &mut TestAppContext) {
		let (rig, cx) = rig(true, cx);
		mouse(cx, INSIDE, "move");
		mouse(cx, INSIDE, "down");
		mouse(cx, OUTSIDE, "move");
		mouse(cx, OUTSIDE, "up");
		rig.render(cx);
		assert_eq!(rig.seen.get(), (false, false));
	}

	#[gpui::test]
	fn a_control_that_appears_under_a_still_pointer_releases_after_a_drag_off(cx: &mut TestAppContext) {
		let (rig, cx) = rig(false, cx);
		// The pointer rests where the control is about to appear and never moves into it.
		mouse(cx, INSIDE, "move");
		rig.show(cx);
		mouse(cx, INSIDE, "down");
		rig.render(cx);
		assert!(
			rig.seen.get().1,
			"the press should show while the pointer is on the control"
		);
		mouse(cx, OUTSIDE, "move");
		mouse(cx, OUTSIDE, "up");
		rig.render(cx);
		assert_eq!(rig.seen.get(), (false, false));
	}

	#[gpui::test]
	fn space_shows_the_press_for_a_moment(cx: &mut TestAppContext) {
		let (rig, cx) = rig(true, cx);
		let focus = rig.focus.borrow().clone().expect("the probe rendered");
		cx.update(|window, cx| window.focus(&focus, cx));
		// The pointer is elsewhere: only the keyboard presses.
		cx.simulate_keystrokes("space");
		rig.render(cx);
		assert_eq!(rig.seen.get(), (false, true), "Space should dip the control");
		std::thread::sleep(KEY_PRESS_PULSE + Duration::from_millis(30));
		rig.render(cx);
		assert_eq!(rig.seen.get(), (false, false), "the dip should end by itself");
	}

	#[test]
	fn the_reduce_motion_helper_follows_the_system_and_the_theme() {
		let cx = gpui::TestAppContext::single();
		cx.update(|cx| {
			crate::init(cx);
			assert!(!reduce_motion(cx));
			cx.set_reduce_motion(true);
			assert!(reduce_motion(cx));
			cx.set_reduce_motion(false);
			crate::Theme::update(cx, |theme| theme.reduce_motion = true);
			assert!(reduce_motion(cx));
		});
	}
}
