// SPDX-License-Identifier: MPL-2.0

//! HUD surfaces: [`pill`] (`ExpandableHUDPill` + `HUDPillSurface`), [`control_bar`], and
//! [`status`]. A collapsed surface is a compact capsule;
//! an expanded surface is a rounded rectangle with the fixed HUD radius.
//!
//! ```ignore
//! control_bar(status("Ready to play").detail("Global · Version 2.6.81"))
//!     .action(button("play", "Play").accent().large().icon(Icon::Play))
//! pill("server", Icon::Globe, "Global 04:12:09")
//!     .noun("server time")
//!     .detail(rows, px(360.), px(200.))
//!     .expanded(open)
//!     .on_toggle(..)
//! ```

use gpui::{
	Animation, AnimationExt, AnyElement, App, ClickEvent, ElementId, FontWeight, IntoElement, ParentElement,
	Pixels, Rgba, Role, SharedString, Styled, Window, div, prelude::*, px,
};

use crate::{
	controls::{ClickHandler, theme_override},
	glass::{Elevation, GlassExt},
	icons::{Icon, IconRef},
	interaction::{layer, spring_value},
	motion::Curve,
	progress::{OutlineMode, progress_outline},
	theme::{ActiveTheme, Theme},
	tokens::{self, control, hud, radius, scale_alpha},
};

fn hud_border(theme: &Theme, progress_visible: bool) -> Rgba {
	scale_alpha(
		theme.palette.hairline,
		if progress_visible {
			0.5
		} else {
			control::HUD_BORDER_OPACITY
		},
	)
}

/// A HUD pill that collapses to a capsule and expands to a rounded rectangle.
#[derive(IntoElement)]
pub struct Pill {
	id: ElementId,
	icon: IconRef,
	title: AnyElement,
	noun: SharedString,
	detail: Option<AnyElement>,
	expanded: bool,
	receded: bool,
	expanded_size: (Pixels, Pixels),
	theme: Option<Theme>,
	on_toggle: Option<ClickHandler>,
}

/// A HUD pill with an icon and a title. Add `.detail(..)` to make it expandable.
pub fn pill(id: impl Into<ElementId>, icon: impl Into<IconRef>, title: impl IntoElement) -> Pill {
	Pill {
		id: id.into(),
		icon: icon.into(),
		title: title.into_any_element(),
		noun: "details".into(),
		detail: None,
		expanded: false,
		receded: false,
		expanded_size: (hud::EXPANDED_STATUS_WIDTH, px(180.)),
		theme: None,
		on_toggle: None,
	}
}

impl Pill {
	/// Completes the "Show …" and "Hide …" accessible label of the disclosure.
	pub fn noun(mut self, noun: impl Into<SharedString>) -> Self {
		self.noun = noun.into();
		self
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

impl RenderOnce for Pill {
	fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let reduce_motion = theme.reduce_motion;
		let can_expand = self.detail.is_some();
		let expanded = self.expanded && can_expand;
		let accent_foreground = tokens::contrast_tint(theme.accent);
		let glass = theme.hud_surface().elevation(if expanded {
			Elevation::Raised
		} else {
			Elevation::Resting
		});
		// `HUDSiblingFocus`: receded pills shrink to 0.96 and fade to 0.62.
		let recede = spring_value(
			(self.id.clone(), "recede"),
			if self.receded { 1. } else { 0. },
			Curve::State,
			reduce_motion,
			window,
			cx,
		);
		let scale = if reduce_motion { 1. } else { 1. - 0.04 * recede };

		let (expanded_width, expanded_height) = self.expanded_size;
		let height = if expanded {
			expanded_height
		} else {
			hud::PILL_HEIGHT
		};
		let corner = if expanded {
			radius::HUD_PILL
		} else {
			hud::PILL_HEIGHT / 2.
		};
		// Scale around the bottom edge, where the pill rises from the control bar.
		let inset_x = px(f32::from(expanded_width.min(px(400.))) * (1. - scale) / 2.);
		let inset_top = px(f32::from(height) * (1. - scale));
		let surface = layer((inset_x, px(0.)))
			.top(inset_top)
			.rounded(corner)
			.when(expanded, |surface| surface.rounded_smoothing_ios())
			.glass(glass)
			.border_1()
			.border_color(hud_border(&theme, false))
			.transitions(move |transitions| transitions.rounded(Curve::Present.transition(reduce_motion)));

		let chevron = if expanded {
			Icon::ChevronUp
		} else {
			Icon::ChevronDown
		};
		let toggle_label: SharedString =
			format!("{} {}", if expanded { "Hide" } else { "Show" }, self.noun).into();
		let header = div()
			.id((self.id.clone(), "header"))
			.relative()
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
					.role(Role::Button)
					.aria_label(toggle_label)
			})
			.when_some(self.on_toggle, |header, on_toggle| header.on_click(on_toggle));

