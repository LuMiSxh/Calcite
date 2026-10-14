// SPDX-License-Identifier: MPL-2.0

//! Modal presentation: [`modal`] (`ThemedModalView`), [`floating_action_bar`]
//! (`FloatingActionBar`), [`done_button`] (`FloatingDoneButton`), and [`modal_layer`],
//! which dims the window and centers the modal.
//!
//! Escape calls the layer's `on_dismiss`, and Return clicks the `done_button`, once
//! [`init_keyboard`](crate::interaction::init_keyboard) runs. While a layer is up, Tab and
//! Shift-Tab cycle inside it, and focus returns to the control that had it when the layer leaves.
//!
//! ```ignore
//! modal_layer("update", modal("update-sheet", "Update available")
//!         .child(div().child("Version 0.8.0 is ready to install."))
//!         .action(button("later", "Later"))
//!         .action(done_button("install").title("Install")))
//!     .on_dismiss(cx.listener(|this, _, _, cx| this.close(cx)))
//! ```

use gpui::{
	Animation, AnimationExt, AnyElement, AnyWindowHandle, App, AsyncApp, ClickEvent, Div, ElementId,
	FontWeight, IntoElement, MouseButton, ParentElement, Pixels, Role, SharedString, Styled, Window, div,
	linear_color_stop, linear_gradient, prelude::*, px, size,
};
use smallvec::SmallVec;
use std::rc::Rc;

use crate::{
	controls::{Button, SharedClickHandler, button, theme_override},
	glass::{Elevation, GlassExt},
	icons::Icon,
	interaction::{KeyHandler, KeyRole, presence, register_key_target, unregister_key_target},
	motion::Curve,
	theme::{ActiveTheme, Theme},
	tokens::{self, scale_alpha, stack},
};

/// The floating glass container of modal and onboarding actions: a capsule row. It reads the
/// theme when it renders; style it like any GPUI element.
pub fn floating_action_bar() -> FloatingActionBar {
	FloatingActionBar {
		base: div(),
		children: SmallVec::new(),
		theme: None,
	}
}

/// The bar of [`floating_action_bar`].
#[derive(IntoElement)]
pub struct FloatingActionBar {
	base: Div,
	children: SmallVec<[AnyElement; 3]>,
	theme: Option<Theme>,
}

impl FloatingActionBar {
	/// Uses `theme` instead of the app's.
	pub fn theme(mut self, theme: Theme) -> Self {
		self.theme = Some(theme);
		self
	}
}

impl Styled for FloatingActionBar {
	fn style(&mut self) -> &mut gpui::StyleRefinement {
		self.base.style()
	}
}

impl ParentElement for FloatingActionBar {
	fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
		self.children.extend(elements);
	}
}

impl RenderOnce for FloatingActionBar {
	fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		self.base
			.flex()
			.flex_row()
			.items_center()
			.gap(px(12.))
			.p(px(10.))
			.rounded_full()
			.glass_hud_at(&theme, Elevation::Resting)
			.children(self.children)
	}
}

/// The standard confirmation action of a floating footer. Return clicks it.
pub fn done_button(id: impl Into<ElementId>) -> Button {
	button(id, "Done")
		.accent()
		.large()
		.icon(Icon::Checkmark)
		.default_action()
}

/// A modal with a branded header, scrolling content, and a floating action bar.
#[derive(IntoElement)]
pub struct Modal {
	id: ElementId,
	title: SharedString,
	width: Pixels,
	height: Pixels,
	children: SmallVec<[AnyElement; 4]>,
	actions: SmallVec<[AnyElement; 3]>,
	theme: Option<Theme>,
}

/// A modal titled `title`, 560 × 420 by default and clamped to the window.
pub fn modal(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Modal {
	Modal {
		id: id.into(),
		title: title.into(),
		width: px(560.),
		height: px(420.),
		children: SmallVec::new(),
		actions: SmallVec::new(),
		theme: None,
	}
}

impl Modal {
	/// The preferred size, clamped to the window when it renders.
	pub fn dimensions(self, width: Pixels, height: Pixels) -> Self {
		self.width(width).height(height)
	}

	pub fn width(mut self, width: Pixels) -> Self {
		self.width = width;
		self
	}

	pub fn height(mut self, height: Pixels) -> Self {
		self.height = height;
		self
	}

	/// Adds an action to the floating footer.
	pub fn action(mut self, action: impl IntoElement) -> Self {
		self.actions.push(action.into_any_element());
		self
	}
}

impl ParentElement for Modal {
	fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
		self.children.extend(elements);
	}
}

