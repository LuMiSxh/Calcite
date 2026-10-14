// SPDX-License-Identifier: MPL-2.0

//! Form controls: [`toggle`] (`SettingsToggle`, in the elongated macOS 26 shape), [`slider`] (`SettingsSlider`),
//! [`segmented`] (`AdaptiveSegmentedControl`), and [`text_field`] (`ThemedTextField`).
//!
//! The controls are stateless: the view owns the value and passes a change handler, like a
//! SwiftUI binding split into a value and a setter.
//!
//! ```ignore
//! toggle("auto-update", self.auto_update).label("Automatic updates")
//!     .on_change(cx.listener(|this, on: &bool, _, cx| { this.auto_update = *on; cx.notify(); }))
//! slider("volume", self.volume).step(0.05).leading_icon(Lucide::Volume).trailing_icon(Lucide::Volume2)
//!     .ticks(11).value_label(true)
//!     .on_change(cx.listener(|this, value: &f32, _, cx| { this.volume = *value; cx.notify(); }))
//! segmented("mode", self.mode, [(Mode::Windowed, "Windowed"), (Mode::Fullscreen, "Fullscreen")])
//!     .on_change(cx.listener(|this, mode: &Mode, _, cx| { this.mode = *mode; cx.notify(); }))
//! ```
//!
//! Every control is a tab stop with the accent focus ring. Space and Return flip a toggle, and a
//! press can drag its knob to either side; the arrow keys step a slider and move a segmented selection.

use std::{ops::RangeInclusive, rc::Rc};

use gpui::{
	App, Bounds, ClickEvent, DispatchPhase, ElementId, Entity, Filter, FocusHandle, Focusable, FontWeight,
	IntoElement, MouseButton, ParentElement, Pixels, Role, SharedString, Styled, Toggled, Window, canvas,
	div, prelude::*, px,
};
use gpui_ce_elements::editable_text::{EditableTextState, StringStorage, actions, text_input};

use crate::{
	controls::{Size, theme_override},
	glass::Elevation,
	icons::IconRef,
	interaction::{
		ControlInteraction, ControlState, Feedback, FocusShape, KeyRole, KeyboardFocus, ScaleExt,
		arrow_direction, focus_handle, glow_enabled, glow_layer, key_step, register_key_target,
	},
	knob::{LENS_GROWTH, SliderPress, ToggleDrag, knob_surface, lens_amount, release_on, release_press},
	motion::{self, Curve, secondary, tokens::SELECT},
	theme::{ActiveTheme, Theme},
	tokens::{self, Chromatic, control, scale_alpha, white, with_alpha},
};

/// A shared change handler, for controls that hand it to several listeners.
pub(crate) type SharedChangeHandler<T> = std::rc::Rc<dyn Fn(&T, &mut Window, &mut App) + 'static>;

type ChangeHandler<T> = Box<dyn Fn(&T, &mut Window, &mut App) + 'static>;

/// An accent switch in the elongated Liquid Glass shape of macOS 26 and 27: a capsule thumb
/// that turns into a clear glass lens while pressed and stretches as it travels.
#[derive(IntoElement)]
pub struct Toggle {
	id: ElementId,
	on: bool,
	label: SharedString,
	size: Size,
	enabled: bool,
	glow: Option<bool>,
	theme: Option<Theme>,
	on_change: Option<ChangeHandler<bool>>,
}

/// A switch showing `on`. Give it a `.label(..)`: it is the accessible name.
pub fn toggle(id: impl Into<ElementId>, on: bool) -> Toggle {
	Toggle {
		id: id.into(),
		on,
		label: SharedString::default(),
		size: Size::Regular,
		enabled: true,
		glow: None,
		theme: None,
		on_change: None,
	}
}

/// Track and thumb geometry. Large is the 64 × 28 track and 39 × 24 thumb of Apple's
/// iOS 26 and macOS 26 design kits; the other sizes keep its proportions.
struct SwitchMetrics {
	track: (f32, f32),
	thumb: (f32, f32),
}

impl SwitchMetrics {
	fn of(size: Size) -> Self {
		match size {
			Size::Large => Self {
				track: (64., 28.),
				thumb: (39., 24.),
			},
			Size::Regular => Self {
				track: (48., 22.),
				thumb: (29., 18.),
			},
			Size::Compact => Self {
				track: (38., 18.),
				thumb: (23., 14.),
			},
		}
	}
}

/// Ends a switch gesture and reports the new value once through `on_change`, if it changed.
fn settle(
	drag: &Entity<ToggleDrag>,
	on: bool,
	on_change: &SharedChangeHandler<bool>,
	window: &mut Window,
	cx: &mut App,
) {
	let Some(progress) = drag.update(cx, |drag, cx| {
		let ended = drag.end();
		if ended.is_some() {
			cx.notify();
		}
		ended
	}) else {
		return;
	};
	let next = release_on(on, progress);
	if next != on {
		on_change(&next, window, cx);
	}
}

