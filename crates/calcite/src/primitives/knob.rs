// SPDX-License-Identifier: MPL-2.0

//! The knob shared by the switch and the slider: the pressed glass lens, the drag gesture, and
//! the press state.

use gpui::{App, Div, ElementId, Entity, Filter, GlassFilter, Styled, Window, px};

use crate::{
	interaction::spring_value,
	motion::Curve,
	theme::Theme,
	tokens::{self, scale_alpha, white},
};

/// How far the pressed lens grows past the thumb, horizontally and vertically.
pub(crate) const LENS_GROWTH: (f32, f32) = (0.42, 0.55);

/// Pointer travel, in pixels, a press may make before it counts as a drag rather than a click.
pub(crate) const DRAG_SLOP: f32 = 3.;

/// Whether the pointer at `x` has moved far enough from a press at `press_x` to drag the knob.
pub(crate) fn is_drag(press_x: f32, x: f32) -> bool {
	(x - press_x).abs() > DRAG_SLOP
}

/// The knob progress (0 off, 1 on) under a pointer at `x`. The press at `press_x` began with the
/// knob at `start`, and `travel` is the knob's distance across the track. The result stays on the track.
pub(crate) fn drag_progress(start: f32, press_x: f32, x: f32, travel: f32) -> f32 {
	if travel <= 0. {
		return start.clamp(0., 1.);
	}
	(start + (x - press_x) / travel).clamp(0., 1.)
}

/// Whether a released switch ends on. A drag settles to the nearer side, past the midpoint is on;
/// a plain press (`None`) flips the value.
pub(crate) fn release_on(on: bool, progress: Option<f32>) -> bool {
	match progress {
		Some(progress) => progress > 0.5,
		None => !on,
	}
}

/// The lens amount a pressed knob turns into: 1 while pressed, and 0 at rest or under Reduce Motion,
/// which keeps the knob solid.
pub(crate) fn lens_target(pressed: bool, reduce_motion: bool) -> f32 {
	if pressed && !reduce_motion { 1. } else { 0. }
}

/// How far a knob has turned into its glass lens, springing in on [`Curve::Press`] and back out on
/// [`Curve::Release`]. The switch knob and the slider thumb share it.
pub(crate) fn lens_amount(
	id: impl Into<ElementId>,
	pressed: bool,
	reduce_motion: bool,
	window: &mut Window,
	cx: &mut App,
) -> f32 {
	let curve = if pressed { Curve::Press } else { Curve::Release };
	spring_value(
		id,
		lens_target(pressed, reduce_motion),
		curve,
		reduce_motion,
		window,
		cx,
	)
	.max(0.)
}

/// Paints a knob `height` tall: the solid white thumb at rest, and as `lens` rises, the clear glass
/// lens it turns into while pressed. Reduce Transparency keeps the solid thumb. `rest_shadow` is the
/// opacity of the resting shadow.
pub(crate) fn knob_surface(
	knob: Div,
	lens: f32,
	height: f32,
	alpha: f32,
	rest_shadow: f32,
	theme: &Theme,
) -> Div {
	if lens > 0.01 && !theme.reduce_transparency() {
		// A clear lens: the solid thumb fades out while the glass bends the track below.
		let fill = white(0.06 + 0.94 * (1. - lens).powi(2));
		knob.backdrop_filter(vec![Filter::Glass(GlassFilter {
			blur: px(0.6),
			bezel: px(height * 0.38),
			refraction: px(height * 0.22 * lens),
			dispersion: 0.3,
			saturation: 1.25,
			brightness: 1.08,
			fill: [fill.red, fill.green, fill.blue, fill.alpha],
			rim: 0.9,
			specular: 0.85,
			..GlassFilter::default()
		})])
		.shadow(vec![
			gpui::BoxShadow::new(px(0.), px(2.), tokens::black(0.22 * lens)).blur_radius(px(8.)),
		])
	} else {
		knob.bg(scale_alpha(white(1.), alpha)).shadow(vec![
			gpui::BoxShadow::new(px(0.), px(1.), tokens::black(rest_shadow)).blur_radius(px(3.)),
		])
	}
}

