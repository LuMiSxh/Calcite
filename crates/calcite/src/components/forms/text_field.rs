// SPDX-License-Identifier: MPL-2.0

//! The text field: [`text_field`] (`ThemedTextField`) edits an [`EditableTextState`] that the
//! view owns.

use gpui::{
	App, ElementId, Entity, Filter, FocusHandle, Focusable, IntoElement, ParentElement, Pixels, SharedString,
	Styled, Window, div, prelude::*,
};
use gpui_ce_elements::editable_text::{EditableTextState, StringStorage, actions, text_input};

use crate::{
	controls::theme_override,
	glass::Elevation,
	icons::IconRef,
	interaction::{ControlInteraction, ControlState, Feedback, KeyRole, register_key_target},
	theme::{ActiveTheme, Theme},
	tokens::{Chromatic, control, scale_alpha},
};

theme_override!(TextField);

impl Redraw {
	/// Re-renders `view` whenever `state` notifies.
	fn observing(
		state: &Entity<EditableTextState>,
		view: gpui::EntityId,
		cx: &mut gpui::Context<Self>,
	) -> Self {
		Self {
			state: state.entity_id(),
			_observe: cx.observe(state, move |_, _, cx| gpui::AppContext::notify(cx, view)),
		}
	}
}

/// Keeps a [`TextField`]'s observer of its state alive while the field is drawn. It watches
/// the state it was made for, so a field that is given another state starts a new observer.
struct Redraw {
	state: gpui::EntityId,
	_observe: gpui::Subscription,
}

impl TextField {
	/// The accessible name of the field; the placeholder names it when there is no label.
	pub fn label(mut self, label: impl Into<SharedString>) -> Self {
		self.label = Some(label.into());
		self
	}

	pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
		self.placeholder = placeholder.into();
		self
	}

	pub fn icon(mut self, icon: impl Into<IconRef>) -> Self {
		self.icon = Some(icon.into());
		self
	}

	pub fn width(mut self, width: Pixels) -> Self {
		self.width = Some(width);
		self
	}

	/// Whether the field takes focus and input. Enabled by default.
	pub fn enabled(mut self, enabled: bool) -> Self {
		self.enabled = enabled;
		self
	}

	/// The inverse of [`TextField::enabled`].
	pub fn disabled(self, disabled: bool) -> Self {
		self.enabled(!disabled)
	}
}

impl RenderOnce for TextField {
	fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let enabled = self.enabled;
		let focus: FocusHandle = self.state.read(cx).focus_handle(cx);
		let focused = focus.is_focused(window);
		let hover_state = ControlState::of(&self.id, window, cx);
		let hover = ControlState::frame(
			&hover_state,
			Feedback::PressOnly,
			true,
			theme.reduce_motion,
			window,
			cx,
		);
		// The text notifies only its own state, and a cached view around the field would not
		// re-render for that. A keyed observer re-renders the view the field is drawn in, as
		// `use_keyed_state` does for the states it creates.
		let view = window.current_view();
		let state = self.state.clone();
		let redraw = window.use_keyed_state(self.id.clone(), cx, {
			let state = state.clone();
			move |_, cx| Redraw::observing(&state, view, cx)
		});
		redraw.update(cx, |redraw, cx| {
			if redraw.state != state.entity_id() {
				*redraw = Redraw::observing(&state, view, cx);
			}
		});
		// A single-line field has no use for Return, so it reaches the default action.
		register_key_target(
			&self.id,
			KeyRole::PassReturn,
			None,
			Some(focus.clone()),
			window,
			cx,
		);
		let chromatic = Chromatic::enabled(theme.accent);
		let glass = theme
			.surface(Some(chromatic.fill))
			.elevation(Elevation::None)
			.overlay(hover.hover_fill(theme.ink(1.)));
		let border = if focused { theme.accent } else { chromatic.border };
		let icon_color = if focused {
			theme.legible(theme.accent)
		} else {
			theme.palette.secondary_text
		};
		let field = div()
			.id(self.id.clone())
			.flex()
			.flex_row()
			.items_center()
			.font_family(theme.sans())
			.gap(theme.pt(7.))
			.px(theme.pt(12.))
			.h(theme.px(control::HEIGHT_REGULAR))
			.rounded(
				theme
					.style
					.radii
					.capsule_radius(theme.px(control::HEIGHT_REGULAR)),
			)
			.border_1()
			.border_color(border)
			.text_size(theme.text_body())
			.text_color(theme.palette.primary_text)
			.control_state(&hover_state, false)
			.when_some(
				self.label
					.clone()
					.or_else(|| (!self.placeholder.is_empty()).then(|| self.placeholder.clone())),
				|field, name| field.aria_label(name),
			)
			.when_some(self.width, |field, width| field.w(width))
			// The editor element does not join the tab order itself; the capsule joins it
			// with the editor's handle, and focus still lands on the editor.
			.when(enabled, |field| {
				field
					.track_focus(&focus.clone().tab_stop(true))
					// The editor would swallow Tab for a tab character; a single line moves focus instead.
					.capture_action(|_: &actions::Tab, window, cx| {
						cx.stop_propagation();
						// Confined to the topmost modal, like Tab on any other control.
						crate::interaction::tab_step(true, window, cx);
					})
			})
			.when(!enabled, |field| {
				field
					.aria_disabled(true)
					.opacity(control::DISABLED_FOREGROUND_OPACITY)
			})
			.when_some(self.icon, |field, icon| {
				field.child(icon.render(theme.pt(11.), icon_color))
			})
			.child(
				text_input((self.id, "input"))
					.accepts_input(enabled)
					.state(self.state.downgrade())
					.placeholder(self.placeholder)
					.placeholder_color(theme.palette.secondary_text)
					.caret_color(gpui::rgb_to_hsla(theme.legible(theme.accent)))
					.selection_color(gpui::rgb_to_hsla(scale_alpha(theme.accent, 0.35)))
					.flex_1()
					.min_w_0()
					.whitespace_nowrap()
					.overflow_x_scroll(),
			);
		match glass.filter() {
			Some(filter) => field.backdrop_filter(vec![Filter::Glass(filter)]),
			None => field.bg(glass.fill()),
		}
	}
}

/// A single-line input in the accent capsule (`ThemedTextField`).
///
/// The text lives in an [`EditableTextState`] the view owns; create it with
/// [`text_state`] and read it with `state.read(cx).as_str()`, or subscribe to its
/// `TextChanged` event.
#[derive(IntoElement)]
pub struct TextField {
	id: ElementId,
	state: Entity<EditableTextState>,
	placeholder: SharedString,
	icon: Option<IconRef>,
	width: Option<Pixels>,
	label: Option<SharedString>,
	enabled: bool,
	theme: Option<Theme>,
}

/// A text field editing `state`.
pub fn text_field(id: impl Into<ElementId>, state: &Entity<EditableTextState>) -> TextField {
	TextField {
		id: id.into(),
		state: state.clone(),
		placeholder: SharedString::default(),
		icon: None,
		width: None,
		label: None,
		enabled: true,
		theme: None,
	}
}

/// Creates the state of a text field with `initial` text.
pub fn text_state(initial: &str, cx: &mut App) -> Entity<EditableTextState> {
	let storage = StringStorage::from(initial);
	cx.new(|cx| EditableTextState::new(storage, cx))
}
