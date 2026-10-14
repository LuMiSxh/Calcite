// SPDX-License-Identifier: MPL-2.0

//! The preview caches each section as a view. These tests pin down what that relies on:
//! keyed element state survives the frames in which the cache is reused, a notification
//! re-renders only the notified section, a window refresh (a theme change) re-renders every
//! section, and a looping animation redraws only the section that shows it.
//! The window frame relies on the same: a keyed state's notification repaints its own view
//! without a window refresh.

use std::{
	cell::{Cell, RefCell},
	rc::Rc,
	time::Duration,
};

use calcite::{continuous::ContinuousMotion, prelude::*};
use gpui::{
	AppContext as _, Context, Entity, IntoElement, ParentElement, Render, StyleRefinement, Styled,
	TestAppContext, Window, WindowHandle, div, px,
};

#[derive(Clone, Default)]
struct Counts {
	renders: Rc<Cell<usize>>,
	inits: Rc<Cell<usize>>,
	/// The section's keyed state, handed out so a test can notify it.
	state: Rc<RefCell<Option<Entity<()>>>>,
}

struct Section {
	counts: Counts,
	looping: bool,
}

impl Render for Section {
	fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
		self.counts.renders.set(self.counts.renders.get() + 1);
		let inits = self.counts.inits.clone();
		let state = window.use_keyed_state("state", cx, move |_, _| inits.set(inits.get() + 1));
		*self.counts.state.borrow_mut() = Some(state);
		if self.looping {
			ContinuousMotion::phase("test-loop", 1., &cx.theme(), window, cx);
		}
		div().size(px(40.))
	}
}

struct Host(Vec<Entity<Section>>);

impl Render for Host {
	fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
		div().size(px(100.)).children(self.0.iter().map(|section| {
			section
				.clone()
				.cached(StyleRefinement::default().w_full().h(px(40.)))
		}))
	}
}

fn draw(window: WindowHandle<Host>, cx: &mut TestAppContext) {
	cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
		.unwrap();
}

fn host(
	looping: [bool; 2],
	cx: &mut TestAppContext,
) -> (WindowHandle<Host>, [Entity<Section>; 2], [Counts; 2]) {
	cx.update(calcite::init);
	let counts = [Counts::default(), Counts::default()];
	let sections: [Entity<Section>; 2] = std::array::from_fn(|index| {
		let counts = counts[index].clone();
		cx.new(move |_| Section {
			counts,
			looping: looping[index],
		})
	});
	let window = cx.add_window({
		let sections = sections.to_vec();
		move |_, _| Host(sections)
	});
	cx.update_window(window.into(), |_, window, _| window.activate())
		.unwrap();
	cx.run_until_parked();
	draw(window, cx);
	(window, sections, counts)
}

#[gpui::test]
fn keyed_state_survives_cache_hits(cx: &mut TestAppContext) {
	let (window, [section, _], [counts, _]) = host([false, false], cx);
	let first = counts.renders.get();
	assert!(first >= 1);
	assert_eq!(counts.inits.get(), 1);

	// Nothing is dirty: the section is reused and does not render.
	draw(window, cx);
	draw(window, cx);
	assert_eq!(counts.renders.get(), first, "the cache was not used");

	// A notified section renders again and still finds its state.
	section.update(cx, |_, cx| cx.notify());
	draw(window, cx);
	assert!(counts.renders.get() > first);
	assert_eq!(counts.inits.get(), 1, "keyed state was lost across a cache hit");
}

#[gpui::test]
fn a_notification_renders_only_its_section(cx: &mut TestAppContext) {
	let (window, [first, _], [a, b]) = host([false, false], cx);
	let (a0, b0) = (a.renders.get(), b.renders.get());
	first.update(cx, |_, cx| cx.notify());
	draw(window, cx);
	assert!(a.renders.get() > a0);
	assert_eq!(b.renders.get(), b0, "a sibling section rendered too");
}

#[gpui::test]
fn a_window_refresh_renders_every_section(cx: &mut TestAppContext) {
	let (window, _, [a, b]) = host([false, false], cx);
	let (a0, b0) = (a.renders.get(), b.renders.get());
	// A theme change refreshes the windows, which bypasses every cache.
	cx.update(|cx| Theme::update(cx, |theme| theme.reduce_motion = !theme.reduce_motion));
	draw(window, cx);
	assert!(
		a.renders.get() > a0 && b.renders.get() > b0,
		"a cached section ignored the refresh"
	);
}

#[gpui::test]
fn a_loop_redraws_only_the_section_that_shows_it(cx: &mut TestAppContext) {
	let (window, _, [looping, still]) = host([true, false], cx);
	let (l0, s0) = (looping.renders.get(), still.renders.get());
	for _ in 0..3 {
		std::thread::sleep(Duration::from_millis(50));
		cx.update_window(window.into(), |_, window, cx| {
			window.simulate_next_frame(cx);
		})
		.unwrap();
		draw(window, cx);
	}
	assert!(looping.renders.get() > l0, "the loop did not advance");
	assert_eq!(
		still.renders.get(),
		s0,
		"a loop redrew a section that does not show it"
	);
}

#[gpui::test]
fn a_keyed_state_notification_renders_only_its_section_without_a_refresh(cx: &mut TestAppContext) {
	let (window, _, [a, b]) = host([false, false], cx);
	let (a0, b0) = (a.renders.get(), b.renders.get());
	// What the window frame does when the hovered resize edge changes: update its keyed state.
	let state = a.state.borrow().clone().expect("the section rendered");
	state.update(cx, |_, cx| cx.notify());
	draw(window, cx);
	assert!(
		a.renders.get() > a0,
		"the notified state did not repaint its view"
	);
	assert_eq!(b.renders.get(), b0, "a sibling section rendered too");
}
