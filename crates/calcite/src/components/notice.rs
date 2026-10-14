// SPDX-License-Identifier: MPL-2.0

//! Notices and small status elements: [`spinner`], [`progress_bar`], [`empty_state`]
//! (`ContentUnavailableView`), [`banner`], [`confirm_dialog`], and [`chip`] (search tags
//! and suggestions).
//!
//! ```ignore
//! banner("saved", "Preset saved").tone(BannerTone::Success).on_dismiss(..)
//! confirm_dialog("delete", "Delete preset?", "This cannot be undone.")
//!     .confirm("Delete", ConfirmRole::Destructive)
//!     .on_confirm(..)
//!     .on_cancel(..)
//! ```

use std::rc::Rc;

use gpui::{
	AnyElement, App, ClickEvent, ElementId, FontWeight, IntoElement, ParentElement, Pixels, Rgba, Role,
	SharedString, Styled, Transformation, Window, div, percentage, prelude::*, px, relative,
};

use crate::{
	continuous::ContinuousMotion,
	controls::{button, icon_button, theme_override},
	glass::{Elevation, GlassExt},
	icons::{Icon, IconRef},
	interaction::{
		ControlInteraction, ControlState, Feedback, FocusShape, KeyboardFocus, ScaleExt, focus_handle,
		glow_enabled, glow_layer, layer, presence, spring_value,
	},
	motion::{Curve, cycle},
	overlay::{floating_action_bar, modal_layer},
	surface::chip_capsule,
	theme::{ActiveTheme, Theme},
	tokens::{self, Chromatic, contrast_tint, control, over, stack, white, with_alpha},
};

type Handler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

const SPINNER_PERIOD: f32 = 1.;
const TRACK_OPACITY: f32 = 0.10;
const GLINT_WIDTH: f32 = 0.3;

/// An indeterminate spinner. Under Reduce Motion the glyph stays still.
#[derive(IntoElement)]
pub struct Spinner {
	id: ElementId,
	size: Pixels,
	color: Option<Rgba>,
	theme: Option<Theme>,
}

/// A 16 pt spinner in the secondary text color.
pub fn spinner() -> Spinner {
	Spinner {
		id: "calcite-spinner".into(),
		size: px(16.),
		color: None,
		theme: None,
	}
}

impl Spinner {
	/// Sets the animation key; sibling spinners need distinct ids.
	pub fn id(mut self, id: impl Into<ElementId>) -> Self {
		self.id = id.into();
		self
	}

	pub fn size(mut self, size: Pixels) -> Self {
		self.size = size;
		self
	}

	pub fn color(mut self, color: Rgba) -> Self {
		self.color = Some(color);
		self
	}
}

impl RenderOnce for Spinner {
	fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let glyph = Icon::LoaderCircle.render(self.size, self.color.unwrap_or(theme.palette.secondary_text));
		// Every spinner turns on one shared clock and holds its angle while paused.
		let glyph = if theme.reduce_motion {
			glyph
		} else {
			let turn = ContinuousMotion::phase("calcite-spinner", SPINNER_PERIOD, &theme, window, cx);
			glyph.with_transformation(Transformation::rotate(percentage(turn)))
		};
		div()
			.id(self.id)
			.flex_none()
			.size(self.size)
			.role(Role::ProgressIndicator)
			.aria_label("Loading")
			.child(glyph)
	}
}

/// A thin capsule progress bar.
#[derive(IntoElement)]
pub struct ProgressBar {
	id: ElementId,
	fraction: Option<f32>,
	height: Pixels,
	label: SharedString,
	theme: Option<Theme>,
}

/// A 4 pt bar. `Some(fraction)` fills 0..=1; `None` slides an indeterminate glint.
pub fn progress_bar(id: impl Into<ElementId>, fraction: Option<f32>) -> ProgressBar {
	ProgressBar {
		id: id.into(),
		fraction,
		height: px(4.),
		label: "Progress".into(),
		theme: None,
	}
}

impl ProgressBar {
	pub fn height(mut self, height: Pixels) -> Self {
		self.height = height;
		self
	}

