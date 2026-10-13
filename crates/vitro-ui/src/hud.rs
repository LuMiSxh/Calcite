// SPDX-License-Identifier: MPL-2.0

//! HUD surfaces: the expandable pill (`ExpandableHUDPill` + `HUDPillSurface`) and the
//! control bar. A collapsed surface is a compact capsule; an expanded surface is a
//! rounded rectangle with the fixed HUD radius.

use gpui::{
	Animation, AnimationExt, AnyElement, App, ClickEvent, ElementId, FontWeight, IntoElement, ParentElement,
	Pixels, Rgba, SharedString, Styled, Window, div, prelude::*, px,
};

use crate::{
	Theme,
	glass::{Elevation, GlassExt},
	icons::Icon,
	motion::Curve,
	progress::{OutlineMode, capsule_progress_outline},
	tokens::{self, control, hud, radius, scale_alpha},
};

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

fn hud_border(progress_visible: bool) -> Rgba {
	scale_alpha(
		tokens::HAIRLINE,
		if progress_visible {
			0.5
		} else {
			control::HUD_BORDER_OPACITY
		},
	)
}

/// A HUD pill that collapses to a capsule and expands to a rounded rectangle.
#[derive(IntoElement)]
pub struct HudPill {
	id: ElementId,
	icon: Icon,
	title: AnyElement,
	detail: Option<AnyElement>,
	expanded: bool,
	receded: bool,
	expanded_size: (Pixels, Pixels),
	theme: Theme,
	on_toggle: Option<ClickHandler>,
}

impl HudPill {
	pub fn new(id: impl Into<ElementId>, icon: Icon, title: impl IntoElement, theme: Theme) -> Self {
		Self {
			id: id.into(),
			icon,
			title: title.into_any_element(),
			detail: None,
			expanded: false,
			receded: false,
			expanded_size: (hud::EXPANDED_STATUS_WIDTH, px(180.)),
			theme,
			on_toggle: None,
		}
	}

	/// Detail content and the panel size it needs when expanded.
	pub fn detail(mut self, detail: impl IntoElement, width: Pixels, height: Pixels) -> Self {
		self.detail = Some(detail.into_any_element());
		self.expanded_size = (width, height);
		self
	}

	pub fn expanded(mut self, expanded: bool) -> Self {
		self.expanded = expanded;
		self
	}

	/// A sibling pill is expanded: this one recedes so the open panel reads as the focus.
	pub fn receded(mut self, receded: bool) -> Self {
		self.receded = receded;
		self
	}

	pub fn on_toggle(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
		self.on_toggle = Some(Box::new(handler));
		self
	}
}

impl RenderOnce for HudPill {
	fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
		let theme = self.theme;
		let reduce_motion = theme.reduce_motion;
		let can_expand = self.detail.is_some();
		let expanded = self.expanded && can_expand;
		let accent_foreground = tokens::contrast_tint(theme.accent);
		let glass = theme.surface(Some(theme.hud_tint)).elevation(if expanded {
			Elevation::Raised
		} else {
			Elevation::Resting
		});

		let chevron = if expanded {
			Icon::ChevronUp
		} else {
			Icon::ChevronDown
		};
		let header = div()
			.id((self.id.clone(), "header"))
			.h(hud::PILL_HEIGHT)
			.flex()
			.flex_row()
			.items_center()
			.gap(px(5.))
			.px(hud::HORIZONTAL_PADDING)
			.child(self.icon.render(px(11.), accent_foreground))
			.child(div().flex_1().min_w_0().child(self.title))
			.when(can_expand, |header| {
				header
					.child(div().w(px(6.)))
					.child(chevron.render(px(10.), accent_foreground))
					.cursor_pointer()
			})
			.when_some(self.on_toggle, |header, on_toggle| header.on_click(on_toggle));