impl Toggle {
	pub fn label(mut self, label: impl Into<SharedString>) -> Self {
		self.label = label.into();
		self
	}

	pub fn size(mut self, size: Size) -> Self {
		self.size = size;
		self
	}

	pub fn large(self) -> Self {
		self.size(Size::Large)
	}

	pub fn compact(self) -> Self {
		self.size(Size::Compact)
	}

	/// Forces the pointer light on or off. Without it the switch follows
	/// [`Style::pointer_glow`](crate::style::Style::pointer_glow); compact switches never glow.
	pub fn glow(mut self, glow: bool) -> Self {
		self.glow = Some(glow);
		self
	}

	pub fn disabled(mut self, disabled: bool) -> Self {
		self.enabled = !disabled;
		self
	}

	pub fn on_change(mut self, handler: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
		self.on_change = Some(Box::new(handler));
		self
	}
}

impl RenderOnce for Toggle {
	fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let reduce_motion = theme.reduce_motion;
		let state = ControlState::of(&self.id, window, cx);
		let focus = focus_handle(&self.id, window, cx);
		let frame = ControlState::frame(
			&state,
			Feedback::PressOnly,
			self.enabled,
			reduce_motion,
			window,
			cx,
		);
		let target = if self.on { 1. } else { 0. };
		let drag = window.use_keyed_state((self.id.clone(), "toggle-drag"), cx, |_, _| ToggleDrag::default());
		let thumb_spring = Curve::Release.spring().expect("Release is a spring");
		let (position, velocity) = match drag.read(cx).progress {
			// While the pointer holds the knob it sits under the pointer, not on its spring. The
			// spring's anchor is kept at the knob, so the release springs from where it was let go.
			Some(progress) => {
				motion::value::hold((self.id.clone(), "thumb"), progress, window, cx);
				(progress, 0.)
			}
			None => {
				let sample = motion::spring(
					(self.id.clone(), "thumb"),
					target,
					thumb_spring,
					reduce_motion,
					window,
					cx,
				);
				(sample.value, sample.velocity)
			}
		};
		let lens = lens_amount(
			(self.id.clone(), "lens"),
			frame.pressed,
			reduce_motion,
			window,
			cx,
		);

		let SwitchMetrics {
			track: (track_width, track_height),
			thumb: (thumb_width, thumb_height),
		} = SwitchMetrics::of(self.size);
		let inset = (track_height - thumb_height) / 2.;
		let travel = track_width - thumb_width - 2. * inset;
		// The knob stretches with its speed: widest mid-travel, round again at rest.
		let (stretch_width, stretch_height) = secondary::stretch(velocity, reduce_motion);
		let width = thumb_width * (1. + LENS_GROWTH.0 * lens) * stretch_width;
		let height = thumb_height * (1. + LENS_GROWTH.1 * lens) * stretch_height;
		let center = inset + thumb_width / 2. + travel * position.clamp(-0.08, 1.08);

		let off_track = if theme.reduce_transparency() {
			theme.palette.opaque_surface
		} else {
			white(0.16)
		};
		let track = tokens::mix(off_track, theme.accent, position.clamp(0., 1.));
		let alpha = if self.enabled {
			1.
		} else {
			control::DISABLED_FOREGROUND_OPACITY
		};
		let thumb = div()
			.absolute()
			.left(px(center - width / 2.))
			.top(px((track_height - height) / 2.))
			.w(px(width))
			.h(px(height))
			.rounded_full();
		let thumb = knob_surface(thumb, lens, height, alpha, 0.28, &theme);
		let on = self.on;
		let glows = self.enabled && glow_enabled(&theme, self.glow, self.size == Size::Compact);
		let glow = glows
			.then(|| glow_layer(&frame, white(1.)))
			.flatten()
			.map(|glow| glow.rounded_full());
		let track = div()
			.id(self.id)
			.relative()
			.w(px(track_width))
			.h(px(track_height))
			.flex_none()
			.rounded_full()
			.bg(scale_alpha(track, alpha))
			.role(Role::Switch)
			.aria_label(self.label)
			.aria_toggled(if on { Toggled::True } else { Toggled::False })
			.aria_disabled(!self.enabled)
			.control_state(&state, glows)
			.children(glow)
			.child(thumb)
			.when(self.enabled, |this| {
				this.keyboard_focus(&focus, FocusShape::Capsule, &theme, window)
			});
		let on_change: Option<SharedChangeHandler<bool>> =
			self.on_change.filter(|_| self.enabled).map(Rc::from);
		let track = match on_change {
			Some(on_change) => {
				let (press, moved, released, left) = (drag.clone(), drag.clone(), drag.clone(), drag.clone());
				let (changed, outside, lost, keys) =
					(on_change.clone(), on_change.clone(), on_change.clone(), on_change);
				track
					.on_mouse_down(MouseButton::Left, move |event, _, cx| {
						press.update(cx, |drag, cx| {
							drag.begin(f32::from(event.position.x), position);
							cx.notify();
						});
					})
					// The knob keeps following the pointer after it leaves the switch.
					.on_mouse_move_all(move |event, phase, _, window, cx| {
						if phase != DispatchPhase::Bubble {
							return;
						}
						// A release that never reached the switch (outside the window, lost focus) ends the gesture here.
						if event.pressed_button != Some(MouseButton::Left) {
							settle(&moved, on, &lost, window, cx);
							return;
						}
						moved.update(cx, |drag, cx| {
							if drag.move_to(f32::from(event.position.x), travel) {
								cx.notify();
							}
						});
					})
					.on_mouse_up(MouseButton::Left, move |_, window, cx| {
						settle(&released, on, &changed, window, cx)
					})
					.on_mouse_up_out(MouseButton::Left, move |_, window, cx| {
						settle(&left, on, &outside, window, cx)
					})
					// A mouse click was settled by the press and release above; keyboard clicks flip it here.
					.on_click(move |event, window, cx| {
						if !matches!(event, ClickEvent::Mouse(_)) {
							keys(&!on, window, cx);
						}
					})
			}
			None => track,
		};
		track.scaled(frame.scale)
	}
}

