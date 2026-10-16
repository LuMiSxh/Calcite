// SPDX-License-Identifier: MPL-2.0

//! What a control paints at one frame: its feedback, its shadows, and its press depth.

use gpui::{BoxShadow, Pixels, Point, Rgba, point};

use crate::tokens::{control, with_alpha};

impl Feedback {
	pub(super) fn scales(self) -> (f32, f32) {
		match self {
			Self::Standard => (1.025, 0.95),
			Self::Subtle => (1.01, 0.97),
			Self::PressOnly => (1., 0.97),
		}
	}

	/// How deep a press is at `scale`: 0 at rest or hovered, 1 fully pressed (it may pass 1 while
	/// the release spring is pulled below the pressed scale).
	pub fn press_depth(self, scale: f32) -> f32 {
		let (_, pressed) = self.scales();
		((1. - scale) / (1. - pressed)).clamp(0., 1.)
	}
}

impl ControlFrame {
	/// The control's opacity this frame: dipped while pressed (`ActionPressStyle`).
	pub fn press_opacity(&self) -> f32 {
		if self.pressed {
			control::PRESSED_OPACITY
		} else {
			1.
		}
	}

	/// `shadows` for this frame: a pressed control sits closer to its backdrop, so the blur shrinks
	/// to 0.6x and the vertical offset to 0.5x at full press (see [`pressed_shadows`]).
	pub fn shadows(&self, shadows: Vec<BoxShadow>) -> Vec<BoxShadow> {
		pressed_shadows(shadows, self.press)
	}

	/// The hover overlay of `tint` (white on neutral surfaces, the accent on tinted ones): the
	/// shared hover strength faded with the hover curve.
	pub fn hover_fill(&self, tint: Rgba) -> Rgba {
		with_alpha(tint, control::HOVER_OPACITY * self.light)
	}
}

/// Blur radius multiplier of a fully pressed shadow.
const PRESSED_SHADOW_BLUR: f32 = 0.6;

/// Vertical offset multiplier of a fully pressed shadow.
const PRESSED_SHADOW_OFFSET: f32 = 0.5;

/// `shadows` pulled in by a press of depth `press` (0..=1): blur toward 0.6x, y offset toward 0.5x.
pub fn pressed_shadows(shadows: Vec<BoxShadow>, press: f32) -> Vec<BoxShadow> {
	let press = if press.is_nan() { 0. } else { press.clamp(0., 1.) };
	if press == 0. {
		return shadows;
	}
	let blur = 1. - (1. - PRESSED_SHADOW_BLUR) * press;
	let offset = 1. - (1. - PRESSED_SHADOW_OFFSET) * press;
	shadows
		.into_iter()
		.map(|shadow| BoxShadow {
			offset: point(shadow.offset.x, shadow.offset.y * offset),
			blur_radius: shadow.blur_radius * blur,
			..shadow
		})
		.collect()
}

/// Press and hover feedback (`ActionPressStyle.Intensity`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Feedback {
	/// Compact buttons and pills: 0.95 pressed, 1.025 hovered.
	Standard,
	/// Large tiles and cards: 0.97 pressed, 1.01 hovered.
	Subtle,
	/// List rows: 0.97 pressed, no hover lift.
	PressOnly,
}

/// What a control looks like in this frame.
#[derive(Clone, Copy, Debug)]
pub struct ControlFrame {
	pub hovered: bool,
	pub pressed: bool,
	/// Scale of the whole control around its center; apply it with [`ScaleExt::scaled`].
	pub scale: f32,
	/// How deep the press is in 0..=1, following the scale spring; shrinks the shadow
	/// ([`ControlFrame::shadows`]).
	pub press: f32,
	/// Pointer light and hover strength in 0..=1, faded with the hover curve.
	pub light: f32,
	/// Where the pointer light sits, in the control's unscaled paint coordinates: the pointer
	/// is mapped through the inverse of the scale so the light lands under the cursor.
	pub pointer: Option<Point<Pixels>>,
	/// Radius of the pointer light: [`pointer_radius_for`] of the control's laid-out size, or
	/// the default until the first layout.
	pub pointer_radius: Pixels,
}