impl RenderOnce for Modal {
	fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let reduce_motion = theme.reduce_motion;
		// Clamp to the window with a 30 pt margin, never below 480 × 320.
		let viewport = window.viewport_size();
		let modal_size = size(
			self.width.min(viewport.width - px(60.)).max(px(480.)),
			self.height.min(viewport.height - px(60.)).max(px(320.)),
		);
		let background = stack(&[theme.palette.panel_background, theme.hud_tint]);
		let entrance = Animation::new(Curve::Present.timed(reduce_motion));
		let header = div()
			.flex()
			.flex_col()
			.gap(px(10.))
			.px(px(24.))
			.pt(px(22.))
			.pb(px(16.))
			.child(
				div()
					.text_size(px(17.))
					.font_weight(FontWeight::BOLD)
					.child(self.title.clone()),
			)
			.child(
				div()
					.flex()
					.flex_row()
					.items_center()
					.gap(px(8.))
					.child(
						div()
							.w(px(48.))
							.h(px(2.))
							.bg(scale_alpha(theme.palette.control, 0.62)),
					)
					.child(div().flex_1().h(px(1.)).bg(theme.palette.hairline)),
			)
			.with_animation((self.id.clone(), "header"), entrance.clone(), |header, t| {
				// A paint-time shift, not a margin: layout does not rerun while it plays.
				header.opacity(t).relative().top(px(6. * (1. - t)))
			});
		let content = div()
			.id((self.id.clone(), "content"))
			.flex_1()
			.min_h_0()
			.overflow_y_scroll()
			.child(
				div()
					.px(px(24.))
					.pt(px(2.))
					.pb(px(96.))
					.flex()
					.flex_col()
					.gap(px(12.))
					.text_size(tokens::text::BODY)
					.children(self.children)
					.with_animation((self.id.clone(), "body"), entrance, |body, t| {
						body.opacity(t).relative().top(px(14. * (1. - t)))
					}),
			);
		let footer = div()
			.absolute()
			.left_0()
			.right_0()
			.bottom_0()
			.pt(px(30.))
			.pr(px(24.))
			.pb(px(18.))
			.flex()
			.flex_row()
			.justify_end()
			// overflow_hidden clips to the rectangle, not the rounded corners, so the footer's own
			// fill follows the card's bottom corners.
			.rounded_bl(theme.style.radii.modal)
			.rounded_br(theme.style.radii.modal)
			.rounded_smoothing_ios()
			.bg(if theme.reduce_transparency() {
				tokens::with_alpha(theme.palette.panel_background, 0.9).into()
			} else {
				linear_gradient(
					180.,
					linear_color_stop(tokens::with_alpha(theme.palette.panel_background, 0.), 0.),
					linear_color_stop(tokens::with_alpha(theme.palette.panel_background, 0.85), 1.),
				)
			})
			.when(!self.actions.is_empty(), |footer| {
				footer.child(floating_action_bar().theme(theme).children(self.actions))
			});
		div()
			.id(self.id)
			.relative()
			.font_family(theme.sans())
			.w(modal_size.width)
			.h(modal_size.height)
			.flex()
			.flex_col()
			.overflow_hidden()
			.rounded(theme.style.radii.modal)
			.rounded_smoothing_ios()
			// Glass when transparency is allowed; an opaque panel under Reduce Transparency.
			.map(|modal| {
				if theme.reduce_transparency() {
					modal
						.bg(background)
						.border_1()
						.border_color(theme.palette.hairline)
				} else {
					modal.glass_hud_at(&theme, Elevation::Raised)
				}
			})
			.shadow(vec![
				gpui::BoxShadow::new(px(0.), px(10.), theme.shadow(0.45)).blur_radius(px(48.)),
			])
			.text_color(theme.palette.primary_text)
			.role(Role::Dialog)
			.aria_label(self.title)
			.child(header)
			.child(content)
			.child(footer)
	}
}

/// Dims the window ([`Theme::scrim`]) and centers `child`. A click on the dimmed area or Escape
/// calls `on_dismiss`; clicks inside the child stay inside it. The layer takes keyboard
/// focus when it appears, so Return and Tab stop reaching the page below; Tab cycles inside
/// the layer, and focus goes back to where it was when the layer is hidden or no longer rendered.
#[derive(IntoElement)]
pub struct ModalLayer {
	id: ElementId,
	child: AnyElement,
	theme: Option<Theme>,
	on_dismiss: Option<SharedClickHandler>,
	shown: bool,
}

pub fn modal_layer(id: impl Into<ElementId>, child: impl IntoElement) -> ModalLayer {
	ModalLayer {
		id: id.into(),
		child: child.into_any_element(),
		theme: None,
		on_dismiss: None,
		shown: true,
	}
}

impl ModalLayer {
	/// Whether the layer is up. Keep rendering the layer and flip this instead of dropping
	/// it: the sheet then fades out on [`Curve::Dismiss`] before it disappears.
	pub fn shown(mut self, shown: bool) -> Self {
		self.shown = shown;
		self
	}

