// SPDX-License-Identifier: MPL-2.0

//! A selectable list row: [`list_row`] shows a label and a value, and one row is selected.

use gpui::{
	App, ClickEvent, ElementId, FontWeight, IntoElement, ParentElement, Role, SharedString, Styled, Window,
	div, prelude::*, px,
};

use crate::{
	controls::{ClickHandler, theme_override},
	icons::Icon,
	interaction::{ClickableRow, FocusShape},
	motion::{self, tokens::SELECT},
	theme::{ActiveTheme, Theme},
	tokens::control,
};

theme_override!(ListRow);

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
