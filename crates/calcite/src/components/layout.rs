// SPDX-License-Identifier: MPL-2.0

//! Page and panel layout: [`page_header`], [`settings_page`], [`settings_panel`],
//! [`danger_zone`], [`action_row`], [`hairline`], [`nav_rail`] with [`nav_item`],
//! [`list_row`], [`link`], [`document_link`], and [`glass_panel`]. Every builder reads the
//! theme from the app (`cx.theme()`); `.theme(theme)` overrides it per component.
//!
//! ```ignore
//! settings_page("updates", "Updates", "Keep the launcher and the game current.")
//!     .child(settings_panel("Launcher", Icon::Sparkles)
//!         .child(action_row("Automatic updates").detail("Checks once a day")
//!             .action(toggle("auto", self.auto).label("Automatic updates"))))
//! ```

use std::{cell::RefCell, rc::Rc};

use gpui::{
	Animation, AnimationExt, AnyElement, App, ClickEvent, Div, ElementId, FocusHandle, FontWeight,
	IntoElement, ParentElement, Role, SharedString, StyleRefinement, Styled, Window, div, prelude::*, px,
};
use smallvec::SmallVec;

use crate::{
	controls::{ClickHandler, ThemeOverride, Tone, theme_override},
	glass::{Elevation, GlassExt},
	icons::{Icon, IconRef},
	interaction::{
		ClickableRow, ControlInteraction, ControlState, Feedback, FocusShape, KeyboardFocus, focus_handle,
		row_fill, spring_value,
	},
	motion::{self, Curve, tokens::SELECT},
	theme::{ActiveTheme, Theme},
	tokens::{self, control, with_alpha},
};

/// A page title, a subtitle, and the accent rule under them (`SectionPageHeader`).
#[derive(IntoElement)]
pub struct PageHeader {
	title: SharedString,
	subtitle: SharedString,
	theme: Option<Theme>,
}

/// A page title and subtitle over the accent rule.
///
/// ```ignore
/// page_header("Appearance", "Accent and glass")
/// ```
pub fn page_header(title: impl Into<SharedString>, subtitle: impl Into<SharedString>) -> PageHeader {
	PageHeader {
		title: title.into(),
		subtitle: subtitle.into(),
		theme: None,
	}
}

impl RenderOnce for PageHeader {
	fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		div()
			.font_family(theme.sans())
			.flex()
			.flex_col()
			.gap(px(8.))
			.child(
				div()
					.text_size(px(26.))
					.font_weight(FontWeight::BOLD)
					.child(self.title),
			)
			.child(
				div()
					.text_size(theme.text_body())
					.text_color(theme.palette.secondary_text)
					.child(self.subtitle),
			)
			.child(
				div()
					.pt(px(6.))
					.flex()
					.flex_row()
					.items_center()
					.gap(px(8.))
					.child(div().w(px(72.)).h(px(3.)).bg(theme.accent))
					.child(div().flex_1().h(px(1.)).bg(theme.ink(0.16))),
			)
	}
}

/// A one-pixel row separator (`SettingsHairline`).
#[derive(IntoElement)]
pub struct Hairline {
	base: Div,
	theme: Option<Theme>,
}

/// A full-width one-pixel separator in the palette's hairline color.
///
/// ```ignore
/// hairline()
/// ```
pub fn hairline() -> Hairline {
	Hairline {
		base: div().w_full().h(px(1.)),
		theme: None,
	}
}

impl Styled for Hairline {
	fn style(&mut self) -> &mut StyleRefinement {
		self.base.style()
	}
}

impl RenderOnce for Hairline {
	fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		self.base.bg(theme.palette.hairline)
	}
}

/// A scrolling settings page with its header (`SettingsPage`).
#[derive(IntoElement)]
pub struct SettingsPage {
	id: ElementId,
	title: SharedString,
	subtitle: SharedString,
	children: SmallVec<[AnyElement; 4]>,
	theme: Option<Theme>,
}