	pub fn on_dismiss(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
		self.on_dismiss = Some(Rc::new(handler));
		self
	}
}

/// Where focus goes back to when the layer leaves. Dropping it while the layer is still up (the
/// app stopped rendering the layer instead of hiding it) restores focus on the next turn of the
/// event loop, the same way hiding does.
struct Opener {
	previous: Option<gpui::FocusHandle>,
	layer: gpui::FocusHandle,
	window: AnyWindowHandle,
	app: AsyncApp,
}

impl Drop for Opener {
	fn drop(&mut self) {
		let Some(previous) = self.previous.take() else {
			return;
		};
		let (layer, window) = (self.layer.clone(), self.window);
		self.app
			.spawn(async move |cx| {
				cx.update(|cx| {
					window
						.update(cx, |_, window, cx| {
							if layer.contains_focused(window, cx) || window.focused(cx).is_none() {
								window.focus(&previous, cx);
							}
						})
						.ok();
				});
			})
			.detach();
	}
}

impl RenderOnce for ModalLayer {
	fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let reduce_motion = theme.reduce_motion;
		let presented = window.use_keyed_state((self.id.clone(), "calcite-presented"), cx, |_, _| false);
		let focus = window
			.use_keyed_state((self.id.clone(), "calcite-layer-focus"), cx, |_, cx| {
				cx.focus_handle()
			})
			.read(cx)
			.clone();
		let opener = window.use_keyed_state((self.id.clone(), "calcite-opener"), cx, |window, cx| Opener {
			previous: None,
			layer: focus.clone(),
			window: window.window_handle(),
			app: cx.to_async(),
		});
		let Some(t) = presence(
			(self.id.clone(), "calcite-layer"),
			self.shown,
			reduce_motion,
			window,
			cx,
		) else {
			presented.update(cx, |presented, _| *presented = false);
			unregister_key_target(&self.id, KeyRole::Dismiss, window, cx);
			unregister_key_target(&self.id, KeyRole::TabScope, window, cx);
			return div().into_any_element();
		};
		// `focus` is focusable but not a tab stop: Tab from the layer moves into the modal.
		if self.shown {
			if !*presented.read(cx) {
				presented.update(cx, |presented, _| *presented = true);
				let previous = window.focused(cx);
				opener.update(cx, |opener, _| opener.previous = previous);
				let focus = focus.clone();
				// Focus moves after this frame, once the layer is in the dispatch tree.
				window.defer(cx, move |window, cx| window.focus(&focus, cx));
			}
			let dismiss = self.on_dismiss.clone().map(|on_dismiss| {
				Rc::new(move |window: &mut Window, cx: &mut App| {
					on_dismiss(&ClickEvent::default(), window, cx)
				}) as KeyHandler
			});
			register_key_target(
				&self.id,
				KeyRole::Dismiss,
				dismiss,
				Some(focus.clone()),
				window,
				cx,
			);
			register_key_target(&self.id, KeyRole::TabScope, None, Some(focus.clone()), window, cx);
		} else {
			// Leaving: Escape and Tab no longer belong to the layer, and focus goes back.
			unregister_key_target(&self.id, KeyRole::Dismiss, window, cx);
			unregister_key_target(&self.id, KeyRole::TabScope, window, cx);
			if let Some(previous) = opener.update(cx, |opener, _| opener.previous.take()) {
				let layer = focus.clone();
				window.defer(cx, move |window, cx| {
					// Only if focus is still ours: a click elsewhere has moved it on.
					if layer.contains_focused(window, cx) || window.focused(cx).is_none() {
						window.focus(&previous, cx);
					}
				});
			}
		}
		let rise = if reduce_motion { 0. } else { SHEET_RISE * (1. - t) };
		div()
			.id(self.id.clone())
			.track_focus(&focus)
			.absolute()
			.top_0()
			.left_0()
			.size_full()
			.flex()
			.items_center()
			.justify_center()
			.bg(theme.scrim())
			.opacity(t.clamp(0., 1.))
			.occlude()
			// A leaving layer still blocks the window, but a click no longer dismisses again.
			.when_some(self.on_dismiss.filter(|_| self.shown), |layer, on_dismiss| {
				layer.on_click(move |event, window, cx| on_dismiss(event, window, cx))
			})
			.child(
				div()
					.id((self.id.clone(), "content"))
					// Keep clicks inside the modal from reaching the dismiss handler.
					.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
					.on_click(|_, _, cx| cx.stop_propagation())
					.relative()
					.top(px(rise))
					.child(self.child),
			)
			.into_any_element()
	}
}

/// How far a presented sheet rises into place.
const SHEET_RISE: f32 = 24.;

theme_override!(Modal, ModalLayer);
