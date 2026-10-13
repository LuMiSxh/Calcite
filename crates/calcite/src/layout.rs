// SPDX-License-Identifier: MPL-2.0

//! Page and panel layout: [`page_header`], [`settings_page`], [`settings_panel`],
//! [`danger_zone`], [`action_row`], [`hairline`], [`nav_rail`] with [`nav_item`],
//! [`list_row`], [`link`], [`document_link`], and [`glass_panel`].
//!
//! ```ignore
//! settings_page("updates", "Updates", "Keep the launcher and the game current.")
//!     .child(settings_panel("Launcher", Icon::Sparkles)
//!         .child(action_row("Automatic updates").detail("Checks once a day")
//!             .action(toggle("auto", self.auto).label("Automatic updates"))))
//! ```

use gpui::{
	AnyElement, App, ClickEvent, Div, ElementId, FontWeight, IntoElement, ParentElement, Rgba, Role,
	SharedString, Styled, Window, div, prelude::*, px,
};
use smallvec::SmallVec;

use crate::{
	controls::{ClickHandler, theme_override},
	glass::{Elevation, GlassExt},
	icons::{Icon, IconRef},
	interaction::spring_value,
	motion::Curve,
	theme::{ActiveTheme, Theme},
	tokens::{self, control, radius, scale_alpha, white, with_alpha},
};

/// The semantic tone of a panel (`SettingsPanelTone`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PanelTone {
	#[default]
	Neutral,
	Success,
	Warning,
	Danger,
}

impl PanelTone {
	fn color(self) -> Rgba {
		match self {
			Self::Neutral => tokens::PRIMARY_TEXT,
			Self::Success => tokens::SUCCESS,
			Self::Warning => tokens::WARNING,
			Self::Danger => tokens::DANGER_FOREGROUND,
		}
	}

	fn border(self) -> Rgba {
		match self {
			Self::Neutral => control::PANEL_BORDER,
			tone => with_alpha(tone.color(), control::SEMANTIC_BORDER_OPACITY),
		}
	}
}

/// A page title, a subtitle, and the accent rule under them (`SectionPageHeader`).
pub fn page_header(title: impl Into<SharedString>, subtitle: impl Into<SharedString>, theme: &Theme) -> Div {
	div()
		.flex()
		.flex_col()
		.gap(px(8.))
		.child(
			div()
				.text_size(px(26.))
				.font_weight(FontWeight::BOLD)
				.child(title.into()),
		)
		.child(
			div()
				.text_size(tokens::text::BODY)
				.text_color(tokens::SECONDARY_TEXT)
				.child(subtitle.into()),
		)
		.child(
			div()
				.pt(px(6.))
				.flex()
				.flex_row()
				.items_center()
				.gap(px(8.))
				.child(div().w(px(72.)).h(px(3.)).bg(theme.accent))
				.child(div().flex_1().h(px(1.)).bg(white(0.16))),
		)
}

/// A one-pixel row separator (`SettingsHairline`).
pub fn hairline() -> Div {
	div().w_full().h(px(1.)).bg(tokens::HAIRLINE)
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
		div().id(self.id).size_full().overflow_y_scroll().child(
			div()
				.flex()
				.flex_col()
				.gap(px(20.))
				.px(px(26.))
				.pt(px(26.))
				.pb(px(90.))
				.child(page_header(self.title, self.subtitle, &theme))
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
	tone: PanelTone,
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
		tone: PanelTone::Neutral,
		expanded: None,
		on_toggle: None,
		children: SmallVec::new(),
		theme: None,
	}
}

/// The danger-toned panel reserved for destructive actions (`DangerZonePanel`).
pub fn danger_zone(id: impl Into<ElementId>) -> SettingsPanel {
	settings_panel(id, "Danger Zone", Icon::Warning).tone(PanelTone::Danger)
}

