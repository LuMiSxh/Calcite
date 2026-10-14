// SPDX-License-Identifier: MPL-2.0

//! The navigation rail's keyboard behavior: one tab stop on the selected entry, and Up and Down
//! that stop at the ends of the rail instead of leaving it. The focused entry is found by pressing
//! Space, which clicks it.

use std::{cell::RefCell, rc::Rc};

use calcite::prelude::*;
use gpui::{
	AnyWindowHandle, AppContext as _, ClickEvent, Context, IntoElement, ParentElement, Render, Styled,
	TestAppContext, Window, WindowHandle, div, px,
};

struct Rails {
	clicks: Rc<RefCell<Vec<&'static str>>>,
}

impl Render for Rails {
	fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
		let note = |name: &'static str| {
			let clicks = self.clicks.clone();
			move |_: &ClickEvent, _: &mut Window, _: &mut gpui::App| clicks.borrow_mut().push(name)
		};
		div()
			.size(px(600.))
			.flex()
			.flex_row()
			.child(
				nav_rail("rail-a")
					.child(nav_item("a-0", "First", Icon::Gear).on_click(note("a-0")))
					.child(
						nav_item("a-1", "Second", Icon::Gear)
							.selected(true)
							.on_click(note("a-1")),
					)
					.child(nav_item("a-2", "Third", Icon::Gear).on_click(note("a-2"))),
			)
			.child(
				nav_rail("rail-b")
					.child(
						nav_item("b-0", "First", Icon::Gear)
							.selected(true)
							.on_click(note("b-0")),
					)
					.child(nav_item("b-1", "Second", Icon::Gear).on_click(note("b-1"))),
			)
			.child(button("after", "After").on_click(note("after")))
	}
}

fn draw(window: WindowHandle<Rails>, cx: &mut TestAppContext) {
	window.update(cx, |_, _, cx| cx.notify()).unwrap();
	cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
		.unwrap();
}

fn press(window: WindowHandle<Rails>, cx: &mut TestAppContext, keys: &str) {
	let any: AnyWindowHandle = window.into();
	cx.simulate_keystrokes(any, keys);
	draw(window, cx);
	if keys == "space" {
		// Space clicks on release; the helper sends no key up.
		let mut visual = gpui::VisualTestContext::from_window(any, cx);
		visual.simulate_event(gpui::KeyUpEvent {
			keystroke: gpui::Keystroke::parse(keys).unwrap(),
		});
		draw(window, cx);
	}
	cx.run_until_parked();
	draw(window, cx);
}

/// The entry that has focus: Space clicks it.
fn focused(
	window: WindowHandle<Rails>,
	clicks: &Rc<RefCell<Vec<&'static str>>>,
	cx: &mut TestAppContext,
) -> Option<&'static str> {
	clicks.borrow_mut().clear();
	press(window, cx, "space");
	clicks.borrow().last().copied()
}

#[gpui::test]
fn tab_enters_on_the_selected_entry_and_arrows_stay_inside(cx: &mut TestAppContext) {
	cx.update(calcite::init);
	let clicks = Rc::new(RefCell::new(Vec::new()));
	let window = cx.add_window({
		let clicks = clicks.clone();
		move |_, _| Rails { clicks }
	});
	cx.update_window(window.into(), |_, window, _| window.activate())
		.unwrap();
	cx.run_until_parked();
	// Two frames: the first tells the rails whether anything is selected.
	draw(window, cx);
	draw(window, cx);

	press(window, cx, "tab");
	assert_eq!(
		focused(window, &clicks, cx),
		Some("a-1"),
		"Tab lands on the selected entry"
	);

	press(window, cx, "down");
	assert_eq!(focused(window, &clicks, cx), Some("a-2"));
	press(window, cx, "down");
	assert_eq!(
		focused(window, &clicks, cx),
		Some("a-2"),
		"Down stops at the last entry"
	);

	press(window, cx, "up");
	press(window, cx, "up");
	press(window, cx, "up");
	assert_eq!(
		focused(window, &clicks, cx),
		Some("a-0"),
		"Up stops at the first entry"
	);

	press(window, cx, "down");
	press(window, cx, "tab");
	assert_eq!(
		focused(window, &clicks, cx),
		Some("b-0"),
		"Tab leaves the rail for the next rail's selection"
	);
	press(window, cx, "down");
	assert_eq!(
		focused(window, &clicks, cx),
		Some("b-1"),
		"each rail keeps its own entries"
	);
	press(window, cx, "tab");
	assert_eq!(focused(window, &clicks, cx), Some("after"));
}