/// The slider thumb: 16 pt tall, and 1.45 times as wide, a capsule like the macOS 26 slider knob.
const THUMB_HEIGHT: f32 = 16.;
const THUMB_WIDTH: f32 = THUMB_HEIGHT * 1.45;

/// A horizontal value slider with an accent fill, optionally framed by icons, a tick row,
/// and a percent label, like the launcher's volume rows.
#[derive(IntoElement)]
pub struct Slider {
	id: ElementId,
	value: f32,
	range: RangeInclusive<f32>,
	step: Option<f32>,
	width: Pixels,
	label: SharedString,
	leading_icon: Option<IconRef>,
	trailing_icon: Option<IconRef>,
	ticks: usize,
	value_label: bool,
	enabled: bool,
	theme: Option<Theme>,
	on_change: Option<SharedChangeHandler<f32>>,
}

/// A slider showing `value` in `0.0..=1.0` unless `.range(..)` says otherwise.
pub fn slider(id: impl Into<ElementId>, value: f32) -> Slider {
	Slider {
		id: id.into(),
		value,
		range: 0.0..=1.0,
		step: None,
		width: px(180.),
		label: SharedString::default(),
		leading_icon: None,
		trailing_icon: None,
		ticks: 0,
		value_label: false,
		enabled: true,
		theme: None,
		on_change: None,
	}
}

impl Slider {
	pub fn range(mut self, range: RangeInclusive<f32>) -> Self {
		self.range = range;
		self
	}

	/// Snaps the value to multiples of `step`; the arrow keys move by it too. Without it,
	/// the arrow keys move 5 % of the range.
	pub fn step(mut self, step: f32) -> Self {
		self.step = Some(step);
		self
	}

	/// The width of the track.
	pub fn width(mut self, width: Pixels) -> Self {
		self.width = width;
		self
	}

	pub fn label(mut self, label: impl Into<SharedString>) -> Self {
		self.label = label.into();
		self
	}

	/// A decorative icon before the track, such as a quiet speaker.
	pub fn leading_icon(mut self, icon: impl Into<IconRef>) -> Self {
		self.leading_icon = Some(icon.into());
		self
	}

	/// A decorative icon after the track, such as a loud speaker.
	pub fn trailing_icon(mut self, icon: impl Into<IconRef>) -> Self {
		self.trailing_icon = Some(icon.into());
		self
	}

	/// A dotted row of `count` evenly spaced ticks under the track. Fewer than two draw none.
	pub fn ticks(mut self, count: usize) -> Self {
		self.ticks = count;
		self
	}

	/// Shows the value as a percent of the range after the track.
	pub fn value_label(mut self, shown: bool) -> Self {
		self.value_label = shown;
		self
	}

	pub fn disabled(mut self, disabled: bool) -> Self {
		self.enabled = !disabled;
		self
	}

	pub fn on_change(mut self, handler: impl Fn(&f32, &mut Window, &mut App) + 'static) -> Self {
		self.on_change = Some(std::rc::Rc::new(handler));
		self
	}
}

/// Snaps `value` to `step` from the start of `range` and clamps it into the range.
fn snap(value: f32, range: &RangeInclusive<f32>, step: Option<f32>) -> f32 {
	let (start, end) = (*range.start(), *range.end());
	let value = match step.filter(|step| *step > 0.) {
		Some(step) => start + ((value - start) / step).round() * step,
		None => value,
	};
	value.clamp(start.min(end), start.max(end))
}