/// Ends a slider press, redrawing the thumb once it lets go.
pub(crate) fn release_press(press: &Entity<SliderPress>, cx: &mut App) {
	press.update(cx, |state, cx| {
		if state.release() {
			cx.notify();
		}
	});
}

/// Whether a slider's thumb is held. A press holds it until the button is released, even while the
/// pointer strays off the track.
#[derive(Default)]
pub(crate) struct SliderPress {
	pub(crate) held: bool,
}

impl SliderPress {
	pub(crate) fn hold(&mut self) {
		self.held = true;
	}

	/// Lets go of the thumb. Returns whether it was held.
	pub(crate) fn release(&mut self) -> bool {
		std::mem::take(&mut self.held)
	}
}

/// The pointer gesture on a switch: where the press began, and the knob progress once the pointer
/// has dragged it past [`DRAG_SLOP`].
#[derive(Default)]
pub(crate) struct ToggleDrag {
	/// Pointer x and knob progress when the press began.
	press: Option<(f32, f32)>,
	/// Knob progress under the pointer, or `None` while the press has not dragged yet.
	pub(crate) progress: Option<f32>,
}

impl ToggleDrag {
	pub(crate) fn begin(&mut self, x: f32, progress: f32) {
		self.press = Some((x, progress));
		self.progress = None;
	}

	/// Follows the pointer. Returns whether the knob moved.
	pub(crate) fn move_to(&mut self, x: f32, travel: f32) -> bool {
		let Some((press_x, start)) = self.press else {
			return false;
		};
		if self.progress.is_none() && !is_drag(press_x, x) {
			return false;
		}
		let next = Some(drag_progress(start, press_x, x, travel));
		let changed = next != self.progress;
		self.progress = next;
		changed
	}

	/// Ends the press. `None` when no press was under way; otherwise the progress the drag reached,
	/// which is `None` for a plain press.
	pub(crate) fn end(&mut self) -> Option<Option<f32>> {
		self.press.take()?;
		Some(self.progress.take())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn drag_moves_the_knob_with_the_pointer_and_clamps_to_the_track() {
		assert!((drag_progress(0., 100., 110., 40.) - 0.25).abs() < 1e-6);
		assert!((drag_progress(0.5, 100., 120., 40.) - 1.).abs() < 1e-6);
		assert!((drag_progress(0.5, 100., 80., 40.) - 0.).abs() < 1e-6);
		assert_eq!(drag_progress(1., 100., 500., 40.), 1.);
		assert_eq!(drag_progress(0.5, 100., 300., 0.), 0.5);
	}

	#[test]
	fn release_settles_to_the_nearer_side() {
		assert!(release_on(false, Some(0.51)));
		assert!(!release_on(true, Some(0.49)));
		assert!(!release_on(false, Some(0.5)));
		assert!(!release_on(true, Some(0.2)));
	}

	#[test]
	fn plain_press_flips_the_value() {
		assert!(release_on(false, None));
		assert!(!release_on(true, None));
	}

	#[test]
	fn small_movement_is_still_a_click() {
		assert!(!is_drag(10., 13.));
		assert!(is_drag(10., 13.5));
		assert!(is_drag(10., 4.));
	}

	#[test]
	fn gesture_reports_its_progress_on_release() {
		let mut click = ToggleDrag::default();
		click.begin(100., 0.);
		assert!(!click.move_to(101., 40.));
		assert_eq!(click.end(), Some(None));
		assert_eq!(click.end(), None);

		let mut drag = ToggleDrag::default();
		drag.begin(100., 0.);
		assert!(drag.move_to(110., 40.));
		assert!(!drag.move_to(110., 40.));
		assert_eq!(drag.end(), Some(Some(0.25)));
	}

	#[test]
	fn lens_turns_on_only_while_pressed_and_not_under_reduce_motion() {
		assert_eq!(lens_target(true, false), 1.);
		assert_eq!(lens_target(false, false), 0.);
		assert_eq!(lens_target(true, true), 0.);
	}

	#[test]
	fn slider_press_holds_until_one_release() {
		let mut press = SliderPress::default();
		assert!(!press.release());
		press.hold();
		assert!(press.release());
		assert!(!press.release());
	}
}