/// A settings page. Add panels with `.child(..)`.
pub fn settings_page(
	id: impl Into<ElementId>,
	title: impl Into<SharedString>,
	subtitle: impl Into<SharedString>,
) -> SettingsPage {
	SettingsPage {
		id: id.into(),
		title: title.into(),
		subtitle: subtitle.into(),
		children: SmallVec::new(),
		theme: None,
	}
}

impl ParentElement for SettingsPage {
	fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
		self.children.extend(elements);
	}
}

impl RenderOnce for SettingsPage {
	fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		div()
			.id(self.id)
			.size_full()
			.font_family(theme.sans())
			.overflow_y_scroll()
			.child(
				div()
					.flex()
					.flex_col()
					.gap(px(20.))
					.px(px(26.))
					.pt(px(26.))
					.pb(px(90.))
					.child(page_header(self.title, self.subtitle).theme(theme))
					.children(self.children),
			)
	}
}

/// A quiet glass panel with a titled header (`SettingsPanel`), optionally collapsible.
#[derive(IntoElement)]
pub struct SettingsPanel {
	id: ElementId,
	title: SharedString,
	icon: IconRef,
	tone: Tone,
	expanded: Option<bool>,
	on_toggle: Option<ClickHandler>,
	children: SmallVec<[AnyElement; 4]>,
	theme: Option<Theme>,
}

/// A settings panel. Add rows with `.child(..)`.
pub fn settings_panel(
	id: impl Into<ElementId>,
	title: impl Into<SharedString>,
	icon: impl Into<IconRef>,
) -> SettingsPanel {
	SettingsPanel {
		id: id.into(),
		title: title.into(),
		icon: icon.into(),
		tone: Tone::Neutral,
		expanded: None,
		on_toggle: None,
		children: SmallVec::new(),
		theme: None,
	}
}

/// The danger-toned panel reserved for destructive actions (`DangerZonePanel`).
pub fn danger_zone(id: impl Into<ElementId>) -> SettingsPanel {
	settings_panel(id, "Danger Zone", Icon::Warning).tone(Tone::Danger)
}

impl SettingsPanel {
	/// Tints the title and the border. Neutral by default.
	pub fn tone(mut self, tone: Tone) -> Self {
		self.tone = tone;
		self
	}

	/// Makes the header a disclosure that shows or hides the content.
	pub fn collapsible(
		mut self,
		expanded: bool,
		on_toggle: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
	) -> Self {
		self.expanded = Some(expanded);
		self.on_toggle = Some(Box::new(on_toggle));
		self
	}
}

impl ParentElement for SettingsPanel {
	fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
		self.children.extend(elements);
	}
}

impl RenderOnce for SettingsPanel {
	fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let reduce_motion = theme.reduce_motion;
		let color = self.tone.text(&theme);
		let border = match self.tone {
			Tone::Neutral => theme.palette.panel_border,
			tone => with_alpha(tone.text(&theme), control::SEMANTIC_BORDER_OPACITY),
		};
		let shows_content = self.expanded.unwrap_or(true);
		let rotation = spring_value(
			(self.id.clone(), "disclosure"),
			if shows_content { 1. } else { 0. },
			Curve::State,
			theme.reduce_motion,
			window,
			cx,
		);
		let label = div()
			.flex()
			.flex_row()
			.items_center()
			.gap(px(7.))
			.text_size(px(13.))
			.font_weight(FontWeight::SEMIBOLD)
			.text_color(color)
			.child(self.icon.render(theme.pt(13.), color))
			.child(self.title.clone());
		let header = match (self.expanded, self.on_toggle) {
			(Some(expanded), Some(on_toggle)) => div()
				.id((self.id.clone(), "header"))
				.flex()
				.flex_row()
				.items_center()
				.role(Role::Button)
				.aria_label(self.title.clone())
				.aria_expanded(expanded)
				.child(label)
				.child(div().flex_1())
				.child(
					// The chevron turns from right to down, like the SwiftUI rotation.
					if rotation > 0.5 {
						Icon::ChevronDown
					} else {
						Icon::ChevronRight
					}
					.render(theme.pt(10.), theme.palette.secondary_text),
				)
				.on_click(on_toggle)
				.into_any_element(),
			_ => label.into_any_element(),
		};
		div()
			.id((self.id.clone(), "panel"))
			.w_full()
			.flex()
			.flex_col()
			.gap(px(12.))
			.p(px(16.))
			.font_family(theme.sans())
			.rounded(theme.style.radii.panel)
			.rounded_smoothing_ios()
			.glass(theme.surface(None).elevation(Elevation::None))
			.border_1()
			.border_color(border)
			.role(Role::Group)
			.aria_label(self.title.clone())
			.child(header)
			.when(shows_content, |panel| {
				// Revealed content fades and settles in, like a SwiftUI disclosure group.
				panel.child(
					div()
						.flex()
						.flex_col()
						.gap(px(12.))
						.children(self.children)
						.with_animation(
							(self.id.clone(), "calcite-disclosure"),
							Animation::new(Curve::Disclosure.timed(reduce_motion)),
							move |content, t| {
								let shift = if reduce_motion {
									0.
								} else {
									DISCLOSURE_SHIFT * (1. - t)
								};
								content.opacity(t.clamp(0., 1.)).mt(px(shift))
							},
						),
				)
			})
	}
}

