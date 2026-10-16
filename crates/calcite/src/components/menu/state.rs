// SPDX-License-Identifier: MPL-2.0

//! Menu state: the rows, the open flag, the focus handles, and the keys that move between rows.

use std::rc::Rc;

use gpui::{App, ElementId, Entity, FocusHandle, SharedString, Window};

use crate::{
	icons::IconRef,
	interaction::{KeyHandler, KeyRole, register_key_target},
};

pub(super) type Action = Rc<dyn Fn(&mut Window, &mut App) + 'static>;

pub(super) struct Item {
	pub(super) label: SharedString,
	pub(super) icon: Option<IconRef>,
	pub(super) checked: bool,
	/// Whether the row is one choice of a set (a picker option) rather than an action.
	pub(super) choice: bool,
	pub(super) divider_before: bool,
	pub(super) action: Action,
}

/// The focus handles of a menu's chip and rows, written while they render (each inside its own
/// element scope, where their keyed state lives) and read by the key handlers.
#[derive(Default)]
pub(super) struct MenuFocus {
	pub(super) trigger: Option<FocusHandle>,
	pub(super) rows: Vec<FocusHandle>,
}

/// The open state of a menu, kept across frames. While the menu is open, Escape closes it
/// before anything under it, and returns focus to the chip if a row had it.
pub(super) fn open_state(
	id: &ElementId,
	handles: &Entity<MenuFocus>,
	window: &mut Window,
	cx: &mut App,
) -> Entity<bool> {
	let open = window.use_keyed_state((id.clone(), "calcite-menu"), cx, |_, _| false);
	if *open.read(cx) {
		let close = open.clone();
		let handles = handles.clone();
		let dismiss: KeyHandler = Rc::new(move |window, cx| {
			let (trigger, in_rows) = {
				let handles = handles.read(cx);
				(
					handles.trigger.clone(),
					handles.rows.iter().any(|row| row.is_focused(window)),
				)
			};
			if let Some(trigger) = trigger.filter(|_| in_rows) {
				window.focus(&trigger, cx);
			}
			close.update(cx, |open, cx| {
				*open = false;
				cx.notify();
			})
		});
		register_key_target(id, KeyRole::Dismiss, Some(dismiss), None, window, cx);
	}
	open
}

pub(super) fn set_open(open: &Entity<bool>, value: Option<bool>, cx: &mut App) {
	open.update(cx, |open, cx| {
		*open = value.unwrap_or(!*open);
		cx.notify();
	});
}

/// Closes the open menu when focus leaves the chip and all of its rows (Tab past the last row, or
/// a click on another control). Registered once per handle, kept in keyed state beside it.
pub(super) fn close_when_focus_leaves(
	key: ElementId,
	handle: &FocusHandle,
	open: &Entity<bool>,
	menu_focus: &Entity<MenuFocus>,
	window: &mut Window,
	cx: &mut App,
) {
	window.use_keyed_state((key, "calcite-menu-leave"), cx, |window, cx| {
		let (open, menu_focus) = (open.clone(), menu_focus.clone());
		window.on_focus_out(handle, cx, move |_, window, cx| {
			if !*open.read(cx) {
				return;
			}
			// No focus at all (window deactivated, field blurred): leave the menu as it is.
			if window.focused(cx).is_none() {
				return;
			}
			let inside = {
				let focus = menu_focus.read(cx);
				focus
					.trigger
					.iter()
					.chain(focus.rows.iter())
					.any(|h| h.is_focused(window))
			};
			if !inside {
				set_open(&open, Some(false), cx);
			}
		})
	});
}

/// Asks the open menu to move focus onto a row once its rows exist (opened from the keyboard).
pub(super) fn set_pending(pending: &Entity<bool>, cx: &mut App) {
	pending.update(cx, |pending, cx| {
		*pending = true;
		cx.notify();
	});
}

/// The element id of row `index` of the menu `id`.
pub(super) fn row_id(id: &ElementId, index: usize) -> ElementId {
	(id.clone(), SharedString::from(index.to_string())).into()
}

/// The row a menu key moves focus to. `current` is the focused row, if any; `key_char` is the
/// text the key types. Arrow keys wrap, Home and End jump, and a letter selects the next row
/// (after `current`, wrapping) whose label starts with it, ignoring case.
pub(super) fn row_for_key(
	key: &str,
	key_char: Option<&str>,
	labels: &[SharedString],
	current: Option<usize>,
) -> Option<usize> {
	let count = labels.len();
	if count == 0 {
		return None;
	}
	match key {
		"down" => Some(current.map_or(0, |index| (index + 1) % count)),
		"up" => Some(current.map_or(count - 1, |index| (index + count - 1) % count)),
		"home" => Some(0),
		"end" => Some(count - 1),
		_ => {
			let typed = key_char?
				.chars()
				.next()
				.filter(|c| !c.is_whitespace() && !c.is_control())?;
			let starts = |index: &usize| {
				labels[*index]
					.chars()
					.next()
					.is_some_and(|first| first.to_lowercase().eq(typed.to_lowercase()))
			};
			let from = current.map_or(0, |index| index + 1);
			(0..count).map(|offset| (from + offset) % count).find(starts)
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn labels() -> Vec<SharedString> {
		["Alpha", "beta", "Bravo", "Gamma"]
			.map(SharedString::from)
			.to_vec()
	}

	#[test]
	fn arrows_wrap_and_start_from_the_ends() {
		let labels = labels();
		assert_eq!(row_for_key("down", None, &labels, None), Some(0));
		assert_eq!(row_for_key("up", None, &labels, None), Some(3));
		assert_eq!(row_for_key("down", None, &labels, Some(3)), Some(0));
		assert_eq!(row_for_key("up", None, &labels, Some(0)), Some(3));
		assert_eq!(row_for_key("home", None, &labels, Some(2)), Some(0));
		assert_eq!(row_for_key("end", None, &labels, Some(0)), Some(3));
		assert_eq!(row_for_key("down", None, &[], None), None);
	}

	#[test]
	fn a_letter_jumps_to_the_next_row_that_starts_with_it() {
		let labels = labels();
		assert_eq!(row_for_key("g", Some("g"), &labels, Some(0)), Some(3));
		// Case does not matter, and repeated presses cycle through the matches.
		assert_eq!(row_for_key("b", Some("b"), &labels, Some(0)), Some(1));
		assert_eq!(row_for_key("b", Some("b"), &labels, Some(1)), Some(2));
		assert_eq!(row_for_key("b", Some("b"), &labels, Some(2)), Some(1));
		assert_eq!(row_for_key("b", Some("B"), &labels, None), Some(1));
		assert_eq!(row_for_key("z", Some("z"), &labels, Some(0)), None);
		assert_eq!(row_for_key("space", Some(" "), &labels, Some(0)), None);
		assert_eq!(row_for_key("enter", None, &labels, Some(0)), None);
	}
}
