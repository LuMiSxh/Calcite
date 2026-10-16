// SPDX-License-Identifier: MPL-2.0

//! Page chrome: [`page_header`] is the title and accent rule, [`hairline`] is the divider, and
//! [`settings_page`] is the scrolling page that holds the panels.

use gpui::{
	AnyElement, App, Div, ElementId, FontWeight, IntoElement, ParentElement, SharedString, StyleRefinement,
	Styled, Window, div, prelude::*, px,
};
use smallvec::SmallVec;

use crate::{
	controls::{ThemeOverride, theme_override},
	theme::{ActiveTheme, Theme},
};

theme_override!(PageHeader, Hairline, SettingsPage);

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
