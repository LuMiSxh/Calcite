// SPDX-License-Identifier: MPL-2.0

//! Text links: [`link`] is an inline action, and [`document_link`] opens a document.

use gpui::{
	App, ClickEvent, ElementId, IntoElement, ParentElement, Role, SharedString, Styled, Window, div,
	prelude::*, px,
};

use crate::{
	controls::{ClickHandler, theme_override},
	icons::{Icon, IconRef},
	interaction::{
		ClickableRow, ControlInteraction, ControlState, Feedback, FocusShape, KeyboardFocus, focus_handle,
	},
	theme::{ActiveTheme, Theme},
	tokens::{self, control},
};

theme_override!(Link, DocumentLink);

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
