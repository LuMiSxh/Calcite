// SPDX-License-Identifier: MPL-2.0

//! Per-control interaction state: hover, press, bounds, pointer, and the springs that turn
//! them into scale and light. Port of `ActionPressStyle` and `PointerSheen`.
//!
//! Keyboard parity lives here too: [`KeyboardFocus`] (`KeyboardFocusIndicator`) makes a
//! control a tab stop with the accent focus ring, and [`init_keyboard`] routes Tab, Return
//! (the default action), and Escape (the topmost dismissible surface).
//!
//! Scale never changes layout: [`ScaleExt::scaled`] paints the whole control (surface,
//! label, icons) through a paint transform, like SwiftUI's `scaleEffect`. GPUI re-renders
//! while a spring moves and stops when it settles.

mod animate;
mod clickable;
mod focus;
mod frame;
mod keyboard;
mod scale;
mod state;
#[cfg(test)]
mod tests;

pub use animate::{layer, presence, spring_value};
pub(crate) use clickable::ClickableRow;
pub use clickable::{ControlInteraction, glow_enabled, glow_layer};
pub use focus::{FOCUS_RING_ALPHA, FocusShape, KeyboardFocus, focus_handle, focus_ring, focus_visible};
pub use frame::{ControlFrame, Feedback, pressed_shadows};
pub(crate) use keyboard::{
	KeyHandler, KeyRole, arrow_direction, key_step, register_key_target, text_input_focused,
	unregister_key_target,
};
pub use keyboard::{init_keyboard, tab_step};
pub use scale::{ScaleAnchor, ScaleEffect, ScaleExt, scale_effect, scale_spring};
pub(crate) use state::row_fill;
pub use state::{ControlState, reduce_motion};