/// The thumb's centre x, in track pixels, at `progress` (0 to 1). The thumb travels between its two
/// inset positions, so the capsule never overhangs either end of the track.
fn thumb_center(progress: f32, track_width: f32, thumb_width: f32) -> f32 {
	thumb_width / 2. + (track_width - thumb_width).max(0.) * progress.clamp(0., 1.)
}

/// The inverse of [`thumb_center`]: the progress (0 to 1) whose thumb centre sits at `center`.
fn thumb_progress(center: f32, track_width: f32, thumb_width: f32) -> f32 {
	let travel = track_width - thumb_width;
	if travel <= 0. {
		return 0.;
	}
	((center - thumb_width / 2.) / travel).clamp(0., 1.)
}

/// Maps a pointer x position to a stepped value inside `range`. It uses the thumb's inset, so the
/// thumb stays under the pointer while dragging.
fn slider_value(x: Pixels, bounds: Bounds<Pixels>, range: &RangeInclusive<f32>, step: Option<f32>) -> f32 {
	let width = f32::from(bounds.size.width).max(1.);
	let fraction = thumb_progress(f32::from(x - bounds.origin.x), width, THUMB_WIDTH);
	let (start, end) = (*range.start(), *range.end());
	snap(start + (end - start) * fraction, range, step)
}

/// The value one arrow key press moves to, in the direction of the range.
fn stepped_value(value: f32, direction: i32, range: &RangeInclusive<f32>, step: Option<f32>) -> f32 {
	let sign = if range.end() >= range.start() { 1. } else { -1. };
	let moved = value + direction as f32 * sign * key_step(range, step);
	snap(moved, range, step)
}

