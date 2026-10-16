// SPDX-License-Identifier: MPL-2.0

//! HUD surfaces: [`pill`] (`ExpandableHUDPill` + `HUDPillSurface`), [`control_bar`], and
//! [`status`]. A collapsed surface is a compact capsule;
//! an expanded surface is a rounded rectangle with the fixed HUD radius.
//!
//! ```ignore
//! control_bar(status("Ready to play").detail("Global · Version 2.6.81"))
//!     .action(button("play", "Play").accent().large().icon(Icon::Play))
//! pill("server", Icon::Globe, || "Global 04:12:09")
//!     .noun("server time")
//!     .detail(rows, px(360.), px(200.))
//!     .expanded(open)
//!     .on_toggle(..)
//! ```

use std::rc::Rc;

use gpui::{
	AnyElement, App, ClickEvent, ElementId, Filter, FontWeight, IntoElement, ParentElement, Pixels, Rgba,
	Role, SharedString, Styled, Window, canvas, div, prelude::*, px,
};

use crate::{
	controls::{ClickHandler, ThemeOverride, theme_override},
	glass::{Elevation, GlassExt},
	icons::{Icon, IconRef},
	interaction::{ScaleAnchor, layer, presence, scale_effect, spring_value},
	morph::boost_filter,
	motion::{
		self, Curve,
		secondary::{self, GlassBoost},
	},
	progress::{OutlineMode, progress_outline},
	theme::{ActiveTheme, Theme},
	tokens::{self, control, hud, scale_alpha},
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

/// `from` toward `to`; `t = 0` gives `from`. Not clamped, so a spring may overshoot.
fn lerp(from: Pixels, to: Pixels, t: f32) -> Pixels {
	px(f32::from(from) + (f32::from(to) - f32::from(from)) * t)
}

/// The pill width at morph progress `open`. Until the collapsed width has been measured (a pill
/// created expanded), the width holds the expanded size rather than guessing.
fn morph_width(collapsed: Option<Pixels>, expanded: Pixels, open: f32) -> Pixels {
	lerp(collapsed.unwrap_or(expanded), expanded, open).max(px(0.))
}

/// Whether the pill has come to rest collapsed. The spring's own verdict (`active`) decides, not a
/// distance threshold: a position test drops the width constraint while the spring still moves.
fn is_settled_closed(expanded: bool, active: bool) -> bool {
	!expanded && !active
}

/// Whether the offscreen collapsed-layout probe is needed: the collapsed width is unknown and the
/// header (which measures it when settled closed) is not showing the collapsed layout.
fn needs_probe(collapsed: Option<Pixels>, settled_closed: bool) -> bool {
	collapsed.is_none() && !settled_closed
}

/// A HUD pill that collapses to a capsule and expands to a rounded rectangle. Opening and
/// closing morph the size, the corner radius, and fade the detail.
#[derive(IntoElement)]
pub struct Pill {
	id: ElementId,
	icon: IconRef,
	title: Rc<dyn Fn() -> AnyElement>,
	noun: SharedString,
	detail: Option<AnyElement>,
	expanded: bool,
	receded: bool,
	enabled: bool,
	expanded_size: (Pixels, Pixels),
	theme: Option<Theme>,
	on_toggle: Option<ClickHandler>,
}

/// A HUD pill with an icon and a title. Add `.detail(..)` to make it expandable. The title is a
/// builder because the pill may render it twice: once visible, once as an invisible measurement of
/// the collapsed width when the pill starts expanded.
///
/// ```ignore
/// pill("region", Icon::Globe, || "Global").detail(list, px(220.), px(160.)).expanded(open)
/// ```
pub fn pill<E: IntoElement>(
	id: impl Into<ElementId>,
	icon: impl Into<IconRef>,
	title: impl Fn() -> E + 'static,
) -> Pill {
	Pill {
		id: id.into(),
		icon: icon.into(),
		title: Rc::new(move || title().into_any_element()),
		noun: "details".into(),
		detail: None,
		expanded: false,
		receded: false,
		enabled: true,
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

	/// Whether the pill's disclosure responds. Enabled by default.
	pub fn enabled(mut self, enabled: bool) -> Self {
		self.enabled = enabled;
		self
	}

	/// The inverse of [`Pill::enabled`].
	pub fn disabled(self, disabled: bool) -> Self {
		self.enabled(!disabled)
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
		let accent_foreground = theme.legible(theme.accent);
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
		// 0 collapsed, 1 expanded: size, corner, and padding follow one spring.
		let morph = motion::spring(
			(self.id.clone(), "morph"),
			if expanded { 1. } else { 0. },
			Curve::Present.spring().expect("Present is a spring"),
			reduce_motion,
			window,
			cx,
		);
		let open = morph.value;
		let settled_closed = is_settled_closed(expanded, morph.active);
		// A collapsed pill is as wide as its content; remember that width to morph from it.
		let natural_width =
			window.use_keyed_state((self.id.clone(), "natural-width"), cx, |_, _| None::<Pixels>);
		let collapsed_width = *natural_width.read(cx);
		let detail_presence = presence(
			(self.id.clone(), "detail-presence"),
			expanded,
			reduce_motion,
			window,
			cx,
		);
		let height = lerp(theme.px(hud::PILL_HEIGHT), expanded_height, open).max(px(0.));
		let width = if settled_closed {
			None
		} else {
			Some(morph_width(collapsed_width, expanded_width, open))
		};
		let corner = lerp(
			theme.style.radii.capsule_radius(theme.px(hud::PILL_HEIGHT)),
			theme.style.radii.hud_pill,
			open.clamp(0., 1.),
		);
		// Fast glass catches more light and bends the backdrop harder.
		let boost = secondary::glass_boost(secondary::speed_factor(morph.velocity), reduce_motion);
		let boosted = (boost != GlassBoost::NONE)
			.then(|| glass.filter())
			.flatten()
			.map(|filter| boost_filter(filter, boost));
		let surface = layer()
			.rounded(corner)
			.when(expanded, |surface| surface.rounded_smoothing_ios())
			.glass(glass)
			.when_some(boosted, |surface, filter| {
				surface.backdrop_filter(vec![Filter::Glass(filter)])
			})
			.border_1()
			.border_color(hud_border(&theme, false));

		let chevron = if expanded {
			Icon::ChevronUp
		} else {
			Icon::ChevronDown
		};
		let toggle_label: SharedString =
			format!("{} {}", if expanded { "Hide" } else { "Show" }, self.noun).into();
		// Fixed size and offset: the header never shrinks when the detail lays out below it, and it
		// does not move with `open`, so the title keeps the same offset from the pill's top edge.
		let header = div()
			.id((self.id.clone(), "header"))
			.relative()
			.h(theme.px(hud::PILL_HEIGHT))
			.flex_shrink_0()
			.flex()
			.flex_row()
			.items_center()
			.gap(px(5.))
			.px(theme.px(hud::HORIZONTAL_PADDING))
			.child(self.icon.render(theme.pt(11.), accent_foreground))
			.child(div().flex_1().min_w_0().child((self.title)()))
			.when(can_expand, |header| {
				header
					.child(div().w(px(6.)))
					.child(chevron.render(theme.pt(10.), accent_foreground))
					.role(Role::Button)
					.aria_label(toggle_label)
					.aria_expanded(expanded)
					.when(!self.enabled, |header| header.aria_disabled(true))
			})
			.when_some(self.on_toggle.filter(|_| self.enabled), |header, on_toggle| {
				header.on_click(on_toggle)
			})
			.when(needs_probe(collapsed_width, settled_closed), |header| {
				// Collapsed layout, invisible and out of flow: its width is the collapsed width.
				let probe_state = natural_width.clone();
				header.child(
					div()
						.absolute()
						.top_0()
						.left_0()
						.h(theme.px(hud::PILL_HEIGHT))
						.opacity(0.)
						.whitespace_nowrap()
						.flex()
						.flex_row()
						.items_center()
						.gap(px(5.))
						.px(theme.px(hud::HORIZONTAL_PADDING))
						.child(self.icon.render(theme.pt(11.), accent_foreground))
						.child(div().min_w_0().child((self.title)()))
						.when(can_expand, |probe| {
							probe
								.child(div().w(px(6.)))
								.child(chevron.render(theme.pt(10.), accent_foreground))
						})
						.child(
							canvas(
								move |bounds, _, cx| {
									probe_state.update(cx, |width, _| *width = Some(bounds.size.width));
								},
								|_, _, _, _| {},
							)
							.absolute()
							.size_full(),
						),
				)
			})
			.child(
				canvas(
					move |bounds, _, cx| {
						if settled_closed {
							natural_width.update(cx, |width, _| *width = Some(bounds.size.width));
						}
					},
					|_, _, _, _| {},
				)
				.absolute()
				.size_full(),
			);

		let mut pill = div()
			.id(self.id.clone())
			.relative()
			.flex()
			.flex_col()
			.text_size(theme.text_caption())
			.text_color(theme.palette.primary_text)
			.font_family(theme.sans())
			.opacity(1. - 0.38 * recede)
			.h(height)
			.when_some(width, |pill, width| pill.w(width))
			.child(surface);
		// The content clips to the animating size; the surface keeps its shadow outside it.
		let mut content = div()
			.relative()
			.size_full()
			.flex()
			.flex_col()
			.overflow_hidden()
			.child(header);
		// The detail fades in as the pill opens and out as it closes, clipped by the shrinking size.
		if let (Some(shown), Some(detail)) = (detail_presence, self.detail) {
			content = content.child(
				div()
					.relative()
					.px(px(14.))
					.pt(lerp(px(6.), px(10.), open.clamp(0., 1.)))
					.pb(px(10.))
					.opacity(shown.clamp(0., 1.))
					.mt(px(8. * (1. - shown.clamp(0., 1.))))
					.child(detail),
			);
		}
		pill = pill.child(content);
		// Scale around the bottom edge, where the pill rises from the control bar.
		scale_effect(scale, pill).anchor(ScaleAnchor::BottomCenter)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn width_morphs_from_collapsed_when_known() {
		assert_eq!(morph_width(Some(px(100.)), px(300.), 0.), px(100.));
		assert_eq!(morph_width(Some(px(100.)), px(300.), 0.5), px(200.));
		assert_eq!(morph_width(Some(px(100.)), px(300.), 1.), px(300.));
	}

	#[test]
	fn width_holds_expanded_when_collapsed_unknown() {
		assert_eq!(morph_width(None, px(300.), 0.5), px(300.));
		assert_eq!(morph_width(None, px(300.), 0.), px(300.));
	}

	#[test]
	fn settled_closed_follows_the_spring_not_the_distance() {
		assert!(is_settled_closed(false, false));
		// Collapsing and still moving, however close to 0: the width constraint stays.
		assert!(!is_settled_closed(false, true));
		assert!(!is_settled_closed(true, false));
	}

	#[test]
	fn probe_only_while_unknown_and_not_settled_closed() {
		assert!(needs_probe(None, false));
		assert!(!needs_probe(None, true));
		assert!(!needs_probe(Some(px(90.)), false));
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
			.rounded(
				theme
					.style
					.radii
					.capsule_radius(theme.px(control::HEIGHT_REGULAR) + px(32.)),
			)
			.font_family(theme.sans())
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
				bar.child(progress_outline(mode).glint(active).animated(motion).theme(theme))
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
			.font_family(theme.sans())
			.gap(px(2.))
			.child(
				div()
					.text_size(if self.emphasized {
						theme.px(tokens::text::STATUS_PERCENT)
					} else {
						theme.px(tokens::text::STATUS_TITLE)
					})
					.font_weight(FontWeight::SEMIBOLD)
					.text_color(theme.palette.primary_text)
					.truncate()
					.child(self.title),
			)
			.when_some(self.detail, |this, detail| {
				this.child(
					div()
						.text_size(theme.text_caption())
						.text_color(theme.palette.secondary_text)
						.truncate()
						.child(detail),
				)
			})
	}
}

theme_override!(Pill, ControlBar);
