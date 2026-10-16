// SPDX-License-Identifier: MPL-2.0

//! The navigation rail: [`nav_rail`] holds [`nav_item`] entries. The arrow keys move between
//! entries, and only the selected entry is a tab stop.

use std::{cell::RefCell, rc::Rc};

use gpui::{
	AnyElement, App, ClickEvent, Div, ElementId, FocusHandle, FontWeight, IntoElement, ParentElement, Role,
	SharedString, StyleRefinement, Styled, Window, div, prelude::*, px,
};
use smallvec::SmallVec;

use crate::{
	controls::{ClickHandler, theme_override},
	icons::IconRef,
	interaction::{ClickableRow, FocusShape, row_fill},
	motion::{self, tokens::SELECT},
	theme::{ActiveTheme, Theme},
	tokens::{self, control},
};

thread_local! {
	/// The rail whose children are rendering. A rail renders before its subtree, so an item
	/// finds its own rail here.
	static CURRENT_RAIL: RefCell<Option<RailEntries>> = const { RefCell::new(None) };
}

theme_override!(NavRail, NavItem);

/// Height of one rail entry, a comfortable click and touch target.
const NAV_ROW_HEIGHT: f32 = 40.;

/// The Settings navigation rail: a tab list with a roving tab stop. Tab enters it on the
/// selected entry and leaves it; Up, Down, Home, and End move focus between the entries and stop
/// at its ends. `id` tells rails apart. Put [`nav_item`]s in it, one of them selected.
///
/// ```ignore
/// nav_rail("settings-rail")
///     .child(nav_item("general", "General", Icon::Gear).selected(true))
///     .child(nav_item("audio", "Audio", Icon::Volume))
/// ```
pub fn nav_rail(id: impl Into<ElementId>) -> NavRail {
	NavRail {
		id: id.into(),
		label: "Navigation".into(),
		base: div()
			.h_full()
			.w(px(200.))
			.flex()
			.flex_col()
			.gap(tokens::space::COMPACT)
			.p(tokens::space::CONTENT)
			.pt(px(44.))
			.border_r_1(),
		scrolls: false,
		children: SmallVec::new(),
		theme: None,
	}
}

impl NavRail {
	/// The accessible name, "Navigation" by default.
	pub fn label(mut self, label: impl Into<SharedString>) -> Self {
		self.label = label.into();
		self
	}

	/// Scrolls the entries vertically when they do not fit.
	pub fn scrollable(mut self) -> Self {
		self.scrolls = true;
		self
	}
}

impl Styled for NavRail {
	fn style(&mut self) -> &mut StyleRefinement {
		self.base.style()
	}
}

impl ParentElement for NavRail {
	fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
		self.children.extend(elements);
	}
}

impl RenderOnce for NavRail {
	fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let entries = window
			.use_keyed_state((self.id.clone(), "calcite-rail"), cx, |_, _| {
				RailEntries::default()
			})
			.read(cx)
			.clone();
		entries.begin();
		CURRENT_RAIL.with(|current| *current.borrow_mut() = Some(entries.clone()));
		let rail = self
			.base
			.id(self.id)
			.role(Role::TabList)
			.aria_label(self.label)
			.bg(theme.palette.navigation_rail)
			.border_color(theme.palette.hairline)
			.on_key_down(move |event, window, cx| {
				if event.keystroke.modifiers.modified() {
					return;
				}
				let handles: Vec<FocusHandle> = entries
					.0
					.borrow()
					.entries
					.iter()
					.map(|entry| entry.handle.clone())
					.collect();
				let Some(current) = handles.iter().position(|handle| handle.is_focused(window)) else {
					return;
				};
				let Some(target) = rail_target(event.keystroke.key.as_str(), current, handles.len()) else {
					return;
				};
				handles[target].focus(window, cx);
				cx.stop_propagation();
			});
		let rail = if self.scrolls {
			rail.overflow_y_scroll()
		} else {
			rail
		};
		rail.children(self.children)
	}
}

/// The navigation rail (see [`nav_rail`]). Style it like any GPUI element.
#[derive(IntoElement)]
pub struct NavRail {
	id: ElementId,
	label: SharedString,
	base: Div,
	scrolls: bool,
	children: SmallVec<[AnyElement; 8]>,
	theme: Option<Theme>,
}

/// One entry of a rail, in render order.
struct RailEntry {
	handle: FocusHandle,
	selected: bool,
}

/// What a rail knows about its items: the ones rendered this frame, and whether the last frame
/// had a selected one.
#[derive(Default)]
struct RailState {
	entries: Vec<RailEntry>,
	had_selected: bool,
}

impl RailEntries {
	/// Starts a frame: forgets the items and remembers whether one was selected.
	fn begin(&self) {
		let mut state = self.0.borrow_mut();
		state.had_selected = state.entries.iter().any(|entry| entry.selected);
		state.entries.clear();
	}

	/// Records an item and returns whether it is a tab stop: the selected entry, or the first
	/// one while nothing was selected, so the rail stays reachable by keyboard.
	fn register(&self, handle: &FocusHandle, selected: bool) -> bool {
		let mut state = self.0.borrow_mut();
		let first = state.entries.is_empty();
		state.entries.push(RailEntry {
			handle: handle.clone(),
			selected,
		});
		selected || (first && !state.had_selected)
	}
}

/// The rail's state, shared with its items.
#[derive(Clone, Default)]
struct RailEntries(Rc<RefCell<RailState>>);