	/// The accessible name, "Progress" by default.
	pub fn label(mut self, label: impl Into<SharedString>) -> Self {
		self.label = label.into();
		self
	}
}

impl RenderOnce for ProgressBar {
	fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let accent = theme.accent;
		let track = div()
			.id(self.id.clone())
			.relative()
			.w_full()
			.h(self.height)
			.flex_none()
			.overflow_hidden()
			.rounded_full()
			.bg(white(TRACK_OPACITY))
			.role(Role::ProgressIndicator)
			.aria_label(self.label);
		let fill = div().absolute().top_0().bottom_0().rounded_full().bg(accent);
		match self.fraction {
			Some(fraction) => {
				let fraction = fraction.clamp(0., 1.);
				let shown = spring_value(
					(self.id, "fill"),
					fraction,
					Curve::Progress,
					theme.reduce_motion,
					window,
					cx,
				);
				track
					.aria_numeric_value(f64::from(fraction))
					.aria_min_numeric_value(0.)
					.aria_max_numeric_value(1.)
					.child(fill.left_0().w(relative(shown.clamp(0., 1.))))
			}
			None if !theme.reduce_motion => {
				let t = ContinuousMotion::phase(
					"calcite-progress-sweep",
					cycle::progress_sweep_total(),
					&theme,
					window,
					cx,
				);
				let travel = (t * cycle::progress_sweep_total() / cycle::PROGRESS_SWEEP_TRAVEL).min(1.);
				let eased = travel * travel * (3. - 2. * travel);
				track.child(
					fill.w(relative(GLINT_WIDTH))
						.left(relative(-GLINT_WIDTH + (1. + GLINT_WIDTH) * eased)),
				)
			}
			// Without motion an indeterminate bar reads as a dimmed full fill.
			None => track.child(fill.left_0().w_full().bg(with_alpha(accent, 0.35))),
		}
	}
}

/// A centered placeholder for empty or failed content (`ContentUnavailableView`).
#[derive(IntoElement)]
pub struct EmptyState {
	icon: IconRef,
	title: SharedString,
	detail: Option<SharedString>,
	action: Option<AnyElement>,
	theme: Option<Theme>,
}

pub fn empty_state(icon: impl Into<IconRef>, title: impl Into<SharedString>) -> EmptyState {
	EmptyState {
		icon: icon.into(),
		title: title.into(),
		detail: None,
		action: None,
		theme: None,
	}
}

impl EmptyState {
	pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
		self.detail = Some(detail.into());
		self
	}

	pub fn action(mut self, action: impl IntoElement) -> Self {
		self.action = Some(action.into_any_element());
		self
	}
}

impl RenderOnce for EmptyState {
	fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		div()
			.font_family(theme.sans())
			.id(ElementId::Name(self.title.clone()))
			.w_full()
			.min_h(px(240.))
			.flex()
			.flex_col()
			.items_center()
			.justify_center()
			.gap(px(8.))
			.p(px(24.))
			.role(Role::Group)
			.aria_label(self.title.clone())
			.child(
				div()
					.pb(px(4.))
					.child(self.icon.render(px(40.), theme.palette.secondary_text)),
			)
			.child(
				div()
					.text_size(px(17.))
					.font_weight(FontWeight::BOLD)
					.text_color(theme.palette.primary_text)
					.text_center()
					.child(self.title),
			)
			.when_some(self.detail, |state, detail| {
				state.child(
					div()
						.max_w(px(360.))
						.text_size(tokens::text::BODY)
						.text_color(theme.palette.secondary_text)
						.text_center()
						.child(detail),
				)
			})
			.when_some(self.action, |state, action| {
				state.child(div().pt(px(6.)).child(action))
			})
	}
}

/// The semantic tone of a [`banner`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BannerTone {
	Info,
	Success,
	Warning,
	Danger,
}

impl BannerTone {
	fn color(self, theme: &Theme) -> Rgba {
		match self {
			Self::Info => theme.accent,
			Self::Success => theme.palette.success,
			Self::Warning => theme.palette.warning,
			Self::Danger => theme.palette.danger_text,
		}
	}

