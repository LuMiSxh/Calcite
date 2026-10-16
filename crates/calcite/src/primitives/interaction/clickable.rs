// SPDX-License-Identifier: MPL-2.0

//! Clickable rows and the pointer wiring that drives their press and hover.

use gpui::{
	App, Div, ElementId, Entity, Filter, FocusHandle, MouseButton, ParentElement, Rgba, Stateful,
	StatefulInteractiveElement, Styled, Window, canvas,
};

use crate::{
	glass::flat_surface,
	theme::Theme,
	tokens::{self, with_alpha},
};

use super::*;

impl ClickableRow {
	pub(crate) fn new(id: &ElementId, theme: &Theme, window: &mut Window, cx: &mut App) -> Self {
		let state = ControlState::of(id, window, cx);
		let focus = focus_handle(id, window, cx);
		let frame = ControlState::frame(&state, Feedback::PressOnly, true, theme.reduce_motion, window, cx);
		Self { state, focus, frame }
	}

	/// Whether the row is a tab stop (the default). A roving group (a rail, a radio group) flags
	/// only its current entry, so Tab visits the group once and the arrow keys move inside it.
	pub(crate) fn tab_stop(mut self, tab_stop: bool) -> Self {
		self.focus = self.focus.tab_stop(tab_stop);
		self
	}

	/// The row's focus handle, for the group that routes the arrow keys.
	pub(crate) fn focus(&self) -> &FocusHandle {
		&self.focus
	}

	/// The row's background for a selection amount of `selected` (0..=1).
	pub(crate) fn fill(&self, theme: &Theme, selected: f32) -> Rgba {
		row_fill(theme.palette.ink, theme.accent, selected, self.frame.light)
	}

	/// Makes `row` respond: hover tracking and the focus ring of `shape`.
	pub(crate) fn wire(
		&self,
		row: Stateful<Div>,
		shape: FocusShape,
		theme: &Theme,
		window: &Window,
	) -> Stateful<Div> {
		row.control_state(&self.state, false)
			.keyboard_focus(&self.focus, shape, theme, window)
	}
}

/// What every clickable row shares: its hover light, selection fill, and
/// keyboard focus ring. Build it per frame, read [`ClickableRow::fill`], then call
/// [`ClickableRow::wire`] on the row's element.
pub(crate) struct ClickableRow {
	state: Entity<ControlState>,
	focus: FocusHandle,
	pub frame: ControlFrame,
}

/// Whether a control carries the pointer light. `setting` is the component's `.glow(bool)`
/// override and `None` follows [`Style::pointer_glow`](crate::style::Style::pointer_glow).
/// Reduce Transparency and compact controls never glow.
pub fn glow_enabled(theme: &Theme, setting: Option<bool>, compact: bool) -> bool {
	setting.unwrap_or(theme.style.pointer_glow) && !compact && !theme.reduce_transparency()
}

/// The pointer light as its own layer over a control's surface: transparent glass that only
/// adds the light. It fades with `frame.light`, so it rises on hover and falls away on
/// leave. `None` while the pointer is outside the control. The caller rounds it like the
/// surface and tracks the pointer with `control_state(&state, true)`.
pub fn glow_layer(frame: &ControlFrame, tint: Rgba) -> Option<gpui::Div> {
	let pointer = frame.pointer?;
	if frame.light <= 0. {
		return None;
	}
	Some(layer().backdrop_filter(vec![Filter::Glass(flat_surface(
		tokens::CLEAR,
		Some(pointer),
		with_alpha(tint, frame.light),
		frame.pointer_radius,
	))]))
}

/// Wires a control's hover, press, pointer, and bounds into its [`ControlState`].
pub trait ControlInteraction: StatefulInteractiveElement + ParentElement + Sized {
	/// When the pointer leaves this whole element, drops the hover and press of every state in
	/// `states`. A fast exit can skip a child's own hover-out event, which would leave it lit.
	fn clear_hover_on_leave(self, states: Vec<Entity<ControlState>>) -> Self {
		self.on_hover(move |hovered, _, cx| {
			if !*hovered {
				for state in &states {
					state.update(cx, |state, cx| {
						if state.clear_hover() {
							cx.notify();
						}
					});
				}
			}
		})
	}

	fn control_state(self, state: &Entity<ControlState>, tracks_pointer: bool) -> Self {
		let hover = state.clone();
		let press = state.clone();
		let release = state.clone();
		let release_out = state.clone();
		let pointer = state.clone();
		let probe = state.clone();
		let key = state.clone();
		let this = self
			.on_hover(move |hovered, _, cx| {
				hover.update(cx, |state, cx| {
					state.hovered = *hovered;
					if !*hovered {
						state.pressed = false;
					}
					cx.notify();
				});
			})
			.on_mouse_down(MouseButton::Left, move |_, _, cx| {
				press.update(cx, |state, cx| {
					state.pressed = true;
					cx.notify();
				});
			})
			.on_mouse_up(MouseButton::Left, move |_, _, cx| {
				release.update(cx, |state, cx| {
					state.pressed = false;
					cx.notify();
				});
			})
			// Released outside after a drag off a control that never saw a hover edge.
			.on_mouse_up_out(MouseButton::Left, move |_, _, cx| {
				release_out.update(cx, |state, cx| {
					if state.pressed {
						state.pressed = false;
						cx.notify();
					}
				});
			})
			// Space and Return activate a focused control without the mouse: show the press.
			.on_key_down(move |event, window, cx| {
				let stroke = &event.keystroke;
				if event.is_held
					|| stroke.modifiers.modified()
					|| !matches!(stroke.key.as_str(), "space" | "enter")
					|| text_input_focused(window)
				{
					return;
				}
				key.update(cx, |state, cx| {
					state.pulse();
					cx.notify();
				});
			});
		let this = if tracks_pointer {
			this.on_mouse_move(move |event, _, cx| {
				pointer.update(cx, |state, cx| {
					state.pointer = Some(event.position);
					if state.hovered {
						cx.notify();
					}
				});
			})
		} else {
			this
		};
		// A zero-cost probe records the laid-out bounds for scale and the centered light.
		this.child(
			canvas(
				move |bounds, _, cx| probe.update(cx, |state, _| state.bounds = Some(bounds)),
				|_, _, _, _| {},
			)
			.absolute()
			.size_full(),
		)
	}
}
impl<E: StatefulInteractiveElement + ParentElement + Sized> ControlInteraction for E {}
