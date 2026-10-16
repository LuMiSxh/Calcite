// SPDX-License-Identifier: MPL-2.0

//! Keyboard behavior across components: Escape leaves a focused text field alone, a modal keeps
//! Tab inside itself and gives focus back when it leaves, and a menu opened from the keyboard
//! takes arrow keys, Home and End, type-ahead, Return, and Escape.

use std::{cell::Cell, cell::RefCell, rc::Rc};

use calcite::{EditableTextState, prelude::*};
use gpui::{
	AnyWindowHandle, Context, Entity, IntoElement, ParentElement, Render, Styled, TestAppContext, Window,
	WindowHandle, div, px,
};

fn draw<V: Render>(window: WindowHandle<V>, cx: &mut TestAppContext) {
	// Notify the root as the app's own state changes would; a frame then renders the whole tree.
	window.update(cx, |_, _, cx| cx.notify()).unwrap();
	cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
		.unwrap();
}

/// Lets a spring-driven surface run for real time, drawing as the platform would.
fn settle<V: Render>(window: WindowHandle<V>, cx: &mut TestAppContext) {
	for _ in 0..40 {
		std::thread::sleep(std::time::Duration::from_millis(8));
		draw(window, cx);
	}
	cx.run_until_parked();
	draw(window, cx);
}

/// One key, then the frame it causes (and the deferred focus moves that follow it).
fn press<V: Render>(window: WindowHandle<V>, cx: &mut TestAppContext, keys: &str) {
	let any: AnyWindowHandle = window.into();
	cx.simulate_keystrokes(any, keys);
	draw(window, cx);
	// Space and Return click a focused control when released; the helper sends no key up.
	if matches!(keys, "space" | "enter") {
		let mut visual = gpui::VisualTestContext::from_window(any, cx);
		visual.simulate_event(gpui::KeyUpEvent {
			keystroke: gpui::Keystroke::parse(keys).unwrap(),
		});
		draw(window, cx);
	}
	cx.run_until_parked();
	draw(window, cx);
}

fn open_window<V: Render + 'static>(
	cx: &mut TestAppContext,
	build: impl FnOnce(&mut Window, &mut Context<V>) -> V,
) -> WindowHandle<V> {
	cx.update(calcite::init);
	let window = cx.add_window(build);
	cx.update_window(window.into(), |_, window, _| window.activate())
		.unwrap();
	cx.run_until_parked();
	draw(window, cx);
	window
}

// ---- Escape and text fields -------------------------------------------------------------

struct Sheet {
	shown: bool,
	field: Entity<EditableTextState>,
	dismissed: Rc<Cell<usize>>,
	clicks: Rc<RefCell<Vec<&'static str>>>,
}

impl Render for Sheet {
	fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
		let note = |name: &'static str| {
			let clicks = self.clicks.clone();
			move |_: &gpui::ClickEvent, _: &mut Window, _: &mut gpui::App| clicks.borrow_mut().push(name)
		};
		let dismissed = self.dismissed.clone();
		div()
			.size(px(900.))
			.flex()
			.flex_col()
			.child(button("outside", "Outside").on_click(note("outside")))
			.child(
				modal_layer(
					"layer",
					modal("sheet", "Sheet")
						.child(text_field("name", &self.field).width(px(200.)))
						.action(button("a", "A").on_click(note("a")))
						.action(button("b", "B").on_click(note("b"))),
				)
				.shown(self.shown)
				.on_dismiss(move |_, _, _| dismissed.set(dismissed.get() + 1)),
			)
			.child(button("after", "After").on_click(note("after")))
	}
}

type Clicks = Rc<RefCell<Vec<&'static str>>>;

