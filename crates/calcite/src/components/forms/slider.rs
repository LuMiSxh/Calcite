// SPDX-License-Identifier: MPL-2.0

//! The value slider: [`slider`] (`SettingsSlider`). The arrow keys step it, and Home, End,
//! PageUp, and PageDown jump to either end or a page.

use std::ops::RangeInclusive;

use gpui::{
	App, Bounds, DispatchPhase, ElementId, IntoElement, MouseButton, ParentElement, Pixels, Role,
	SharedString, Styled, Window, canvas, div, prelude::*, px,
};

use super::SharedChangeHandler;
use crate::{
	controls::theme_override,
	icons::IconRef,
	interaction::{FocusShape, KeyboardFocus, focus_handle, key_step},
	knob::{LENS_GROWTH, SliderPress, knob_surface, lens_amount, release_press},
	motion::{self, tokens::SMOOTH},
	theme::{ActiveTheme, Theme},
	tokens::{self, control, scale_alpha},
};

theme_override!(Slider);

/// The slider thumb: 16 pt tall, and 1.45 times as wide, a capsule like the macOS 26 slider knob.
const THUMB_HEIGHT: f32 = 16.;

const THUMB_WIDTH: f32 = THUMB_HEIGHT * 1.45;

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

	/// Whether the control responds. Enabled by default.
	pub fn enabled(mut self, enabled: bool) -> Self {
		self.enabled = enabled;
		self
	}

	/// The inverse of `enabled`.
	pub fn disabled(self, disabled: bool) -> Self {
		self.enabled(!disabled)
	}

	pub fn on_change(mut self, handler: impl Fn(&f32, &mut Window, &mut App) + 'static) -> Self {
		self.on_change = Some(std::rc::Rc::new(handler));
		self
	}
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
		// The laid-out width wins once known: in a row with side items the track yields to them
		// instead of overlapping, and the thumb math follows what is actually drawn.
		let track_width = laid_out_width(
			bounds.read(cx).map(|area| f32::from(area.size.width)),
			f32::from(self.width),
		);
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
		// The drawn value glides to the target on SMOOTH, so a track click or an arrow key moves the
		// thumb smoothly. A held thumb is the exception: it sits exactly under the pointer.
		let glide = motion::spring(
			(self.id.clone(), "calcite-slider-fraction"),
			fraction,
			SMOOTH,
			reduce_motion,
			window,
			cx,
		)
		.value
		.clamp(0., 1.);
		let drawn = if held { fraction } else { glide };
		let lens = lens_amount((self.id.clone(), "lens"), held, reduce_motion, window, cx);
		// The taller growth scales both sides, so the lens keeps the capsule's aspect.
		let grow = 1. + LENS_GROWTH.1 * lens;
		let (grown_width, grown_height) = (THUMB_WIDTH * grow, THUMB_HEIGHT * grow);
		let center = thumb_center(drawn, track_width, THUMB_WIDTH);
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
			.flex_shrink(1.)
			.min_w_0()
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
					.bg(theme.ink(0.18 * alpha)),
			)
			.child(
				div()
					.absolute()
					.left_0()
					.top(px(THUMB_HEIGHT / 2.))
					.w(px(thumb_center(drawn, track_width, THUMB_WIDTH)))
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
					if event.keystroke.modifiers.modified() {
						return;
					}
					if let Some(next) = key_value(&event.keystroke.key, *value, range, step) {
						cx.stop_propagation();
						on_change(&next, window, cx);
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
			return slider.flex_none().into_any_element();
		}
		// Side items center on the track, not on the track and its ticks.
		let beside = || div().h(px(track_height)).flex().items_center().flex_none();
		div()
			.flex()
			.flex_row()
			.font_family(theme.sans())
			.items_start()
			.gap(tokens::space::CONTROL)
			.max_w_full()
			.when_some(self.leading_icon, |row, icon| {
				row.child(beside().child(icon.render(theme.text_body(), secondary)))
			})
			.child(slider)
			.when_some(self.trailing_icon, |row, icon| {
				row.child(beside().child(icon.render(theme.text_body(), secondary)))
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
						.text_size(theme.text_caption())
						.text_color(secondary)
						.child(format!("{percent}%")),
				)
			})
			.into_any_element()
	}
}

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

/// The value a key press moves a slider to, or `None` for a key the slider does not use. Arrows
/// move one step, PageUp and PageDown ten steps (a tenth of the range without a step), and Home
/// and End go to the ends of the range.
fn key_value(key: &str, value: f32, range: &RangeInclusive<f32>, step: Option<f32>) -> Option<f32> {
	let (start, end) = (*range.start(), *range.end());
	let page = match step.filter(|step| *step > 0.) {
		Some(step) => 10. * step,
		None => (end - start).abs() * 0.1,
	};
	match key {
		"home" => Some(snap(start, range, step)),
		"end" => Some(snap(end, range, step)),
		"left" | "down" => Some(moved_value(value, -1., key_step(range, step), range, step)),
		"right" | "up" => Some(moved_value(value, 1., key_step(range, step), range, step)),
		"pagedown" => Some(moved_value(value, -1., page, range, step)),
		"pageup" => Some(moved_value(value, 1., page, range, step)),
		_ => None,
	}
}

/// `value` moved `direction` (plus or minus 1) times `distance` along `range`, then snapped.
fn moved_value(
	value: f32,
	direction: f32,
	distance: f32,
	range: &RangeInclusive<f32>,
	step: Option<f32>,
) -> f32 {
	let sign = if range.end() >= range.start() { 1. } else { -1. };
	snap(value + direction * sign * distance, range, step)
}

/// The slider track's width: what layout gave it once measured, else the width it asked for.
fn laid_out_width(measured: Option<f32>, requested: f32) -> f32 {
	measured.filter(|width| *width > 0.).unwrap_or(requested)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn keys_step_page_and_jump_the_slider() {
		let range = 0.0..=1.0;
		let moved = |key: &str, step: Option<f32>| key_value(key, 0.5, &range, step).unwrap();
		assert!((moved("right", Some(0.25)) - 0.75).abs() < 1e-6);
		assert!((moved("left", Some(0.25)) - 0.25).abs() < 1e-6);
		assert!((moved("pageup", None) - 0.6).abs() < 1e-6);
		assert!((moved("pagedown", Some(0.05))).abs() < 1e-6);
		assert_eq!(moved("home", None), 0.);
		assert_eq!(moved("end", None), 1.);
		assert_eq!(key_value("tab", 0.5, &range, None), None);
	}

	#[test]
	fn the_track_follows_its_laid_out_width() {
		assert_eq!(laid_out_width(None, 200.), 200.);
		assert_eq!(laid_out_width(Some(0.), 200.), 200.);
		assert_eq!(laid_out_width(Some(150.), 200.), 150.);
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