		let (expanded_width, expanded_height) = self.expanded_size;
		let mut surface = div()
			.id(self.id.clone())
			.flex()
			.flex_col()
			.overflow_hidden()
			.text_size(tokens::text::CAPTION)
			.text_color(tokens::PRIMARY_TEXT)
			.glass(glass)
			.border_1()
			.border_color(hud_border(false))
			.opacity(if self.receded { 0.62 } else { 1. })
			.transitions(move |transitions| {
				transitions
					.w(Curve::Present.transition(reduce_motion))
					.h(Curve::Present.transition(reduce_motion))
					.rounded(Curve::Present.transition(reduce_motion))
					.opacity(Curve::State.transition(reduce_motion))
			});
		surface = if expanded {
			surface
				.w(expanded_width)
				.h(expanded_height)
				.rounded(radius::HUD_PILL)
				.rounded_smoothing_ios()
				.py(px(4.))
		} else {
			surface.h(hud::PILL_HEIGHT).rounded(hud::PILL_HEIGHT / 2.)
		};
		surface = surface.child(header);
		if let (true, Some(detail)) = (expanded, self.detail) {
			surface = surface.child(
				div()
					.px(px(14.))
					.pt(px(6.))
					.pb(px(10.))
					.child(detail)
					.with_animation(
						(self.id, "detail"),
						Animation::new(Curve::State.timed(reduce_motion)),
						|detail, t| detail.opacity(t).mt(px(8. * (1. - t))),
					),
			);
		}
		surface
	}
}

/// The compact capsule bar at the bottom of the window. It owns the download outline.
#[derive(IntoElement)]
pub struct ControlBar {
	status: AnyElement,
	actions: Vec<AnyElement>,
	progress: Option<(OutlineMode, bool)>,
	theme: Theme,
}

impl ControlBar {
	pub fn new(status: impl IntoElement, theme: Theme) -> Self {
		Self {
			status: status.into_any_element(),
			actions: Vec::new(),
			progress: None,
			theme,
		}
	}

	pub fn action(mut self, action: impl IntoElement) -> Self {
		self.actions.push(action.into_any_element());
		self
	}

	/// Shows the outline. `active` adds the glow and sweep of a running transfer.
	pub fn progress(mut self, mode: Option<OutlineMode>, active: bool) -> Self {
		self.progress = mode.map(|mode| (mode, active));
		self
	}
}

impl RenderOnce for ControlBar {
	fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
		let theme = self.theme;
		let glass = theme.surface(Some(theme.hud_tint));
		div()
			.relative()
			.w_full()
			.flex()
			.flex_row()
			.items_center()
			.gap(px(16.))
			.p(px(16.))
			.rounded_full()
			.glass(glass)
			.border_1()
			.border_color(hud_border(self.progress.is_some()))
			.child(div().flex_1().min_w_0().child(self.status))
			.child(
				div()
					.flex()
					.flex_row()
					.items_center()
					.gap(px(8.))
					.children(self.actions),
			)
			.when_some(self.progress, |bar, (mode, active)| {
				bar.child(capsule_progress_outline(
					"control-bar-outline",
					mode,
					theme.accent,
					active,
					theme.reduce_motion,
				))
			})
	}
}

/// Status title with an optional secondary line (`LauncherActivityStatusView`).
pub fn status_text(
	title: impl Into<SharedString>,
	detail: Option<SharedString>,
	emphasized: bool,
) -> impl IntoElement {
	div()
		.flex()
		.flex_col()
		.gap(px(2.))
		.child(
			div()
				.text_size(if emphasized {
					tokens::text::STATUS_PERCENT
				} else {
					tokens::text::STATUS_TITLE
				})
				.font_weight(FontWeight::SEMIBOLD)
				.text_color(tokens::PRIMARY_TEXT)
				.truncate()
				.child(title.into()),
		)
		.when_some(detail, |this, detail| {
			this.child(
				div()
					.text_size(tokens::text::CAPTION)
					.text_color(tokens::SECONDARY_TEXT)
					.truncate()
					.child(detail),
			)
		})
}

/// One selectable row inside an expanded pill (for example a region).
pub fn pill_row(
	id: impl Into<ElementId>,
	label: impl Into<SharedString>,
	value: impl Into<SharedString>,
	selected: bool,
	theme: Theme,
	on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
	let accent = tokens::contrast_tint(theme.accent);
	let reduce_motion = theme.reduce_motion;
	let selected_fill = scale_alpha(theme.accent, 0.13);
	div()
		.id(id)
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
		.transitions(move |transitions| transitions.bg(Curve::Select.transition(reduce_motion)))
		.hover(move |style| {
			style.bg(if selected {
				selected_fill
			} else {
				tokens::white(0.06)
			})
		})
		.on_click(on_click)
		.child(div().w(px(12.)).when(selected, |this| {
			this.child(Icon::Checkmark.render(px(10.), accent))
		}))
		.child(
			div()
				.flex_1()
				.font_weight(FontWeight::SEMIBOLD)
				.text_color(if selected { accent } else { tokens::PRIMARY_TEXT })
				.child(label.into()),
		)
		.child(div().text_color(tokens::SECONDARY_TEXT).child(value.into()))
}
