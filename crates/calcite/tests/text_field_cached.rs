// SPDX-License-Identifier: MPL-2.0

//! A text field inside a cached view shows each keystroke as it is typed. The cache paints a
//! view's elements again only when the view re-renders, and an editable text's state notifies
//! only itself, so the field has to re-render its own view on each change.

use std::{cell::Cell, rc::Rc};

use calcite::{EditableTextState, prelude::*};
use gpui::{
	AnyWindowHandle, Context, Entity, EntityInputHandler as _, IntoElement, ParentElement, Render,
	StyleRefinement, Styled, TestAppContext, Window, WindowHandle, div, px,
};

struct Section {
	state: Entity<EditableTextState>,
	renders: Rc<Cell<usize>>,
}

impl Render for Section {
	fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
		self.renders.set(self.renders.get() + 1);
		div()
			.w(px(240.))
			.h(px(40.))
			.child(text_field("field", &self.state).width(px(200.)))
	}
}

struct Host(Entity<Section>);

impl Render for Host {
	fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
		div().size(px(300.)).child(
			self.0
				.clone()
				.cached(StyleRefinement::default().w_full().h(px(40.))),
		)
	}
}

fn draw(window: WindowHandle<Host>, cx: &mut TestAppContext) {
	cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
		.unwrap();
}

#[gpui::test]
fn each_keystroke_repaints_a_text_field_in_a_cached_view(cx: &mut TestAppContext) {
	cx.update(calcite::init);
	let state = cx.update(|cx| text_state("", cx));
	let renders = Rc::new(Cell::new(0));
	let section = cx.new({
		let (state, renders) = (state.clone(), renders.clone());
		move |_| Section { state, renders }
	});
	let window = cx.add_window(move |_, _| Host(section));
	let any: AnyWindowHandle = window.into();
	let focus = cx.update(|cx| state.read(cx).focus_handle(cx));
	cx.update_window(any, |_, window, cx| {
		window.activate();
		window.focus(&focus, cx);
	})
	.unwrap();
	cx.run_until_parked();
	draw(window, cx);

	for key in ["a", "b", "c"] {
		assert_redraws(cx, window, &renders, key, |cx| cx.simulate_input(any, key));
	}
	assert_eq!(cx.update(|cx| state.read(cx).as_str().to_string()), "abc");

	// Moving the caret and selecting repaint the field too.
	assert_redraws(cx, window, &renders, "left", |cx| {
		cx.simulate_keystrokes(any, "left")
	});
	assert_redraws(cx, window, &renders, "shift-left", |cx| {
		cx.simulate_keystrokes(any, "shift-left")
	});
}

#[gpui::test]
fn swapping_the_state_keeps_each_keystroke_repainting(cx: &mut TestAppContext) {
	cx.update(calcite::init);
	let first = cx.update(|cx| text_state("", cx));
	let renders = Rc::new(Cell::new(0));
	let section = cx.new({
		let (state, renders) = (first.clone(), renders.clone());
		move |_| Section { state, renders }
	});
	let window = cx.add_window({
		let section = section.clone();
		move |_, _| Host(section)
	});
	let any: AnyWindowHandle = window.into();
	cx.update_window(any, |_, window, cx| {
		window.activate();
		let focus = first.read(cx).focus_handle(cx);
		window.focus(&focus, cx);
	})
	.unwrap();
	cx.run_until_parked();
	draw(window, cx);

	// The same field id now edits a second state, as a view does when it swaps its model.
	let second = cx.update(|cx| text_state("", cx));
	section.update(cx, |section, cx| {
		section.state = second.clone();
		cx.notify();
	});
	cx.update_window(any, |_, window, cx| {
		let focus = second.read(cx).focus_handle(cx);
		window.focus(&focus, cx);
	})
	.unwrap();
	cx.run_until_parked();
	draw(window, cx);

	for key in ["a", "b"] {
		assert_redraws(cx, window, &renders, key, |cx| cx.simulate_input(any, key));
	}
	assert_eq!(cx.update(|cx| second.read(cx).as_str().to_string()), "ab");
}

/// Runs `action` and a frame, then asserts the cached view re-rendered. A cached view that
/// does not re-render keeps painting the field as it was.
fn assert_redraws(
	cx: &mut TestAppContext,
	window: WindowHandle<Host>,
	renders: &Rc<Cell<usize>>,
	what: &str,
	action: impl FnOnce(&mut TestAppContext),
) {
	let before = renders.get();
	action(cx);
	draw(window, cx);
	assert!(
		renders.get() > before,
		"{what} did not re-render the cached view, so the field still paints the old text"
	);
}

/// An input method composes text before it commits it: the underlined preedit has to show
/// while it changes, so each step repaints the cached view too. The test drives the same
/// `EntityInputHandler` calls the platform's input handler makes.
#[gpui::test]
fn ime_composition_repaints_a_text_field_in_a_cached_view(cx: &mut TestAppContext) {
	cx.update(calcite::init);
	let state = cx.update(|cx| text_state("", cx));
	let renders = Rc::new(Cell::new(0));
	let section = cx.new({
		let (state, renders) = (state.clone(), renders.clone());
		move |_| Section { state, renders }
	});
	let window = cx.add_window(move |_, _| Host(section));
	let any: AnyWindowHandle = window.into();
	let focus = cx.update(|cx| state.read(cx).focus_handle(cx));
	cx.update_window(any, |_, window, cx| {
		window.activate();
		window.focus(&focus, cx);
	})
	.unwrap();
	cx.run_until_parked();
	draw(window, cx);

	let compose = |text: &'static str, commit: bool| {
		let state = state.clone();
		move |cx: &mut TestAppContext| {
			cx.update_window(any, |_, window, cx| {
				state.update(cx, |state, cx| {
					if commit {
						state.replace_text_in_range(None, text, window, cx);
					} else {
						state.replace_and_mark_text_in_range(None, text, None, window, cx);
					}
				})
			})
			.unwrap();
		}
	};
	assert_redraws(cx, window, &renders, "composing n", compose("n", false));
	assert_redraws(cx, window, &renders, "composing ni", compose("ni", false));
	assert_redraws(cx, window, &renders, "committing", compose("你", true));
	assert_eq!(cx.update(|cx| state.read(cx).as_str().to_string()), "你");
}
