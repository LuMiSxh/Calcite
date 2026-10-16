// SPDX-License-Identifier: MPL-2.0

//! Form controls: [`toggle`] (`SettingsToggle`, in the elongated macOS 26 shape), [`slider`] (`SettingsSlider`),
//! [`segmented`] (`AdaptiveSegmentedControl`), and [`text_field`] (`ThemedTextField`).
//!
//! The controls are stateless: the view owns the value and passes a change handler, like a
//! SwiftUI binding split into a value and a setter.
//!
//! ```ignore
//! toggle("auto-update", self.auto_update).label("Automatic updates")
//!     .on_change(cx.listener(|this, on: &bool, _, cx| { this.auto_update = *on; cx.notify(); }))
//! slider("volume", self.volume).step(0.05).leading_icon(Lucide::Volume).trailing_icon(Lucide::Volume2)
//!     .ticks(11).value_label(true)
//!     .on_change(cx.listener(|this, value: &f32, _, cx| { this.volume = *value; cx.notify(); }))
//! segmented("mode", self.mode, [(Mode::Windowed, "Windowed"), (Mode::Fullscreen, "Fullscreen")])
//!     .on_change(cx.listener(|this, mode: &Mode, _, cx| { this.mode = *mode; cx.notify(); }))
//! ```
//!
//! Every control is a tab stop with the accent focus ring. Space and Return flip a toggle, and a
//! press can drag its knob to either side. The arrow keys step a slider, which also takes Home,
//! End, PageUp, and PageDown; they move a segmented selection too.

use gpui::{App, Window};

mod segmented;
mod slider;
mod text_field;
mod toggle;

pub use segmented::{Segmented, segmented};
pub use slider::{Slider, slider};
pub use text_field::{TextField, text_field, text_state};
pub use toggle::{Toggle, toggle};

/// A shared change handler, for controls that hand it to several listeners.
pub(crate) type SharedChangeHandler<T> = std::rc::Rc<dyn Fn(&T, &mut Window, &mut App) + 'static>;

type ChangeHandler<T> = Box<dyn Fn(&T, &mut Window, &mut App) + 'static>;