		let mut pill = div()
			.id(self.id.clone())
			.relative()
			.flex()
			.flex_col()
			.text_size(tokens::text::CAPTION)
			.text_color(theme.palette.primary_text)
			.opacity(1. - 0.38 * recede)
			.transitions(move |transitions| {
				transitions
					.w(Curve::Present.transition(reduce_motion))
					.h(Curve::Present.transition(reduce_motion))
			})
			.h(height)
			.when(expanded, |pill| pill.w(expanded_width))
			.child(surface);
		// The content clips to the animating size; the surface keeps its shadow outside it.
		let mut content = div()
			.relative()
			.size_full()
			.flex()
			.flex_col()
			.overflow_hidden()
			.when(expanded, |content| content.py(px(4.)))
			.child(header);
		if let (true, Some(detail)) = (expanded, self.detail) {
			content = content.child(
				div()
					.relative()
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
		pill = pill.child(content);
		pill
	}
}

/// The compact capsule bar at the bottom of the window. It owns the download outline.
#[derive(IntoElement)]
pub struct ControlBar {
	status: AnyElement,
	actions: Vec<AnyElement>,
	progress: Option<(OutlineMode, bool)>,
	theme: Option<Theme>,
}

/// The HUD control bar with status on the left and actions on the right.
pub fn control_bar(status: impl IntoElement) -> ControlBar {
	ControlBar {
		status: status.into_any_element(),
		actions: Vec::new(),
		progress: None,
		theme: None,
	}
}

impl ControlBar {
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
	fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let motion = theme.decorative_motion(window);
		div()
			.relative()
			.w_full()
			.flex()
			.flex_row()
			.items_center()
			.gap(px(16.))
			.p(px(16.))
			.rounded_full()
			.glass(theme.hud_surface())
			.border_1()
			.border_color(hud_border(&theme, self.progress.is_some()))
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
				bar.child(progress_outline(
					"calcite-control-bar-outline",
					mode,
					&theme,
					active,
					motion,
				))
			})
	}
}

/// A status title with an optional secondary line (`LauncherActivityStatusView`).
#[derive(IntoElement)]
pub struct Status {
	title: SharedString,
	detail: Option<SharedString>,
	emphasized: bool,
}

/// A status title. `.emphasized()` uses the larger percentage style.
pub fn status(title: impl Into<SharedString>) -> Status {
	Status {
		title: title.into(),
		detail: None,
		emphasized: false,
	}
}

impl Status {
	pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
		self.detail = Some(detail.into());
		self
	}

	pub fn detail_opt(mut self, detail: Option<SharedString>) -> Self {
		self.detail = detail;
		self
	}

	pub fn emphasized(mut self, emphasized: bool) -> Self {
		self.emphasized = emphasized;
		self
	}
}

impl RenderOnce for Status {
	fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = cx.theme();
		div()
			.flex()
			.flex_col()
			.gap(px(2.))
			.child(
				div()
					.text_size(if self.emphasized {
						tokens::text::STATUS_PERCENT
					} else {
						tokens::text::STATUS_TITLE
					})
					.font_weight(FontWeight::SEMIBOLD)
					.text_color(theme.palette.primary_text)
					.truncate()
					.child(self.title),
			)
			.when_some(self.detail, |this, detail| {
				this.child(
					div()
						.text_size(tokens::text::CAPTION)
						.text_color(theme.palette.secondary_text)
						.truncate()
						.child(detail),
				)
			})
	}
}

theme_override!(Pill, ControlBar);