/// A title and detail on the left and actions on the right (`SettingsActionRow`).
#[derive(IntoElement)]
pub struct ActionRow {
	title: SharedString,
	detail: Option<SharedString>,
	actions: SmallVec<[AnyElement; 2]>,
}

/// A settings row. Add controls with `.action(..)`.
pub fn action_row(title: impl Into<SharedString>) -> ActionRow {
	ActionRow {
		title: title.into(),
		detail: None,
		actions: SmallVec::new(),
	}
}

impl ActionRow {
	pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
		self.detail = Some(detail.into());
		self
	}

	pub fn action(mut self, action: impl IntoElement) -> Self {
		self.actions.push(action.into_any_element());
		self
	}
}

impl RenderOnce for ActionRow {
	fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = cx.theme();
		div()
			.w_full()
			.flex()
			.flex_row()
			.items_center()
			.font_family(theme.sans())
			.gap(px(20.))
			.child(
				div()
					.flex_1()
					.min_w_0()
					.flex()
					.flex_col()
					.gap(px(4.))
					.child(div().text_size(theme.text_body()).child(self.title))
					.when_some(self.detail, |label, detail| {
						label.child(
							div()
								.text_size(theme.text_caption())
								.text_color(theme.palette.secondary_text)
								.child(detail),
						)
					}),
			)
			.child(
				div()
					.flex()
					.flex_row()
					.items_center()
					.gap(px(8.))
					.flex_none()
					.children(self.actions),
			)
	}
}

/// How far revealed panel content travels into place.
const DISCLOSURE_SHIFT: f32 = -8.;

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

/// The rail's state, shared with its items.
#[derive(Clone, Default)]
struct RailEntries(Rc<RefCell<RailState>>);

