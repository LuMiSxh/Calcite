// SPDX-License-Identifier: MPL-2.0
use std::time::Duration;

use gpui::{
	Bounds, BoxShadow, ElementId, Entity, FocusHandle, InteractiveElement, IntoElement, MouseButton,
	ParentElement, Pixels, Point, Styled, Window, div, point, px,
};

use crate::{
	motion::{
		Spring,
		tokens::{JELLY, PRESS},
		value::{SpringState, advance},
	},
	theme::Theme,
	tokens::{self, control},
};

use super::{
	focus::{FOCUS_OUTSET, FOCUS_RING_OFFSET, FOCUS_RING_WIDTH},
	scale::{is_unscaled, scale_about, unscale_about},
	state::KEY_PRESS_PULSE,
};

use super::*;
use gpui::{Context, Render, TestAppContext, VisualTestContext};
use std::{
	cell::{Cell, RefCell},
	rc::Rc,
};

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