fn sheet(shown: bool, cx: &mut TestAppContext) -> (WindowHandle<Sheet>, Rc<Cell<usize>>, Clicks) {
	let dismissed = Rc::new(Cell::new(0));
	let clicks = Rc::new(RefCell::new(Vec::new()));
	let window = {
		let (dismissed, clicks) = (dismissed.clone(), clicks.clone());
		cx.update(calcite::init);
		let field = cx.update(|cx| text_state("", cx));
		open_window(cx, move |_, _| Sheet {
			shown,
			field,
			dismissed,
			clicks,
		})
	};
	(window, dismissed, clicks)
}

#[gpui::test]
fn escape_in_a_text_field_blurs_it_and_does_not_dismiss(cx: &mut TestAppContext) {
	let (window, dismissed, _) = sheet(true, cx);
	press(window, cx, "tab");
	press(window, cx, "escape");
	assert_eq!(
		dismissed.get(),
		0,
		"Escape in a focused text field dismissed the modal"
	);
	let focused = cx
		.update_window(window.into(), |_, window, cx| window.focused(cx).is_some())
		.unwrap();
	assert!(!focused, "the text field's own Escape should have blurred it");
	press(window, cx, "escape");
	assert_eq!(dismissed.get(), 1, "the next Escape should dismiss");
}

#[gpui::test]
fn tab_stays_inside_a_modal_and_focus_returns_to_the_opener(cx: &mut TestAppContext) {
	let (window, _, clicks) = sheet(false, cx);
	// Focus the page's first control, then raise the modal over it.
	press(window, cx, "tab");
	window
		.update(cx, |sheet, _, cx| {
			sheet.shown = true;
			cx.notify();
		})
		.unwrap();
	draw(window, cx);
	cx.run_until_parked();
	draw(window, cx);

	// Field, A, B, then around to the field: never the buttons behind the modal.
	for _ in 0..4 {
		press(window, cx, "tab");
	}
	press(window, cx, "shift-tab");
	press(window, cx, "shift-tab");
	press(window, cx, "space");
	assert!(
		clicks.borrow().iter().all(|name| matches!(*name, "a" | "b")),
		"Tab reached the page behind the modal: {:?}",
		clicks.borrow()
	);
	assert!(!clicks.borrow().is_empty(), "no modal control had focus");

	// Hiding the layer returns focus to the control that had it.
	clicks.borrow_mut().clear();
	window
		.update(cx, |sheet, _, cx| {
			sheet.shown = false;
			cx.notify();
		})
		.unwrap();
	draw(window, cx);
	cx.run_until_parked();
	draw(window, cx);
	press(window, cx, "space");
	assert_eq!(
		*clicks.borrow(),
		["outside"],
		"focus did not return to the opener"
	);
}

// ---- Menus ------------------------------------------------------------------------------

/// A modal whose only tab stop is a text field, over a page with a button after it.
struct Solo {
	field: Entity<EditableTextState>,
}

impl Render for Solo {
	fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
		div()
			.size(px(900.))
			.flex()
			.flex_col()
			.child(button("outside", "Outside"))
			.child(modal_layer(
				"layer",
				modal("sheet", "Sheet").child(text_field("name", &self.field).width(px(200.))),
			))
			.child(button("after", "After"))
	}
}

#[gpui::test]
fn tab_in_a_text_field_inside_a_modal_stays_in_the_modal(cx: &mut TestAppContext) {
	cx.update(calcite::init);
	let field = cx.update(|cx| text_state("", cx));
	let window = open_window(cx, {
		let field = field.clone();
		move |_, _| Solo { field }
	});
	let in_field = |cx: &mut TestAppContext| {
		let field = field.clone();
		cx.update_window(window.into(), move |_, window, cx| {
			field.read(cx).focus_handle(cx).is_focused(window)
		})
		.unwrap()
	};
	press(window, cx, "tab");
	assert!(in_field(cx), "Tab from the layer should land on the field");
	// The field is the modal's only tab stop: Tab must wrap onto itself, not leave for the page.
	press(window, cx, "tab");
	assert!(in_field(cx), "Tab in the field left the modal");
	press(window, cx, "shift-tab");
	assert!(in_field(cx), "Shift-Tab in the field left the modal");
}