thread_local! {
	/// The rail whose children are rendering. A rail renders before its subtree, so an item
	/// finds its own rail here.
	static CURRENT_RAIL: RefCell<Option<RailEntries>> = const { RefCell::new(None) };
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

/// One selectable row inside an expanded pill or panel (for example a region).
#[derive(IntoElement)]
pub struct ListRow {
	id: ElementId,
	label: SharedString,
	value: Option<SharedString>,
	selected: bool,
	enabled: bool,
	theme: Option<Theme>,
	on_click: Option<ClickHandler>,
}

/// A selectable row with a label and an optional trailing value.
pub fn list_row(id: impl Into<ElementId>, label: impl Into<SharedString>) -> ListRow {
	ListRow {
		id: id.into(),
		label: label.into(),
		value: None,
		selected: false,
		enabled: true,
		theme: None,
		on_click: None,
	}
}

impl ListRow {
	/// Whether the row responds. Enabled by default.
	pub fn enabled(mut self, enabled: bool) -> Self {
		self.enabled = enabled;
		self
	}

	/// The inverse of [`ListRow::enabled`].
	pub fn disabled(self, disabled: bool) -> Self {
		self.enabled(!disabled)
	}

	pub fn value(mut self, value: impl Into<SharedString>) -> Self {
		self.value = Some(value.into());
		self
	}

	pub fn selected(mut self, selected: bool) -> Self {
		self.selected = selected;
		self
	}

	pub fn on_click(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
		self.on_click = Some(Box::new(handler));
		self
	}
}

impl RenderOnce for ListRow {
	fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let accent = theme.legible(theme.accent);
		let reduce_motion = theme.reduce_motion;
		let selected = self.selected;
		let enabled = self.enabled;
		let row = ClickableRow::new(&self.id, &theme, window, cx);
		let shown = motion::spring(
			(self.id.clone(), "calcite-list-selection"),
			if selected { 1. } else { 0. },
			SELECT,
			reduce_motion,
			window,
			cx,
		)
		.value
		.clamp(0., 1.);
		let item = div()
			.id(self.id)
			.font_family(theme.sans())
			.h(px(26.))
			.px(px(8.))
			.flex()
			.flex_row()
			.items_center()
			.gap(px(6.))
			.rounded(theme.style.radii.row)
			.rounded_smoothing_ios()
			.bg(row.fill(&theme, shown))
			.role(Role::ListBoxOption)
			.aria_label(self.label.clone())
			.aria_selected(selected);
		let item = if enabled {
			row.wire(item, FocusShape::Rounded(theme.style.radii.row), &theme, window)
		} else {
			item.aria_disabled(true)
				.cursor_default()
				.opacity(control::DISABLED_FOREGROUND_OPACITY)
		};
		item.when_some(self.on_click.filter(|_| enabled), |row, on_click| {
			row.on_click(on_click)
		})
		.child(div().w(px(12.)).when(selected, |this| {
			this.child(Icon::Checkmark.render(theme.pt(10.), accent))
		}))
		.child(
			div()
				.flex_1()
				.font_weight(FontWeight::SEMIBOLD)
				.text_color(if selected {
					accent
				} else {
					theme.palette.primary_text
				})
				.child(self.label),
		)
		.when_some(self.value, |row, value| {
			row.child(div().text_color(theme.palette.secondary_text).child(value))
		})
	}
}

/// An accent text link that underlines on hover (`AccentLink`).
#[derive(IntoElement)]
pub struct Link {
	id: ElementId,
	title: SharedString,
	enabled: bool,
	theme: Option<Theme>,
	on_click: Option<ClickHandler>,
}

/// An accent text link.
///
/// ```ignore
/// link("privacy", "Privacy notice").on_click(..)
/// ```
pub fn link(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Link {
	Link {
		id: id.into(),
		title: title.into(),
		enabled: true,
		theme: None,
		on_click: None,
	}
}

impl Link {
	/// Whether the link responds. Enabled by default.
	pub fn enabled(mut self, enabled: bool) -> Self {
		self.enabled = enabled;
		self
	}

	/// The inverse of [`Link::enabled`].
	pub fn disabled(self, disabled: bool) -> Self {
		self.enabled(!disabled)
	}

	pub fn on_click(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
		self.on_click = Some(Box::new(handler));
		self
	}
}

impl RenderOnce for Link {
	fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let enabled = self.enabled;
		let focus = focus_handle(&self.id, window, cx);
		let state = ControlState::of(&self.id, window, cx);
		let frame = ControlState::frame(
			&state,
			Feedback::PressOnly,
			enabled,
			theme.reduce_motion,
			window,
			cx,
		);
		let link = div()
			.id(self.id)
			.relative()
			.text_color(theme.legible(theme.accent))
			.font_family(theme.sans())
			.role(Role::Link)
			.aria_label(self.title.clone())
			.when(frame.hovered, |link| link.underline());
		let link = if enabled {
			link.cursor_pointer().control_state(&state, false).keyboard_focus(
				&focus,
				FocusShape::Rounded(px(3.)),
				&theme,
				window,
			)
		} else {
			link.aria_disabled(true)
				.cursor_default()
				.opacity(control::DISABLED_FOREGROUND_OPACITY)
		};
		link.child(self.title)
			.when_some(self.on_click.filter(|_| enabled), |link, on_click| {
				link.on_click(on_click)
			})
	}
}