impl SettingsPanel {
	pub fn tone(mut self, tone: PanelTone) -> Self {
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
		let color = self.tone.color();
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
			.child(self.icon.render(px(13.), color))
			.child(self.title.clone());
		let header = match (self.expanded, self.on_toggle) {
			(Some(expanded), Some(on_toggle)) => div()
				.id((self.id.clone(), "header"))
				.flex()
				.flex_row()
				.items_center()
				.cursor_pointer()
				.role(Role::Button)
				.aria_label(SharedString::from(format!(
					"{}, {}",
					self.title,
					if expanded { "expanded" } else { "collapsed" }
				)))
				.child(label)
				.child(div().flex_1())
				.child(
					// The chevron turns from right to down, like the SwiftUI rotation.
					if rotation > 0.5 {
						Icon::ChevronDown
					} else {
						Icon::ChevronRight
					}
					.render(px(10.), tokens::SECONDARY_TEXT),
				)
				.on_click(on_toggle)
				.into_any_element(),
			_ => label.into_any_element(),
		};
		div()
			.w_full()
			.flex()
			.flex_col()
			.gap(px(12.))
			.p(px(16.))
			.rounded(radius::PANEL)
			.rounded_smoothing_ios()
			.glass(theme.surface(None).elevation(Elevation::None))
			.border_1()
			.border_color(self.tone.border())
			.child(header)
			.when(shows_content, |panel| panel.children(self.children))
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
	fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
		div()
			.w_full()
			.flex()
			.flex_row()
			.items_center()
			.gap(px(20.))
			.child(
				div()
					.flex_1()
					.min_w_0()
					.flex()
					.flex_col()
					.gap(px(4.))
					.child(div().text_size(tokens::text::BODY).child(self.title))
					.when_some(self.detail, |label, detail| {
						label.child(
							div()
								.text_size(tokens::text::CAPTION)
								.text_color(tokens::SECONDARY_TEXT)
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

/// The Settings navigation rail.
pub fn nav_rail() -> Div {
	div()
		.h_full()
		.w(px(200.))
		.flex()
		.flex_col()
		.gap(px(2.))
		.p(px(10.))
		.pt(px(44.))
		.bg(control::NAVIGATION_RAIL_BACKGROUND)
		.border_r_1()
		.border_color(tokens::HAIRLINE)
}

/// One rail entry; the selection carries the accent fill and marker.
#[derive(IntoElement)]
pub struct NavItem {
	id: ElementId,
	title: SharedString,
	icon: IconRef,
	selected: bool,
	theme: Option<Theme>,
	on_click: Option<ClickHandler>,
}

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
		theme: None,
		on_click: None,
	}
}

impl NavItem {
	pub fn selected(mut self, selected: bool) -> Self {
		self.selected = selected;
		self
	}

	pub fn on_click(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
		self.on_click = Some(Box::new(handler));
		self
	}
}

impl RenderOnce for NavItem {
	fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let accent = tokens::contrast_tint(theme.accent);
		let foreground = if self.selected {
			accent
		} else {
			tokens::PRIMARY_TEXT
		};
		let selected = self.selected;
		div()
			.id(self.id)
			.relative()
			.h(px(30.))
			.px(px(10.))
			.flex()
			.flex_row()
			.items_center()
			.gap(px(8.))
			.rounded(radius::ROW)
			.rounded_smoothing_ios()
			.text_size(px(13.))
			.font_weight(if selected {
				FontWeight::SEMIBOLD
			} else {
				FontWeight::NORMAL
			})
			.text_color(foreground)
			.bg(if selected {
				scale_alpha(theme.accent, 0.13)
			} else {
				tokens::CLEAR
			})
			.when(!selected, |item| {
				item.hover(|style| style.bg(control::NAVIGATION_HOVER_FILL))
			})
			.cursor_pointer()
			.role(Role::Tab)
			.aria_label(self.title.clone())
			.when(selected, |item| {
				item.child(
					div()
						.absolute()
						.left(px(0.))
						.top(px(7.))
						.w(px(3.))
						.h(px(16.))
						.rounded_full()
						.bg(theme.accent),
				)
			})
			.child(self.icon.render(px(14.), foreground))
			.child(self.title)
			.when_some(self.on_click, |item, on_click| item.on_click(on_click))
	}
}

/// One selectable row inside an expanded pill or panel (for example a region).
#[derive(IntoElement)]
pub struct ListRow {
	id: ElementId,
	label: SharedString,
	value: Option<SharedString>,
	selected: bool,
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
		theme: None,
		on_click: None,
	}
}

impl ListRow {
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
	fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let accent = tokens::contrast_tint(theme.accent);
		let reduce_motion = theme.reduce_motion;
		let selected = self.selected;
		let selected_fill = scale_alpha(theme.accent, 0.13);
		div()
			.id(self.id)
			.h(px(26.))
			.px(px(8.))
			.flex()
			.flex_row()
			.items_center()
			.gap(px(6.))
			.rounded(radius::CONTROL)
			.rounded_smoothing_ios()
			.bg(if selected { selected_fill } else { tokens::CLEAR })
			.cursor_pointer()
			.role(Role::Button)
			.aria_label(self.label.clone())
			.transitions(move |transitions| transitions.bg(Curve::Select.transition(reduce_motion)))
			.hover(move |style| style.bg(if selected { selected_fill } else { white(0.06) }))
			.when_some(self.on_click, |row, on_click| row.on_click(on_click))
			.child(div().w(px(12.)).when(selected, |this| {
				this.child(Icon::Checkmark.render(px(10.), accent))
			}))
			.child(
				div()
					.flex_1()
					.font_weight(FontWeight::SEMIBOLD)
					.text_color(if selected { accent } else { tokens::PRIMARY_TEXT })
					.child(self.label),
			)
			.when_some(self.value, |row, value| {
				row.child(div().text_color(tokens::SECONDARY_TEXT).child(value))
			})
	}
}

/// An accent text link that underlines on hover (`AccentLink`).
#[derive(IntoElement)]
pub struct Link {
	id: ElementId,
	title: SharedString,
	theme: Option<Theme>,
	on_click: Option<ClickHandler>,
}

pub fn link(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Link {
	Link {
		id: id.into(),
		title: title.into(),
		theme: None,
		on_click: None,
	}
}

impl Link {
	pub fn on_click(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
		self.on_click = Some(Box::new(handler));
		self
	}
}

impl RenderOnce for Link {
	fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		div()
			.id(self.id)
			.text_color(tokens::contrast_tint(theme.accent))
			.cursor_pointer()
			.role(Role::Link)
			.aria_label(self.title.clone())
			.hover(|style| style.underline())
			.child(self.title)
			.when_some(self.on_click, |link, on_click| link.on_click(on_click))
	}
}

/// A row that opens a document (`DocumentLinkRow`).
#[derive(IntoElement)]
pub struct DocumentLink {
	id: ElementId,
	title: SharedString,
	icon: IconRef,
	theme: Option<Theme>,
	on_click: Option<ClickHandler>,
}

pub fn document_link(
	id: impl Into<ElementId>,
	title: impl Into<SharedString>,
	icon: impl Into<IconRef>,
) -> DocumentLink {
	DocumentLink {
		id: id.into(),
		title: title.into(),
		icon: icon.into(),
		theme: None,
		on_click: None,
	}
}

impl DocumentLink {
	pub fn on_click(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
		self.on_click = Some(Box::new(handler));
		self
	}
}

impl RenderOnce for DocumentLink {
	fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let accent = tokens::contrast_tint(theme.accent);
		let hover_fill = scale_alpha(theme.accent, 0.08);
		let reduce_motion = theme.reduce_motion;
		div()
			.id(self.id)
			.w_full()
			.flex()
			.flex_row()
			.items_center()
			.gap(px(8.))
			.px(px(6.))
			.py(px(12.))
			.rounded(radius::ROW)
			.text_size(tokens::text::BODY)
			.cursor_pointer()
			.role(Role::Link)
			.aria_label(self.title.clone())
			.transitions(move |transitions| {
				transitions
					.bg(Curve::Hover.transition(reduce_motion))
					.text_color(Curve::Hover.transition(reduce_motion))
			})
			.hover(move |style| style.bg(hover_fill).text_color(accent))
			.child(self.icon.render(px(13.), tokens::SECONDARY_TEXT))
			.child(div().flex_1().child(self.title))
			.child(Icon::ChevronRight.render(px(9.), white(0.32)))
			.when_some(self.on_click, |row, on_click| row.on_click(on_click))
	}
}

/// A raised glass panel with the fixed HUD radius, ready for children.
pub fn glass_panel(theme: &Theme) -> Div {
	div()
		.p(px(14.))
		.rounded(radius::HUD_PILL)
		.rounded_smoothing_ios()
		.glass(theme.hud_surface().elevation(Elevation::Raised))
		.border_1()
		.border_color(scale_alpha(tokens::HAIRLINE, control::HUD_BORDER_OPACITY))
}

theme_override!(SettingsPage, SettingsPanel, NavItem, ListRow, Link, DocumentLink);