	fn icon(self) -> Icon {
		match self {
			Self::Info => Icon::Info,
			Self::Success => Icon::Checkmark,
			Self::Warning => Icon::Warning,
			Self::Danger => Icon::CircleAlert,
		}
	}
}

/// A glass HUD notice, for toasts and inline alerts. One line stays a capsule; with a
/// detail it uses the fixed HUD radius.
#[derive(IntoElement)]
pub struct Banner {
	id: ElementId,
	title: SharedString,
	tone: BannerTone,
	detail: Option<SharedString>,
	action: Option<AnyElement>,
	on_dismiss: Option<Handler>,
	shown: bool,
	theme: Option<Theme>,
}

pub fn banner(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Banner {
	Banner {
		id: id.into(),
		title: title.into(),
		tone: BannerTone::Info,
		detail: None,
		action: None,
		on_dismiss: None,
		shown: true,
		theme: None,
	}
}

impl Banner {
	/// Whether the banner is up. Keep rendering it and flip this to let it fade out.
	pub fn shown(mut self, shown: bool) -> Self {
		self.shown = shown;
		self
	}

	pub fn tone(mut self, tone: BannerTone) -> Self {
		self.tone = tone;
		self
	}

	pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
		self.detail = Some(detail.into());
		self
	}

	/// A trailing action, usually a compact [`button`].
	pub fn action(mut self, action: impl IntoElement) -> Self {
		self.action = Some(action.into_any_element());
		self
	}

	/// Shows a trailing close button that calls `handler`.
	pub fn on_dismiss(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
		self.on_dismiss = Some(Rc::new(handler));
		self
	}
}

impl RenderOnce for Banner {
	fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let reduce_motion = theme.reduce_motion;
		let Some(shown) = presence(
			(self.id.clone(), "calcite-banner"),
			self.shown,
			reduce_motion,
			window,
			cx,
		) else {
			return div().into_any_element();
		};
		let tint = contrast_tint(self.tone.color(&theme));
		let has_detail = self.detail.is_some();
		let text = div()
			.flex_1()
			.min_w_0()
			.flex()
			.flex_col()
			.gap(px(2.))
			.child(
				div()
					.text_size(tokens::text::BODY)
					.font_weight(FontWeight::SEMIBOLD)
					.text_color(theme.palette.primary_text)
					.child(self.title.clone()),
			)
			.when_some(self.detail, |text, detail| {
				text.child(
					div()
						.text_size(tokens::text::CAPTION)
						.text_color(theme.palette.secondary_text)
						.child(detail),
				)
			});
		div()
			.id(self.id.clone())
			.flex()
			.flex_row()
			.items_center()
			.gap(px(10.))
			.font_family(theme.sans())
			.min_h(px(40.))
			.max_w(px(480.))
			.pl(px(14.))
			.pr(px(if self.on_dismiss.is_some() { 4. } else { 14. }))
			.py(px(if has_detail { 10. } else { 4. }))
			.map(|banner| {
				if has_detail {
					banner.rounded(theme.style.radii.hud_pill).rounded_smoothing_ios()
				} else {
					banner.rounded_full()
				}
			})
			.glass_hud_at(&theme, Elevation::Raised)
			.role(match self.tone {
				BannerTone::Warning | BannerTone::Danger => Role::Alert,
				BannerTone::Info | BannerTone::Success => Role::Status,
			})
			.aria_label(self.title)
			.child(self.tone.icon().render(px(15.), tint))
			.child(text)
			.when_some(self.action, |banner, action| banner.child(action))
			.when_some(self.on_dismiss, |banner, on_dismiss| {
				banner.child(
					icon_button((self.id.clone(), "dismiss"), Icon::XMark, "Dismiss")
						.diameter(px(28.))
						.on_click(move |event, window, cx| on_dismiss(event, window, cx)),
				)
			})
			.opacity(shown.clamp(0., 1.))
			.mt(px(if reduce_motion {
				0.
			} else {
				BANNER_DROP * (1. - shown)
			}))
			.into_any_element()
	}
}

/// How far a banner travels as it appears and leaves.
const BANNER_DROP: f32 = -10.;

/// How the confirming action of a [`confirm_dialog`] reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfirmRole {
	/// The accent action.
	Default,
	/// An action that deletes or discards: the danger tone.
	Destructive,
}