/// A row that opens a document (`DocumentLinkRow`).
#[derive(IntoElement)]
pub struct DocumentLink {
	id: ElementId,
	title: SharedString,
	icon: IconRef,
	enabled: bool,
	theme: Option<Theme>,
	on_click: Option<ClickHandler>,
}

/// A row that opens a document, with an icon and a chevron.
///
/// ```ignore
/// document_link("license", "License", Icon::Document).on_click(..)
/// ```
pub fn document_link(
	id: impl Into<ElementId>,
	title: impl Into<SharedString>,
	icon: impl Into<IconRef>,
) -> DocumentLink {
	DocumentLink {
		id: id.into(),
		title: title.into(),
		icon: icon.into(),
		enabled: true,
		theme: None,
		on_click: None,
	}
}

impl DocumentLink {
	/// Whether the row responds. Enabled by default.
	pub fn enabled(mut self, enabled: bool) -> Self {
		self.enabled = enabled;
		self
	}

	/// The inverse of [`DocumentLink::enabled`].
	pub fn disabled(self, disabled: bool) -> Self {
		self.enabled(!disabled)
	}

	pub fn on_click(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
		self.on_click = Some(Box::new(handler));
		self
	}
}

impl RenderOnce for DocumentLink {
	fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let accent = theme.legible(theme.accent);
		let row = ClickableRow::new(&self.id, &theme, window, cx);
		// The accent hover fill and text fade in together with the hover light.
		let hover_fill = row.frame.hover_fill(theme.accent);
		let text = tokens::mix(theme.palette.primary_text, accent, row.frame.light);
		let item = div()
			.id(self.id)
			.w_full()
			.flex()
			.flex_row()
			.items_center()
			.gap(px(8.))
			.px(px(6.))
			.py(px(12.))
			.rounded(theme.style.radii.row)
			.text_size(theme.text_body())
			.font_family(theme.sans())
			.bg(hover_fill)
			.text_color(text)
			.role(Role::Link)
			.aria_label(self.title.clone());
		let enabled = self.enabled;
		let item = if enabled {
			row.wire(item, FocusShape::Rounded(theme.style.radii.row), &theme, window)
		} else {
			item.aria_disabled(true)
				.cursor_default()
				.opacity(control::DISABLED_FOREGROUND_OPACITY)
		};
		item.child(self.icon.render(theme.pt(13.), theme.palette.secondary_text))
			.child(div().flex_1().child(self.title))
			.child(Icon::ChevronRight.render(theme.pt(9.), theme.ink(0.32)))
			.when_some(self.on_click.filter(|_| enabled), |row, on_click| {
				row.on_click(on_click)
			})
	}
}

/// A raised glass panel with the fixed HUD radius, ready for children. Style it like any GPUI
/// element: 14 pt padding is the default.
#[derive(IntoElement)]
pub struct GlassPanel {
	base: Div,
	children: SmallVec<[AnyElement; 4]>,
	theme: Option<Theme>,
}

/// A raised glass panel.
///
/// ```ignore
/// glass_panel().w(px(270.)).child("Showcase controls")
/// ```
pub fn glass_panel() -> GlassPanel {
	GlassPanel {
		base: div().p(px(14.)),
		children: SmallVec::new(),
		theme: None,
	}
}

impl Styled for GlassPanel {
	fn style(&mut self) -> &mut StyleRefinement {
		self.base.style()
	}
}

impl ParentElement for GlassPanel {
	fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
		self.children.extend(elements);
	}
}

impl RenderOnce for GlassPanel {
	fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		self.base
			.rounded(theme.style.radii.hud_pill)
			.rounded_smoothing_ios()
			.glass_hud_at(&theme, Elevation::Raised)
			.children(self.children)
	}
}

theme_override!(
	PageHeader,
	Hairline,
	SettingsPage,
	SettingsPanel,
	NavRail,
	NavItem,
	ListRow,
	Link,
	DocumentLink,
	GlassPanel
);

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