impl RenderOnce for Slider {
	fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let bounds = window.use_keyed_state((self.id.clone(), "track"), cx, |_, _| None::<Bounds<Pixels>>);
		let reduce_motion = theme.reduce_motion;
		let focus = focus_handle(&self.id, window, cx);
		let (start, end) = (*self.range.start(), *self.range.end());
		let fraction = if end == start {
			0.
		} else {
			((self.value - start) / (end - start)).clamp(0., 1.)
		};
		let percent = (fraction * 100.).round() as i32;
		let alpha = if self.enabled {
			1.
		} else {
			control::DISABLED_FOREGROUND_OPACITY
		};
		let secondary = scale_alpha(theme.palette.secondary_text, alpha);
		let track_height = THUMB_HEIGHT + 4.;
		let track_width = f32::from(self.width);
		// The ticks line up with the thumb's center at both ends of its travel.
		let ticks = (self.ticks >= 2).then(|| {
			div()
				.absolute()
				.top(px(track_height + 2.))
				.left(px(THUMB_WIDTH / 2. - 1.))
				.right(px(THUMB_WIDTH / 2. - 1.))
				.flex()
				.flex_row()
				.justify_between()
				.children((0..self.ticks).map(|_| div().size(px(2.)).rounded_full().bg(secondary)))
		});
		let height = if ticks.is_some() {
			track_height + 6.
		} else {
			track_height
		};
		// The thumb grows into the glass lens while a press holds it, as the switch knob does.
		let press_state = window.use_keyed_state((self.id.clone(), "slider-press"), cx, |_, _| {
			SliderPress::default()
		});
		let held = press_state.read(cx).held;
		let lens = lens_amount((self.id.clone(), "lens"), held, reduce_motion, window, cx);
		// The taller growth scales both sides, so the lens keeps the capsule's aspect.
		let grow = 1. + LENS_GROWTH.1 * lens;
		let (grown_width, grown_height) = (THUMB_WIDTH * grow, THUMB_HEIGHT * grow);
		let center = thumb_center(fraction, track_width, THUMB_WIDTH);
		let knob = knob_surface(
			div()
				.absolute()
				.left(px(center - grown_width / 2.))
				.top(px((track_height - grown_height) / 2.))
				.w(px(grown_width))
				.h(px(grown_height))
				.rounded_full(),
			lens,
			grown_height,
			alpha,
			0.35,
			&theme,
		);
		let probe = bounds.clone();
		let mut slider = div()
			.id(self.id.clone())
			.relative()
			.w(self.width)
			.h(px(height))
			.flex_none()
			.role(Role::Slider)
			.aria_label(self.label)
			.aria_numeric_value(f64::from(self.value))
			.aria_min_numeric_value(f64::from(start.min(end)))
			.aria_max_numeric_value(f64::from(start.max(end)))
			.aria_numeric_value_step(f64::from(key_step(&self.range, self.step)))
			.aria_value(format!("{percent} percent"))
			.aria_disabled(!self.enabled)
			.child(
				div()
					.absolute()
					.left_0()
					.right_0()
					.top(px(THUMB_HEIGHT / 2.))
					.h(px(4.))
					.rounded_full()
					.bg(white(0.18 * alpha)),
			)
			.child(
				div()
					.absolute()
					.left_0()
					.top(px(THUMB_HEIGHT / 2.))
					.w(px(thumb_center(fraction, track_width, THUMB_WIDTH)))
					.h(px(4.))
					.rounded_full()
					.bg(scale_alpha(theme.accent, alpha)),
			)
			.child(knob)
			.children(ticks)
			.child(
				canvas(
					move |area, _, cx| probe.update(cx, |bounds, _| *bounds = Some(area)),
					|_, _, _, _| {},
				)
				.absolute()
				.size_full(),
			);
		if self.enabled {
			slider = slider.keyboard_focus(&focus, FocusShape::Capsule, &theme, window);
		}
		if let (true, Some(on_change)) = (self.enabled, self.on_change) {
			let (range, step) = (self.range.clone(), self.step);
			let press = (bounds.clone(), on_change.clone(), range.clone());
			let keys = (on_change.clone(), range.clone(), self.value);
			let (down, up, up_out, lost) = (
				press_state.clone(),
				press_state.clone(),
				press_state.clone(),
				press_state,
			);
			slider = slider
				.on_key_down(move |event, window, cx| {
					let (on_change, range, value) = &keys;
					if let Some(direction) = arrow_direction(event) {
						cx.stop_propagation();
						on_change(&stepped_value(*value, direction, range, step), window, cx);
					}
				})
				.on_mouse_down(MouseButton::Left, move |event, window, cx| {
					down.update(cx, |state, cx| {
						state.hold();
						cx.notify();
					});
					if let Some(area) = *press.0.read(cx) {
						(press.1)(&slider_value(event.position.x, area, &press.2, step), window, cx);
					}
				})
				.on_mouse_up(MouseButton::Left, move |_, _, cx| release_press(&up, cx))
				.on_mouse_up_out(MouseButton::Left, move |_, _, cx| release_press(&up_out, cx))
				// The press holds the thumb for the whole drag, even off the track. A release the
				// track never saw (outside the window) ends it on the next move.
				.on_mouse_move_all(move |event, phase, _, _, cx| {
					if phase == DispatchPhase::Bubble && event.pressed_button != Some(MouseButton::Left) {
						release_press(&lost, cx);
					}
				})
				.on_mouse_move(move |event, window, cx| {
					if event.pressed_button != Some(MouseButton::Left) {
						return;
					}
					if let Some(area) = *bounds.read(cx) {
						on_change(&slider_value(event.position.x, area, &range, step), window, cx);
					}
				});
		}
		if self.leading_icon.is_none() && self.trailing_icon.is_none() && !self.value_label {
			return slider.into_any_element();
		}
		// Side items center on the track, not on the track and its ticks.
		let beside = || div().h(px(track_height)).flex().items_center().flex_none();
		div()
			.flex()
			.flex_row()
			.font_family(theme.sans())
			.items_start()
			.gap(tokens::space::CONTROL)
			.flex_none()
			.when_some(self.leading_icon, |row, icon| {
				row.child(beside().child(icon.render(tokens::text::BODY, secondary)))
			})
			.child(slider)
			.when_some(self.trailing_icon, |row, icon| {
				row.child(beside().child(icon.render(tokens::text::BODY, secondary)))
			})
			.when(self.value_label, |row| {
				// The slider reports its value itself, so the label stays out of the
				// accessibility tree (`accessibilityHidden`).
				row.child(
					beside()
						.id((self.id, "value"))
						.aria_hidden()
						.w(px(42.))
						.justify_end()
						.text_size(tokens::text::CAPTION)
						.text_color(secondary)
						.child(format!("{percent}%")),
				)
			})
			.into_any_element()
	}
}

/// A row of mutually exclusive options; the selection carries the accent glass.
#[derive(IntoElement)]
pub struct Segmented<T: Clone + PartialEq + 'static> {
	id: ElementId,
	selected: T,
	options: Vec<(T, SharedString)>,
	enabled: bool,
	theme: Option<Theme>,
	on_change: Option<SharedChangeHandler<T>>,
}

/// A segmented control over `options`, showing `selected`.
pub fn segmented<T: Clone + PartialEq + 'static, L: Into<SharedString>>(
	id: impl Into<ElementId>,
	selected: T,
	options: impl IntoIterator<Item = (T, L)>,
) -> Segmented<T> {
	Segmented {
		id: id.into(),
		selected,
		options: options
			.into_iter()
			.map(|(value, label)| (value, label.into()))
			.collect(),
		enabled: true,
		theme: None,
		on_change: None,
	}
}

impl<T: Clone + PartialEq + 'static> Segmented<T> {
	pub fn disabled(mut self, disabled: bool) -> Self {
		self.enabled = !disabled;
		self
	}

	pub fn on_change(mut self, handler: impl Fn(&T, &mut Window, &mut App) + 'static) -> Self {
		self.on_change = Some(std::rc::Rc::new(handler));
		self
	}

	pub fn theme(mut self, theme: Theme) -> Self {
		self.theme = Some(theme);
		self
	}
}