/// A small confirmation dialog over a dimmed window. Clicking the dimmed area cancels.
#[derive(IntoElement)]
pub struct ConfirmDialog {
	id: ElementId,
	title: SharedString,
	message: SharedString,
	confirm_label: SharedString,
	role: ConfirmRole,
	cancel_label: SharedString,
	on_confirm: Option<Handler>,
	on_cancel: Option<Handler>,
	shown: bool,
	theme: Option<Theme>,
}

/// A confirmation with "Cancel" and "Confirm". Place it at the window root like
/// [`modal_layer`].
pub fn confirm_dialog(
	id: impl Into<ElementId>,
	title: impl Into<SharedString>,
	message: impl Into<SharedString>,
) -> ConfirmDialog {
	ConfirmDialog {
		id: id.into(),
		title: title.into(),
		message: message.into(),
		confirm_label: "Confirm".into(),
		role: ConfirmRole::Default,
		cancel_label: "Cancel".into(),
		on_confirm: None,
		on_cancel: None,
		shown: true,
		theme: None,
	}
}

impl ConfirmDialog {
	/// Whether the dialog is up; see [`ModalLayer::shown`](crate::overlay::ModalLayer::shown).
	pub fn shown(mut self, shown: bool) -> Self {
		self.shown = shown;
		self
	}

	pub fn confirm(mut self, label: impl Into<SharedString>, role: ConfirmRole) -> Self {
		self.confirm_label = label.into();
		self.role = role;
		self
	}

	pub fn cancel(mut self, label: impl Into<SharedString>) -> Self {
		self.cancel_label = label.into();
		self
	}

	pub fn on_confirm(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
		self.on_confirm = Some(Rc::new(handler));
		self
	}

	/// Runs on the cancel button and on a click on the dimmed area.
	pub fn on_cancel(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
		self.on_cancel = Some(Rc::new(handler));
		self
	}
}

impl RenderOnce for ConfirmDialog {
	fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
		use crate::controls::ThemeOverride;

		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let cancel = button((self.id.clone(), "cancel"), self.cancel_label)
			.theme(theme)
			.when_some(self.on_cancel.clone(), |cancel, on_cancel| {
				cancel.on_click(move |event, window, cx| on_cancel(event, window, cx))
			});
		let confirm = button((self.id.clone(), "confirm"), self.confirm_label).theme(theme);
		let confirm = match self.role {
			ConfirmRole::Default => confirm.accent(),
			ConfirmRole::Destructive => confirm.danger(),
		}
		.when_some(self.on_confirm, |confirm, on_confirm| {
			confirm.on_click(move |event, window, cx| on_confirm(event, window, cx))
		});
		let card = div()
			.font_family(theme.sans())
			.id((self.id.clone(), "card"))
			.w(px(420.))
			.flex()
			.flex_col()
			.gap(px(10.))
			.px(px(24.))
			.pt(px(22.))
			.pb(px(16.))
			.rounded(theme.style.radii.modal)
			.rounded_smoothing_ios()
			.bg(stack(&[theme.palette.panel_background, theme.hud_tint]))
			.border_1()
			.border_color(theme.palette.hairline)
			.shadow(vec![
				gpui::BoxShadow::new(px(0.), px(10.), tokens::black(0.45)).blur_radius(px(48.)),
			])
			.role(Role::AlertDialog)
			.aria_label(self.title.clone())
			.child(
				div()
					.text_size(px(17.))
					.font_weight(FontWeight::BOLD)
					.text_color(theme.palette.primary_text)
					.child(self.title),
			)
			.child(
				div()
					.text_size(tokens::text::BODY)
					.text_color(theme.palette.secondary_text)
					.child(self.message),
			)
			.child(
				div()
					.pt(px(8.))
					.flex()
					.flex_row()
					.justify_end()
					.child(floating_action_bar(&theme).child(cancel).child(confirm)),
			);
		modal_layer(self.id, card)
			.shown(self.shown)
			.theme(theme)
			.when_some(self.on_cancel, |layer, on_cancel| {
				layer.on_dismiss(move |event, window, cx| on_cancel(event, window, cx))
			})
	}
}