/// Where Up, Down, Home, or End moves focus from entry `current` of `len`: it stops at the ends
/// instead of leaving the rail.
fn rail_target(key: &str, current: usize, len: usize) -> Option<usize> {
	let last = len.checked_sub(1)?;
	match key {
		"up" => Some(current.saturating_sub(1)),
		"down" => Some((current + 1).min(last)),
		"home" => Some(0),
		"end" => Some(last),
		_ => None,
	}
}

impl NavItem {
	pub fn selected(mut self, selected: bool) -> Self {
		self.selected = selected;
		self
	}

	/// Whether the entry responds. Enabled by default.
	pub fn enabled(mut self, enabled: bool) -> Self {
		self.enabled = enabled;
		self
	}

	/// The inverse of [`NavItem::enabled`].
	pub fn disabled(self, disabled: bool) -> Self {
		self.enabled(!disabled)
	}

	pub fn on_click(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
		self.on_click = Some(Box::new(handler));
		self
	}
}

impl RenderOnce for NavItem {
	fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let enabled = self.enabled;
		let row = ClickableRow::new(&self.id, &theme, window, cx);
		// The tab stop follows the selection, so the handle is flagged for this frame.
		let tab_stop = match CURRENT_RAIL.with(|rail| rail.borrow().clone()) {
			Some(rail) if enabled => rail.register(row.focus(), self.selected),
			_ => enabled,
		};
		let row = row.tab_stop(tab_stop);
		let accent = theme.legible(theme.accent);
		let foreground = if self.selected {
			accent
		} else {
			theme.palette.primary_text
		};
		let selected = self.selected;
		// The fill and the marker follow the selection instead of jumping to the new item.
		let shown = motion::spring(
			(self.id.clone(), "calcite-nav-selection"),
			if selected { 1. } else { 0. },
			SELECT,
			theme.reduce_motion,
			window,
			cx,
		)
		.value
		.clamp(0., 1.);
		let item = div()
			.id(self.id)
			.relative()
			.font_family(theme.sans())
			.h(px(NAV_ROW_HEIGHT))
			.px(tokens::space::CONTENT)
			.flex()
			.flex_row()
			.items_center()
			.gap(px(8.))
			.rounded(theme.style.radii.row)
			.rounded_smoothing_ios()
			.text_size(theme.text_navigation())
			.font_weight(if selected {
				FontWeight::SEMIBOLD
			} else {
				FontWeight::NORMAL
			})
			.text_color(foreground)
			.bg(row_fill(theme.palette.ink, theme.accent, shown, row.frame.light))
			.role(Role::Tab)
			.aria_label(self.title.clone())
			.aria_selected(selected);
		let item = if enabled {
			row.wire(item, FocusShape::Rounded(theme.style.radii.row), &theme, window)
		} else {
			item.aria_disabled(true)
				.cursor_default()
				.opacity(control::DISABLED_FOREGROUND_OPACITY)
		};
		item.when(shown > 0.01, |item| {
			item.child(
				div()
					.absolute()
					.left(px(0.))
					.top(px(NAV_ROW_HEIGHT / 2. - 8. * shown))
					.w(px(3.))
					.h(px(16. * shown))
					.rounded_full()
					.bg(theme.accent),
			)
		})
		.child(self.icon.render(theme.pt(16.), foreground))
		.child(self.title)
		.when_some(self.on_click.filter(|_| enabled), |item, on_click| {
			item.on_click(on_click)
		})
	}
}

/// One rail entry; the selection carries the accent fill and marker. Only the selected entry
/// is a tab stop: Up and Down move between entries (see [`nav_rail`]).
#[derive(IntoElement)]
pub struct NavItem {
	id: ElementId,
	title: SharedString,
	icon: IconRef,
	selected: bool,
	enabled: bool,
	theme: Option<Theme>,
	on_click: Option<ClickHandler>,
}

/// A rail entry with an icon and a title.
///
/// ```ignore
/// nav_item("audio", "Audio", Icon::Volume).selected(page == Page::Audio).on_click(..)
/// ```
pub fn nav_item(
	id: impl Into<ElementId>,
	title: impl Into<SharedString>,
	icon: impl Into<IconRef>,
) -> NavItem {
	NavItem {
		id: id.into(),
		title: title.into(),
		icon: icon.into(),
		selected: false,
		enabled: true,
		theme: None,
		on_click: None,
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn rail_arrows_stop_at_the_ends() {
		assert_eq!(rail_target("up", 0, 3), Some(0));
		assert_eq!(rail_target("up", 2, 3), Some(1));
		assert_eq!(rail_target("down", 2, 3), Some(2));
		assert_eq!(rail_target("down", 0, 3), Some(1));
		assert_eq!(rail_target("home", 2, 3), Some(0));
		assert_eq!(rail_target("end", 0, 3), Some(2));
		assert_eq!(rail_target("left", 0, 3), None);
		assert_eq!(rail_target("down", 0, 0), None);
	}

	#[test]
	fn only_the_selected_entry_is_a_tab_stop() {
		let rail = RailEntries::default();
		rail.begin();
		let cx = gpui::TestAppContext::single();
		let handles: Vec<FocusHandle> = (0..3).map(|_| cx.update(|cx| cx.focus_handle())).collect();
		// Nothing was selected before: the first entry stands in so the rail stays reachable.
		assert!(rail.register(&handles[0], false));
		assert!(!rail.register(&handles[1], false));
		assert!(rail.register(&handles[2], true));
		// Once a frame had a selected entry, the first one stops being a tab stop.
		rail.begin();
		assert!(!rail.register(&handles[0], false));
		assert!(rail.register(&handles[1], true));
		assert!(!rail.register(&handles[2], false));
	}
}
