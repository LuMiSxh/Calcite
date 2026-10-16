// SPDX-License-Identifier: MPL-2.0

//! The keyboard focus ring: a tab stop that shows the accent halo when it has focus.

use gpui::{App, ElementId, FocusHandle, InteractiveElement, ParentElement, Pixels, Styled, Window, div, px};

use crate::{
	theme::Theme,
	tokens::{self, with_alpha},
};

/// The outline a focus ring follows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FocusShape {
	/// A rounded rectangle with the control's corner radius.
	Rounded(Pixels),
}

/// The ring's stroke width.
pub(super) const FOCUS_RING_WIDTH: f32 = 2.;

/// The gap between the control's edge and the ring.
pub(super) const FOCUS_RING_OFFSET: f32 = 2.;

/// How far the ring's box sits outside the control.
pub(super) const FOCUS_OUTSET: f32 = FOCUS_RING_WIDTH + FOCUS_RING_OFFSET;

/// The opacity of the focus ring: soft enough to read as a halo, strong enough to find.
pub const FOCUS_RING_ALPHA: f32 = 0.6;

/// The focus handle of the control `id`, a tab stop kept for as long as the control renders.
pub fn focus_handle(id: &ElementId, window: &mut Window, cx: &mut App) -> FocusHandle {
	// The tab order reads the handle's own flag: `.tab_index` on an element that tracks an
	// explicit handle does not reach it.
	window
		.use_keyed_state((id.clone(), "calcite-focus"), cx, |_, cx| {
			cx.focus_handle().tab_stop(true)
		})
		.read(cx)
		.clone()
}

/// Whether `handle` shows its ring: focused by the keyboard in the active window.
pub fn focus_visible(handle: &FocusHandle, window: &Window) -> bool {
	handle.is_focused(window) && window.last_input_was_keyboard() && window.is_window_active()
}

/// The soft accent ring around a focused control: 2 pt wide, 2 pt off the edge, at
/// [`FOCUS_RING_ALPHA`]. Its parent must be `relative`.
pub fn focus_ring(shape: FocusShape, theme: &Theme) -> gpui::Div {
	let outset = px(-FOCUS_OUTSET);
	let ring = div()
		.absolute()
		.top(outset)
		.bottom(outset)
		.left(outset)
		.right(outset)
		.border_2()
		.border_color(with_alpha(tokens::contrast_tint(theme.accent), FOCUS_RING_ALPHA));
	match shape {
		FocusShape::Rounded(corner) => ring.rounded(corner + px(FOCUS_OUTSET)),
	}
}

/// Port of `KeyboardFocusIndicator`: a tab stop that shows the accent ring while focused by
/// the keyboard. Space and Return then click it through GPUI's keyboard click.
pub trait KeyboardFocus: InteractiveElement + ParentElement + Sized {
	fn keyboard_focus(self, handle: &FocusHandle, shape: FocusShape, theme: &Theme, window: &Window) -> Self {
		let ring = focus_visible(handle, window).then(|| focus_ring(shape, theme));
		self.track_focus(handle)
			// Like macOS buttons, a click does not take focus, so Return keeps reaching the
			// default action and a focused text field keeps its focus.
			.capture_any_mouse_down(|_, window, _| window.prevent_default())
			.children(ring)
	}
}
impl<E: InteractiveElement + ParentElement + Sized> KeyboardFocus for E {}
