// SPDX-License-Identifier: MPL-2.0

//! The mounted behaviour: real layout, the group's reported shapes, and clicks.
use std::{
	cell::{Cell, RefCell},
	time::{Duration, Instant},
};

use gpui::{Context, Entity, Modifiers, Render, TestAppContext, VisualTestContext, point, size};

use super::*;
const TRIGGER: (f32, f32) = (120., 32.);
const BODY: (f32, f32) = (200., 100.);

#[derive(Default)]
struct Probes {
	/// The trigger's content bounds and how often it painted.
	trigger: Cell<Option<Bounds<Pixels>>>,
	painted: Cell<u32>,
	/// Every state `open` took, in order.
	history: RefCell<Vec<bool>>,
}

struct Host {
	open: bool,
	probes: Rc<Probes>,
}

impl Host {
	fn set(&mut self, open: bool, cx: &mut Context<Self>) {
		self.open = open;
		self.probes.history.borrow_mut().push(open);
		cx.notify();
	}
}

impl Render for Host {
	fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
		let probes = self.probes.clone();
		let toggle = cx.listener(|this, _, _, cx| this.set(!this.open, cx));
		let view = cx.entity();
		// A block parent with an in-flow trigger: the layout that exposes absolute-position mistakes.
		div().p(px(40.)).child(
			attached_surface("test-attached")
				.open(self.open)
				.placement(Placement::Down)
				.trigger(move |_, _, _| {
					div()
						.id("test-trigger")
						.relative()
						.w(px(TRIGGER.0))
						.h(px(TRIGGER.1))
						.child("Pick")
						.child(
							canvas(
								{
									let probes = probes.clone();
									move |bounds, _, _| probes.trigger.set(Some(bounds))
								},
								move |_, _, _, _| probes.painted.set(probes.painted.get() + 1),
							)
							.absolute()
							.top_0()
							.left_0()
							.size_full(),
						)
						.on_click(toggle)
						.into_any_element()
				})
				.body(BODY, 16., |_, _| {
					div().w(px(BODY.0)).h(px(BODY.1)).child("rows").into_any_element()
				})
				.on_dismiss(move |_, cx| view.update(cx, |this, cx| this.set(false, cx))),
		)
	}
}

fn mount(cx: &mut TestAppContext) -> (Entity<Host>, Rc<Probes>, &mut VisualTestContext) {
	cx.update(crate::init);
	let probes = Rc::new(Probes::default());
	let (view, cx) = cx.add_window_view({
		let probes = probes.clone();
		move |_, _| Host { open: false, probes }
	});
	cx.update(|window, _| window.activate());
	draw(&view, cx);
	(view, probes, cx)
}

fn draw(view: &Entity<Host>, cx: &mut VisualTestContext) {
	view.update(cx, |_, cx| cx.notify());
	cx.update(|window, cx| window.draw(cx).clear(cx));
}

fn sweep(view: &Entity<Host>, cx: &mut VisualTestContext, mut each: impl FnMut()) {
	let start = Instant::now();
	while start.elapsed() < Duration::from_millis(1200) {
		draw(view, cx);
		each();
		std::thread::sleep(Duration::from_millis(4));
	}
}

/// The surface's outer rect as last painted (the probe sits inside its 1 px border).
fn surface() -> Option<Bounds<Pixels>> {
	LAST_SURFACE.with(|last| {
		last.borrow().map(|inner| Bounds {
			origin: inner.origin - point(px(1.), px(1.)),
			size: inner.size + size(px(2.), px(2.)),
		})
	})
}

#[gpui::test]
fn trigger_stays_and_one_surface_morphs_out_of_it(cx: &mut TestAppContext) {
	let (view, probes, cx) = mount(cx);
	let rest = probes.trigger.get().expect("trigger laid out");
	assert_eq!(rest.size, size(px(TRIGGER.0), px(TRIGGER.1)));
	assert!(surface().is_none(), "no surface at rest");

	let check = |opening: bool, cx: &mut VisualTestContext| {
		let mut grew = false;
		let mut last_painted = probes.painted.get();
		let eps = px(0.01);
		sweep(&view, cx, || {
			assert_eq!(probes.trigger.get(), Some(rest), "the trigger moved");
			assert!(
				probes.painted.get() > last_painted,
				"the trigger content was not painted"
			);
			last_painted = probes.painted.get();
			if let Some(s) = surface() {
				let cap = px(CLOSE_BUMP_PX) + eps;
				assert!(s.origin.y <= rest.origin.y + eps && s.origin.x <= rest.origin.x + eps);
				assert!(s.origin.y >= rest.origin.y - cap && s.origin.x >= rest.origin.x - cap);
				assert!(s.bottom() >= rest.bottom() - eps && s.right() >= rest.right() - eps);
				assert!(s.size.width <= px(BODY.0 + 3.) && s.size.height <= px(TRIGGER.1 + BODY.1 + 3.));
				grew |= s.size.height > rest.size.height + px(1.);
			}
		});
		assert!(
			grew,
			"the surface never grew while {}",
			if opening { "opening" } else { "closing" }
		);
	};
	view.update(cx, |host, cx| host.set(true, cx));
	check(true, cx);
	let open = surface().expect("open surface");
	assert!(
		(f32::from(open.size.height) - (TRIGGER.1 + BODY.1)).abs() < 0.1,
		"{open:?}"
	);
	view.update(cx, |host, cx| host.set(false, cx));
	check(false, cx);
	assert!(surface().is_none(), "the surface leaves once settled closed");
}

#[gpui::test]
fn clicking_the_trigger_while_open_closes_without_reopening(cx: &mut TestAppContext) {
	let (view, probes, cx) = mount(cx);
	let rest = probes.trigger.get().unwrap();
	let center = point(rest.center().x, rest.center().y);
	view.update(cx, |host, cx| host.set(true, cx));
	sweep(&view, cx, || {});
	cx.simulate_click(center, Modifiers::none());
	sweep(&view, cx, || {});
	assert_eq!(*probes.history.borrow(), [true, false], "toggled more than once");
}