impl<T: Clone + PartialEq + 'static> RenderOnce for Segmented<T> {
	fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let focus = focus_handle(&self.id, window, cx);
		let accent = tokens::contrast_tint(theme.accent);
		let reduced = theme.reduce_transparency();
		let container_fill = if reduced {
			theme.palette.opaque_surface
		} else {
			white(control::ENABLED_NEUTRAL_SURFACE_OPACITY)
		};
		let chromatic = Chromatic::enabled(theme.accent);
		let selected_glass = theme.surface(Some(chromatic.fill)).elevation(Elevation::None);
		let enabled = self.enabled;
		let id = self.id.clone();
		let count = self.options.len();
		let selected_index = self.options.iter().position(|(value, _)| *value == self.selected);
		let values: Vec<T> = self.options.iter().map(|(value, _)| value.clone()).collect();
		let keys = self.on_change.clone().filter(|_| enabled);
		// One glass indicator slides between the segments, like a macOS segmented control.
		// It needs the laid-out width, so the first frame styles the selected segment itself.
		let bounds = window.use_keyed_state((id.clone(), "calcite-segmented-bounds"), cx, |_, _| {
			None::<Bounds<Pixels>>
		});
		let probe = bounds.clone();
		let width = bounds.read(cx).map(|bounds| f32::from(bounds.size.width));
		let slide = motion::spring(
			(id.clone(), "calcite-segmented-position"),
			selected_index.unwrap_or(0) as f32,
			SELECT,
			theme.reduce_motion,
			window,
			cx,
		);
		let position = slide.value;
		let indicator = width
			.filter(|_| selected_index.is_some() && count > 0)
			.map(|width| {
				let inner = width - 2. * SEGMENTED_PADDING;
				let segment = (inner - SEGMENTED_GAP * (count as f32 - 1.)) / count as f32;
				let (left, indicator_width) = lead_edge(
					SEGMENTED_PADDING + position * (segment + SEGMENTED_GAP),
					segment,
					slide.velocity,
					theme.reduce_motion,
				);
				let indicator = div()
					.absolute()
					.top(px(SEGMENTED_PADDING))
					.bottom(px(SEGMENTED_PADDING))
					.left(px(left))
					.w(px(indicator_width))
					.rounded_full()
					.border_1()
					.border_color(chromatic.border);
				match selected_glass.filter() {
					Some(filter) => indicator.backdrop_filter(vec![Filter::Glass(filter)]),
					None => indicator.bg(selected_glass.fill()),
				}
			});
		let slides = indicator.is_some();
		div()
			.font_family(theme.sans())
			.id(self.id)
			.relative()
			.child(
				canvas(
					move |bounds, _, cx| probe.update(cx, |stored, _| *stored = Some(bounds)),
					|_, _, _, _| {},
				)
				.absolute()
				.size_full(),
			)
			.children(indicator)
			.flex()
			.flex_row()
			.gap(px(SEGMENTED_GAP))
			.p(px(SEGMENTED_PADDING))
			.rounded_full()
			.bg(container_fill)
			.border_1()
			.border_color(with_alpha(
				theme.palette.control,
				control::ENABLED_NEUTRAL_BORDER_OPACITY,
			))
			.role(Role::RadioGroup)
			.aria_disabled(!enabled)
			.when(enabled, |this| {
				this.keyboard_focus(&focus, FocusShape::Capsule, &theme, window)
			})
			// Like a macOS segmented control, the arrow keys move the selection itself.
			.when_some(keys, move |this, on_change| {
				this.on_key_down(move |event, window, cx| {
					let Some(direction) = arrow_direction(event) else {
						return;
					};
					cx.stop_propagation();
					let next = match selected_index {
						Some(index) => index.saturating_add_signed(direction as isize).min(count - 1),
						None => 0,
					};
					if Some(next) != selected_index
						&& let Some(value) = values.get(next)
					{
						on_change(value, window, cx);
					}
				})
			})
			.children(
				self.options
					.into_iter()
					.enumerate()
					.map(|(index, (value, label))| {
						let selected = value == self.selected;
						let segment_id: ElementId =
							(id.clone(), SharedString::from(index.to_string())).into();
						let segment_state = ControlState::of(&segment_id, window, cx);
						let segment_frame = ControlState::frame(
							&segment_state,
							Feedback::PressOnly,
							enabled,
							theme.reduce_motion,
							window,
							cx,
						);
						let on_change = self.on_change.clone();
						let foreground = if !enabled {
							with_alpha(theme.accent, control::DISABLED_FOREGROUND_OPACITY)
						} else if selected {
							accent
						} else {
							theme.palette.secondary_text
						};
						let segment = div()
							.id(segment_id)
							.flex_1()
							.flex()
							.items_center()
							.justify_center()
							.px(px(10.))
							.py(px(5.))
							.rounded_full()
							.text_size(tokens::text::CAPTION)
							.font_weight(FontWeight::SEMIBOLD)
							.text_color(foreground)
							.role(Role::RadioButton)
							.aria_label(label.clone())
							.aria_selected(selected)
							.aria_toggled(if selected { Toggled::True } else { Toggled::False })
							.aria_position_in_set(index + 1)
							.aria_size_of_set(count)
							.child(label);
						let segment = if selected && slides {
							segment
						} else if selected {
							let segment = segment.border_1().border_color(chromatic.border);
							match selected_glass.filter() {
								Some(filter) => segment.backdrop_filter(vec![Filter::Glass(filter)]),
								None => segment.bg(selected_glass.fill()),
							}
						} else {
							segment
								.bg(segment_frame.hover_fill(white(1.)))
								.control_state(&segment_state, false)
						};
						segment.when_some(
							on_change.filter(|_| enabled && !selected),
							move |segment, on_change| {
								segment.on_click(move |_, window, cx| on_change(&value, window, cx))
							},
						)
					}),
			)
	}
}