/// A small capsule for suggestions and search tags.
#[derive(IntoElement)]
pub struct Chip {
	id: ElementId,
	text: SharedString,
	selected: bool,
	glow: Option<bool>,
	on_click: Option<Handler>,
	on_remove: Option<Handler>,
	theme: Option<Theme>,
}

pub fn chip(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Chip {
	Chip {
		id: id.into(),
		text: text.into(),
		selected: false,
		glow: None,
		on_click: None,
		on_remove: None,
		theme: None,
	}
}

impl Chip {
	/// Tints the chip with the accent.
	pub fn selected(mut self, selected: bool) -> Self {
		self.selected = selected;
		self
	}

	/// Forces the pointer light on or off. Without it the chip follows
	/// [`Style::pointer_glow`](crate::style::Style::pointer_glow).
	pub fn glow(mut self, glow: bool) -> Self {
		self.glow = Some(glow);
		self
	}

	pub fn on_click(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
		self.on_click = Some(Rc::new(handler));
		self
	}

	/// Shows a trailing remove mark. Without `on_click`, the whole chip removes it, like a
	/// committed search tag.
	pub fn on_remove(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
		self.on_remove = Some(Rc::new(handler));
		self
	}
}

impl RenderOnce for Chip {
	fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let state = ControlState::of(&self.id, window, cx);
		let focus = focus_handle(&self.id, window, cx);
		let frame = ControlState::frame(&state, Feedback::Standard, true, theme.reduce_motion, window, cx);
		let (fill, border, foreground) = if self.selected {
			let accent = Chromatic::enabled(theme.accent);
			(accent.fill, accent.border, contrast_tint(theme.accent))
		} else {
			let fill = if theme.reduce_transparency() {
				theme.palette.opaque_surface
			} else {
				white(control::ENABLED_NEUTRAL_SURFACE_OPACITY)
			};
			(
				fill,
				with_alpha(theme.palette.control, control::ENABLED_NEUTRAL_BORDER_OPACITY),
				theme.palette.secondary_text,
			)
		};
		let hover = frame.hover_fill(white(1.));
		let glows = glow_enabled(&theme, self.glow, false);
		let glow = glows
			.then(|| glow_layer(&frame, foreground))
			.flatten()
			.map(|glow| glow.rounded_full());
		let surface = layer()
			.rounded_full()
			.bg(over(hover, fill))
			.border_1()
			.border_color(border);
		let removable = self.on_remove.is_some();
		let label: SharedString = if removable && self.on_click.is_none() {
			format!("Remove {} tag", self.text).into()
		} else {
			self.text.clone()
		};
		let remove = self.on_remove.clone().map(|on_remove| {
			let mark = div()
				.id((self.id.clone(), "remove"))
				.relative()
				.size(px(16.))
				.flex()
				.items_center()
				.justify_center()
				.child(Icon::XMark.render(px(9.), foreground));
			if self.on_click.is_some() {
				mark.role(Role::Button)
					.aria_label(SharedString::from(format!("Remove {}", self.text)))
					.on_click(move |event, window, cx| {
						cx.stop_propagation();
						on_remove(event, window, cx);
					})
			} else {
				mark
			}
		});
		let on_click = self.on_click.or(self.on_remove);
		chip_capsule(self.id)
			.font_family(theme.sans())
			.pr(px(if removable {
				4.
			} else {
				control::COMPACT_HORIZONTAL_PADDING
			}))
			.text_color(foreground)
			.opacity(frame.press_opacity())
			.role(Role::Button)
			.aria_selected(self.selected)
			.aria_label(label)
			.child(surface)
			.children(glow)
			.child(div().relative().child(self.text))
			.children(remove)
			.control_state(&state, glows)
			.keyboard_focus(&focus, FocusShape::Capsule, &theme, window)
			.when_some(on_click, |chip, on_click| {
				chip.on_click(move |event, window, cx| on_click(event, window, cx))
			})
			.scaled(frame.scale)
	}
}

theme_override!(Spinner, ProgressBar, EmptyState, Banner, ConfirmDialog, Chip);
