// SPDX-License-Identifier: MPL-2.0

//! Notices and small status elements: [`spinner`], [`progress_bar`], [`empty_state`]
//! (`ContentUnavailableView`), [`banner`], [`confirm_dialog`], and [`chip`] (search tags
//! and suggestions).
//!
//! ```ignore
//! banner("saved", "Preset saved").tone(Tone::Success).on_dismiss(..)
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
	controls::{Tone, button, icon_button, theme_override},
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
	tokens::{Chromatic, control, over, stack, with_alpha},
};

type Handler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

const SPINNER_PERIOD: f32 = 1.;
const TRACK_OPACITY: f32 = 0.10;
const GLINT_WIDTH: f32 = 0.3;

/// An indeterminate spinner. Under Reduce Motion the glyph stays still.
#[derive(IntoElement)]
pub struct Spinner {
	id: ElementId,
	diameter: Pixels,
	label: SharedString,
	color: Option<Rgba>,
	theme: Option<Theme>,
}

/// A 16 pt spinner in the secondary text color.
///
/// ```ignore
/// spinner().id("sync-spinner").diameter(px(20.)).label("Syncing")
/// ```
pub fn spinner() -> Spinner {
	Spinner {
		id: "calcite-spinner".into(),
		diameter: px(16.),
		label: "Loading".into(),
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

	/// The glyph's width and height.
	pub fn diameter(mut self, diameter: Pixels) -> Self {
		self.diameter = diameter;
		self
	}

	/// The accessible name, "Loading" by default.
	pub fn label(mut self, label: impl Into<SharedString>) -> Self {
		self.label = label.into();
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
		let glyph =
			Icon::LoaderCircle.render(self.diameter, self.color.unwrap_or(theme.palette.secondary_text));
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
			.size(self.diameter)
			.role(Role::ProgressIndicator)
			.aria_label(self.label)
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
///
/// ```ignore
/// progress_bar("download", Some(0.4)).label("Downloading update")
/// ```
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
			.rounded(theme.style.radii.capsule_radius(self.height))
			.bg(theme.ink(TRACK_OPACITY))
			.role(Role::ProgressIndicator)
			.aria_label(self.label);
		let fill = div()
			.absolute()
			.top_0()
			.bottom_0()
			.rounded(theme.style.radii.capsule_radius(self.height))
			.bg(accent);
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
				let track = track.aria_value("In progress");
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
			None => track
				.aria_value("In progress")
				.child(fill.left_0().w_full().bg(with_alpha(accent, 0.35))),
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

/// A centered icon, title, and optional detail and action.
///
/// ```ignore
/// empty_state(Icon::Search, "No results").detail("Try another search.")
/// ```
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
			.when_some(self.detail.clone(), |state, detail| {
				state.aria_description(detail)
			})
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
						.text_size(theme.text_body())
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

/// The glyph of a banner in `tone`: a check, a warning triangle, or an alert circle, and an
/// info mark for the accent, neutral, and custom tones.
fn tone_icon(tone: Tone) -> Icon {
	match tone {
		Tone::Success => Icon::Checkmark,
		Tone::Warning => Icon::Warning,
		Tone::Danger => Icon::CircleAlert,
		Tone::Accent | Tone::Neutral | Tone::Color(_) => Icon::Info,
	}
}

/// A glass HUD notice, for toasts and inline alerts. One line stays a capsule; with a
/// detail it uses the fixed HUD radius.
#[derive(IntoElement)]
pub struct Banner {
	id: ElementId,
	title: SharedString,
	tone: Tone,
	detail: Option<SharedString>,
	action: Option<AnyElement>,
	on_dismiss: Option<Handler>,
	shown: bool,
	theme: Option<Theme>,
}

/// A banner. Defaults: accent (informational) tone, shown.
///
/// ```ignore
/// banner("offline", "You are offline").tone(Tone::Warning).detail("Changes sync later.")
/// ```
pub fn banner(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Banner {
	Banner {
		id: id.into(),
		title: title.into(),
		tone: Tone::Accent,
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

	/// The banner's tone: the glyph and its color. Warning and danger read as alerts.
	pub fn tone(mut self, tone: Tone) -> Self {
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
		let min_height = px(40.);
		let tint = theme.legible(self.tone.text(&theme));
		let detail_text = self.detail.clone();
		let has_detail = self.detail.is_some();
		let text = div()
			.flex_1()
			.min_w_0()
			.flex()
			.flex_col()
			.gap(px(2.))
			.child(
				div()
					.text_size(theme.text_body())
					.font_weight(FontWeight::SEMIBOLD)
					.text_color(theme.palette.primary_text)
					.child(self.title.clone()),
			)
			.when_some(self.detail, |text, detail| {
				text.child(
					div()
						.text_size(theme.text_caption())
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
			.min_h(min_height)
			.max_w(px(480.))
			.pl(px(14.))
			.pr(px(if self.on_dismiss.is_some() { 4. } else { 14. }))
			.py(px(if has_detail { 10. } else { 4. }))
			.map(|banner| {
				if has_detail {
					banner.rounded(theme.style.radii.hud_pill).rounded_smoothing_ios()
				} else {
					banner.rounded(theme.style.radii.capsule_radius(min_height))
				}
			})
			.glass_hud_at(&theme, Elevation::Raised)
			.role(match self.tone {
				Tone::Warning | Tone::Danger => Role::Alert,
				_ => Role::Status,
			})
			.aria_label(self.title)
			.when_some(detail_text, |banner, detail| banner.aria_description(detail))
			.child(tone_icon(self.tone).render(px(15.), tint))
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
	fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
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
		.prominent()
		.when_some(self.on_confirm, |confirm, on_confirm| {
			confirm.on_click(move |event, window, cx| on_confirm(event, window, cx))
		});
		let card = div()
			.font_family(theme.sans())
			.id((self.id.clone(), "card"))
			.w(theme.pt(420.).min(window.viewport_size().width - px(60.)))
			.flex()
			.flex_col()
			.gap(theme.pt(10.))
			.px(theme.pt(24.))
			.pt(theme.pt(22.))
			.pb(theme.pt(16.))
			.rounded(theme.style.radii.modal)
			.rounded_smoothing_ios()
			.bg(stack(&[theme.palette.panel_background, theme.hud_tint]))
			.border_1()
			.border_color(theme.palette.hairline)
			.shadow(vec![
				gpui::BoxShadow::new(px(0.), px(10.), theme.shadow(0.45)).blur_radius(px(48.)),
			])
			.role(Role::AlertDialog)
			.aria_label(self.title.clone())
			.aria_description(self.message.clone())
			.aria_modal(true)
			.child(
				div()
					.text_size(theme.pt(17.))
					.font_weight(FontWeight::BOLD)
					.text_color(theme.palette.primary_text)
					.child(self.title),
			)
			.child(
				div()
					.text_size(theme.text_body())
					.text_color(theme.palette.secondary_text)
					.child(self.message),
			)
			.child(
				div()
					.pt(theme.pt(8.))
					.flex()
					.flex_row()
					.justify_end()
					.child(floating_action_bar().theme(theme).child(cancel).child(confirm)),
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
	enabled: bool,
	glow: Option<bool>,
	on_click: Option<Handler>,
	on_remove: Option<Handler>,
	theme: Option<Theme>,
}

/// A search tag or suggestion.
///
/// ```ignore
/// chip("rust", "Rust").selected(true).on_click(..)
/// chip("tag", "draft").on_remove(..)
/// ```
pub fn chip(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Chip {
	Chip {
		id: id.into(),
		text: text.into(),
		selected: false,
		enabled: true,
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

	/// Whether the chip responds. Enabled by default.
	pub fn enabled(mut self, enabled: bool) -> Self {
		self.enabled = enabled;
		self
	}

	/// The inverse of [`Chip::enabled`].
	pub fn disabled(self, disabled: bool) -> Self {
		self.enabled(!disabled)
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
		let enabled = self.enabled;
		let frame = ControlState::frame(
			&state,
			Feedback::Standard,
			enabled,
			theme.reduce_motion,
			window,
			cx,
		);
		let (fill, border, foreground) = if self.selected {
			let accent = Chromatic::enabled(theme.accent);
			(accent.fill, accent.border, theme.legible(theme.accent))
		} else {
			let fill = if theme.reduce_transparency() {
				theme.palette.opaque_surface
			} else {
				theme.ink(control::ENABLED_NEUTRAL_SURFACE_OPACITY)
			};
			(
				fill,
				with_alpha(theme.palette.control, control::ENABLED_NEUTRAL_BORDER_OPACITY),
				theme.palette.secondary_text,
			)
		};
		let chip_radius = theme
			.style
			.radii
			.capsule_radius(theme.px(control::HEIGHT_COMPACT));
		let hover = frame.hover_fill(theme.ink(1.));
		let glows = enabled && glow_enabled(&theme, self.glow, false);
		let glow = glows
			.then(|| glow_layer(&frame, foreground))
			.flatten()
			.map(|glow| glow.rounded(chip_radius));
		let surface = layer()
			.rounded(chip_radius)
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
		chip_capsule(self.id, &theme)
			.font_family(theme.sans())
			.pr(theme.pt(if removable {
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
			.when(enabled, |chip| {
				chip.control_state(&state, glows).keyboard_focus(
					&focus,
					FocusShape::Rounded(chip_radius),
					&theme,
					window,
				)
			})
			.when(!enabled, |chip| {
				chip.aria_disabled(true)
					.cursor_default()
					.opacity(control::DISABLED_FOREGROUND_OPACITY)
			})
			.when_some(on_click.filter(|_| enabled), |chip, on_click| {
				chip.on_click(move |event, window, cx| on_click(event, window, cx))
			})
			.scaled(frame.scale)
	}
}

theme_override!(Spinner, ProgressBar, EmptyState, Banner, ConfirmDialog, Chip);