struct Host {
	chosen: Rc<RefCell<Vec<&'static str>>>,
}

impl Render for Host {
	fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
		let pick = |name: &'static str| {
			let chosen = self.chosen.clone();
			move |_: &mut Window, _: &mut gpui::App| chosen.borrow_mut().push(name)
		};
		div().size(px(600.)).p(px(40.)).child(
			action_menu("menu", "Menu", Icon::Document)
				.item("Alpha", Icon::Document, pick("Alpha"))
				.item("Beta", Icon::Document, pick("Beta"))
				.item("Gamma", Icon::Document, pick("Gamma")),
		)
	}
}

fn menu_window(cx: &mut TestAppContext) -> (WindowHandle<Host>, Rc<RefCell<Vec<&'static str>>>) {
	let chosen = Rc::new(RefCell::new(Vec::new()));
	let window = open_window(cx, {
		let chosen = chosen.clone();
		move |_, _| Host { chosen }
	});
	(window, chosen)
}

#[gpui::test]
fn a_menu_opened_by_the_keyboard_navigates_and_chooses_with_the_keyboard(cx: &mut TestAppContext) {
	let (window, chosen) = menu_window(cx);
	press(window, cx, "tab");
	press(window, cx, "down");
	settle(window, cx);
	// Focus is on the first row: End and a letter move it, Return chooses.
	press(window, cx, "end");
	press(window, cx, "home");
	press(window, cx, "down");
	press(window, cx, "up");
	press(window, cx, "down");
	press(window, cx, "down");
	press(window, cx, "down");
	// Wrapped from Gamma back to Alpha; "b" then goes to Beta.
	press(window, cx, "b");
	press(window, cx, "enter");
	assert_eq!(*chosen.borrow(), ["Beta"]);
}

#[gpui::test]
fn escape_closes_a_menu_and_returns_focus_to_its_chip(cx: &mut TestAppContext) {
	let (window, chosen) = menu_window(cx);
	press(window, cx, "tab");
	press(window, cx, "space");
	settle(window, cx);
	press(window, cx, "down");
	press(window, cx, "escape");
	settle(window, cx);
	assert!(chosen.borrow().is_empty());
	// The chip has focus again: Space opens the menu and Return chooses its first row.
	press(window, cx, "space");
	settle(window, cx);
	press(window, cx, "enter");
	assert_eq!(*chosen.borrow(), ["Alpha"]);
}

struct AfterMenu {
	chosen: Rc<RefCell<Vec<&'static str>>>,
}

impl Render for AfterMenu {
	fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
		let chosen = self.chosen.clone();
		div()
			.size(px(600.))
			.p(px(40.))
			.flex()
			.flex_col()
			.gap(px(200.))
			.child(
				action_menu("menu", "Menu", Icon::Document)
					.item("Alpha", Icon::Document, move |_, _| {
						chosen.borrow_mut().push("Alpha")
					})
					.item("Beta", Icon::Document, |_, _| {}),
			)
			.child(button("after", "After"))
	}
}

#[gpui::test]
fn tab_out_of_an_open_menu_closes_it(cx: &mut TestAppContext) {
	let chosen = Rc::new(RefCell::new(Vec::new()));
	let window = open_window(cx, {
		let chosen = chosen.clone();
		move |_, _| AfterMenu { chosen }
	});
	press(window, cx, "tab");
	press(window, cx, "down");
	settle(window, cx);
	// The tab order is chip, page button, rows: Alpha, Beta, around to the chip, then out to "After".
	press(window, cx, "tab");
	press(window, cx, "tab");
	press(window, cx, "tab");
	settle(window, cx);
	// With the menu closed, the previous tab stop from "After" is the chip, not a row.
	press(window, cx, "shift-tab");
	press(window, cx, "enter");
	settle(window, cx);
	press(window, cx, "enter");
	assert_eq!(
		*chosen.borrow(),
		["Alpha"],
		"the menu stayed open after Tab left it"
	);
}