/// The indicator of a segmented control while it moves at `velocity` segments per second: the edge
/// it moves toward stretches by half of the knob stretch ([`secondary::stretch`]) and the trailing
/// edge stays, so it reads as a drop pulled along. At rest it is `(left, width)` unchanged.
fn lead_edge(left: f32, width: f32, velocity: f32, reduced: bool) -> (f32, f32) {
	let extra = width * (secondary::stretch(velocity, reduced).0 - 1.) * 0.5;
	if velocity < 0. {
		(left - extra, width + extra)
	} else {
		(left, width + extra)
	}
}

/// Inner padding and spacing of a segmented control. The border sits outside the padding.
const SEGMENTED_PADDING: f32 = 4.;
const SEGMENTED_GAP: f32 = 4.;

/// Keeps a [`TextField`]'s observer of its state alive while the field is drawn. It watches
/// the state it was made for, so a field that is given another state starts a new observer.
struct Redraw {
	state: gpui::EntityId,
	_observe: gpui::Subscription,
}

impl Redraw {
	/// Re-renders `view` whenever `state` notifies.
	fn observing(
		state: &Entity<EditableTextState>,
		view: gpui::EntityId,
		cx: &mut gpui::Context<Self>,
	) -> Self {
		Self {
			state: state.entity_id(),
			_observe: cx.observe(state, move |_, _, cx| gpui::AppContext::notify(cx, view)),
		}
	}
}

/// A single-line input in the accent capsule (`ThemedTextField`).
///
/// The text lives in an [`EditableTextState`] the view owns; create it with
/// [`text_state`] and read it with `state.read(cx).as_str()`, or subscribe to its
/// `TextChanged` event.
#[derive(IntoElement)]
pub struct TextField {
	id: ElementId,
	state: Entity<EditableTextState>,
	placeholder: SharedString,
	icon: Option<IconRef>,
	width: Option<Pixels>,
	theme: Option<Theme>,
}

/// A text field editing `state`.
pub fn text_field(id: impl Into<ElementId>, state: &Entity<EditableTextState>) -> TextField {
	TextField {
		id: id.into(),
		state: state.clone(),
		placeholder: SharedString::default(),
		icon: None,
		width: None,
		theme: None,
	}
}

/// Creates the state of a text field with `initial` text.
pub fn text_state(initial: &str, cx: &mut App) -> Entity<EditableTextState> {
	let storage = StringStorage::from(initial);
	cx.new(|cx| EditableTextState::new(storage, cx))
}

impl TextField {
	pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
		self.placeholder = placeholder.into();
		self
	}

	pub fn icon(mut self, icon: impl Into<IconRef>) -> Self {
		self.icon = Some(icon.into());
		self
	}

	pub fn width(mut self, width: Pixels) -> Self {
		self.width = Some(width);
		self
	}
}

