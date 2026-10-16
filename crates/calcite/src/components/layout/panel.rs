// SPDX-License-Identifier: MPL-2.0

//! Settings panels and their rows: [`settings_panel`] groups [`action_row`] entries, [`danger_zone`]
//! is the destructive variant, and [`glass_panel`] is the plain glass surface.

use gpui::{
	Animation, AnimationExt, AnyElement, App, ClickEvent, Div, ElementId, FontWeight, IntoElement,
	ParentElement, Role, SharedString, StyleRefinement, Styled, Window, div, prelude::*, px,
};
use smallvec::SmallVec;

use crate::{
	controls::{ClickHandler, Tone, theme_override},
	glass::{Elevation, GlassExt},
	icons::{Icon, IconRef},
	interaction::spring_value,
	motion::Curve,
	theme::{ActiveTheme, Theme},
	tokens::{control, with_alpha},
};

theme_override!(SettingsPanel, GlassPanel);

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

/// How far revealed panel content travels into place.
const DISCLOSURE_SHIFT: f32 = -8.;