impl RenderOnce for TextField {
	fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let focus: FocusHandle = self.state.read(cx).focus_handle(cx);
		let focused = focus.is_focused(window);
		let hover_state = ControlState::of(&self.id, window, cx);
		let hover = ControlState::frame(
			&hover_state,
			Feedback::PressOnly,
			true,
			theme.reduce_motion,
			window,
			cx,
		);
		// The text notifies only its own state, and a cached view around the field would not
		// re-render for that. A keyed observer re-renders the view the field is drawn in, as
		// `use_keyed_state` does for the states it creates.
		let view = window.current_view();
		let state = self.state.clone();
		let redraw = window.use_keyed_state(self.id.clone(), cx, {
			let state = state.clone();
			move |_, cx| Redraw::observing(&state, view, cx)
		});
		redraw.update(cx, |redraw, cx| {
			if redraw.state != state.entity_id() {
				*redraw = Redraw::observing(&state, view, cx);
			}
		});
		// A single-line field has no use for Return, so it reaches the default action.
		register_key_target(
			&self.id,
			KeyRole::PassReturn,
			None,
			Some(focus.clone()),
			window,
			cx,
		);
		let chromatic = Chromatic::enabled(theme.accent);
		let glass = theme
			.surface(Some(chromatic.fill))
			.elevation(Elevation::None)
			.overlay(hover.hover_fill(white(1.)));
		let border = if focused { theme.accent } else { chromatic.border };
		let icon_color = if focused {
			tokens::contrast_tint(theme.accent)
		} else {
			theme.palette.secondary_text
		};
		let field = div()
			.id(self.id.clone())
			.flex()
			.flex_row()
			.items_center()
			.font_family(theme.sans())
			.gap(px(7.))
			.px(px(12.))
			.h(control::HEIGHT_REGULAR)
			.rounded_full()
			.border_1()
			.border_color(border)
			.text_size(tokens::text::BODY)
			.text_color(theme.palette.primary_text)
			.control_state(&hover_state, false)
			.when_some(self.width, |field, width| field.w(width))
			// The editor element does not join the tab order itself; the capsule joins it
			// with the editor's handle, and focus still lands on the editor.
			.track_focus(&focus.clone().tab_stop(true))
			// The editor would swallow Tab for a tab character; a single line moves focus instead.
			.capture_action(|_: &actions::Tab, window, cx| {
				cx.stop_propagation();
				window.focus_next(cx);
			})
			.when_some(self.icon, |field, icon| {
				field.child(icon.render(px(11.), icon_color))
			})
			.child(
				text_input((self.id, "input"))
					.state(self.state.downgrade())
					.placeholder(self.placeholder)
					.placeholder_color(theme.palette.secondary_text)
					.caret_color(gpui::rgb_to_hsla(tokens::contrast_tint(theme.accent)))
					.selection_color(gpui::rgb_to_hsla(scale_alpha(theme.accent, 0.35)))
					.flex_1()
					.min_w_0()
					.whitespace_nowrap()
					.overflow_x_scroll(),
			);
		match glass.filter() {
			Some(filter) => field.backdrop_filter(vec![Filter::Glass(filter)]),
			None => field.bg(glass.fill()),
		}
	}
}

theme_override!(Toggle, Slider, TextField);

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn indicator_is_unchanged_at_rest_and_under_reduce_motion() {
		assert_eq!(lead_edge(10., 80., 0., false), (10., 80.));
		assert_eq!(lead_edge(10., 80., 8., true), (10., 80.));
	}

	#[test]
	fn indicator_stretches_toward_the_edge_it_moves_to() {
		// Full speed: the knob stretch is 22%, half of it on the lead edge.
		let (left, width) = lead_edge(10., 100., 8., false);
		assert_eq!(left, 10.);
		assert!((width - 111.).abs() < 1e-3);
		let (left, width) = lead_edge(10., 100., -8., false);
		assert!((left - -1.).abs() < 1e-3 && (width - 111.).abs() < 1e-3);
		// The trailing edge stays put either way.
		assert!((left + width - 110.).abs() < 1e-3);
	}

	#[test]
	fn thumb_is_a_capsule_1_45_times_as_wide_as_it_is_tall() {
		assert!((THUMB_WIDTH / THUMB_HEIGHT - 1.45).abs() < 1e-6);
	}

	#[test]
	fn thumb_center_runs_between_the_inset_ends() {
		let half = THUMB_WIDTH / 2.;
		assert!((thumb_center(0., 200., THUMB_WIDTH) - half).abs() < 1e-3);
		assert!((thumb_center(1., 200., THUMB_WIDTH) - (200. - half)).abs() < 1e-3);
		assert!((thumb_center(0.5, 200., THUMB_WIDTH) - 100.).abs() < 1e-3);
	}

	#[test]
	fn thumb_center_stays_inside_the_track() {
		let half = THUMB_WIDTH / 2.;
		for step in 0..=100 {
			let center = thumb_center(step as f32 / 100., 200., THUMB_WIDTH);
			assert!(center - half >= -1e-3 && center + half <= 200. + 1e-3);
		}
		assert!((thumb_center(-1., 200., THUMB_WIDTH) - half).abs() < 1e-3);
		assert!((thumb_center(2., 200., THUMB_WIDTH) - (200. - half)).abs() < 1e-3);
	}

	#[test]
	fn pointer_maps_back_to_the_progress_under_the_thumb() {
		for step in 0..=10 {
			let progress = step as f32 / 10.;
			let center = thumb_center(progress, 200., THUMB_WIDTH);
			assert!((thumb_progress(center, 200., THUMB_WIDTH) - progress).abs() < 1e-5);
		}
		assert_eq!(thumb_progress(0., 200., THUMB_WIDTH), 0.);
		assert_eq!(thumb_progress(200., 200., THUMB_WIDTH), 1.);
		assert_eq!(thumb_progress(5., 10., THUMB_WIDTH), 0.);
	}
}
