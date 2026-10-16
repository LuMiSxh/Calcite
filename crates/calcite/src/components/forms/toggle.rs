// SPDX-License-Identifier: MPL-2.0

//! The on/off switch: [`toggle`] (`SettingsToggle`), in the elongated macOS 26 shape. Space and
//! Return flip it, and a press can drag the knob to either side.

use std::rc::Rc;

use gpui::{
	App, ClickEvent, DispatchPhase, ElementId, Entity, IntoElement, MouseButton, ParentElement, Role,
	SharedString, Styled, Toggled, Window, div, prelude::*, px,
};

use super::{ChangeHandler, SharedChangeHandler};
use crate::{
	controls::{Size, theme_override},
	interaction::{
		ControlInteraction, ControlState, Feedback, FocusShape, KeyboardFocus, ScaleExt, focus_handle,
		glow_enabled, glow_layer,
	},
	knob::{LENS_GROWTH, ToggleDrag, knob_surface, lens_amount, release_on},
	motion::{self, Curve, secondary},
	theme::{ActiveTheme, Theme},
	tokens::{self, control, scale_alpha},
};

theme_override!(Toggle);

impl Toggle {
	pub fn label(mut self, label: impl Into<SharedString>) -> Self {
		self.label = label.into();
		self
	}

	pub fn size(mut self, size: Size) -> Self {
		self.size = size;
		self
	}

	pub fn large(self) -> Self {
		self.size(Size::Large)
	}

	pub fn compact(self) -> Self {
		self.size(Size::Compact)
	}

	/// Forces the pointer light on or off. Without it the switch follows
	/// [`Style::pointer_glow`](crate::style::Style::pointer_glow); compact switches never glow.
	pub fn glow(mut self, glow: bool) -> Self {
		self.glow = Some(glow);
		self
	}

	/// Whether the control responds. Enabled by default.
	pub fn enabled(mut self, enabled: bool) -> Self {
		self.enabled = enabled;
		self
	}

	/// The inverse of `enabled`.
	pub fn disabled(self, disabled: bool) -> Self {
		self.enabled(!disabled)
	}

	pub fn on_change(mut self, handler: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
		self.on_change = Some(Box::new(handler));
		self
	}
}

impl RenderOnce for Toggle {
	fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let reduce_motion = theme.reduce_motion;
		let state = ControlState::of(&self.id, window, cx);
		let focus = focus_handle(&self.id, window, cx);
		let frame = ControlState::frame(
			&state,
			Feedback::PressOnly,
			self.enabled,
			reduce_motion,
			window,
			cx,
		);
		let target = if self.on { 1. } else { 0. };
		let drag = window.use_keyed_state((self.id.clone(), "toggle-drag"), cx, |_, _| ToggleDrag::default());
		let thumb_spring = Curve::Release.spring().expect("Release is a spring");
		let (position, velocity) = match drag.read(cx).progress {
			// While the pointer holds the knob it sits under the pointer, not on its spring. The
			// spring's anchor is kept at the knob, so the release springs from where it was let go.
			Some(progress) => {
				motion::value::hold((self.id.clone(), "thumb"), progress, window, cx);
				(progress, 0.)
			}
			None => {
				let sample = motion::spring(
					(self.id.clone(), "thumb"),
					target,
					thumb_spring,
					reduce_motion,
					window,
					cx,
				);
				(sample.value, sample.velocity)
			}
		};
		let lens = lens_amount(
			(self.id.clone(), "lens"),
			frame.pressed,
			reduce_motion,
			window,
			cx,
		);

		let rim = motion::value::landing(
			(self.id.clone(), "landing"),
			target,
			thumb_spring,
			reduce_motion,
			window,
			cx,
		);
		let SwitchMetrics {
			track: (track_width, track_height),
			thumb: (thumb_width, thumb_height),
		} = SwitchMetrics::of(self.size);
		let inset = (track_height - thumb_height) / 2.;
		let travel = track_width - thumb_width - 2. * inset;
		// The knob stretches with its speed: widest mid-travel, round again at rest.
		let (stretch_width, stretch_height) = secondary::stretch(velocity, reduce_motion);
		let width = thumb_width * (1. + LENS_GROWTH.0 * lens) * stretch_width;
		let height = thumb_height * (1. + LENS_GROWTH.1 * lens) * stretch_height;
		let center = inset + thumb_width / 2. + travel * position.clamp(-0.08, 1.08);

		let off_track = if theme.reduce_transparency() {
			theme.palette.opaque_surface
		} else {
			theme.ink(0.18)
		};
		let track = tokens::mix(off_track, theme.accent, position.clamp(0., 1.));
		let alpha = if self.enabled {
			1.
		} else {
			control::DISABLED_FOREGROUND_OPACITY
		};
		let thumb = div()
			.absolute()
			.left(px(center - width / 2.))
			.top(px((track_height - height) / 2.))
			.w(px(width))
			.h(px(height))
			.rounded(theme.style.radii.capsule_radius(px(height)));
		// The knob rims with light for a moment as it lands, then the rim fades out.
		let thumb = thumb.when(rim > 0.01, |thumb| thumb.border_1().border_color(theme.ink(rim)));
		let thumb = knob_surface(thumb, lens, height, alpha, 0.28, &theme);
		let on = self.on;
		let glows = self.enabled && glow_enabled(&theme, self.glow, self.size == Size::Compact);
		let glow = glows
			.then(|| glow_layer(&frame, theme.ink(1.)))
			.flatten()
			.map(|glow| glow.rounded(theme.style.radii.capsule_radius(px(track_height))));
		let track = div()
			.id(self.id)
			.relative()
			.w(px(track_width))
			.h(px(track_height))
			.flex_none()
			.rounded(theme.style.radii.capsule_radius(px(track_height)))
			.bg(scale_alpha(track, alpha))
			.role(Role::Switch)
			.aria_label(self.label)
			.aria_toggled(if on { Toggled::True } else { Toggled::False })
			.aria_disabled(!self.enabled)
			.control_state(&state, glows)
			.children(glow)
			.child(thumb)
			.when(self.enabled, |this| {
				this.keyboard_focus(
					&focus,
					FocusShape::Rounded(theme.style.radii.capsule_radius(px(track_height))),
					&theme,
					window,
				)
			});
		let on_change: Option<SharedChangeHandler<bool>> =
			self.on_change.filter(|_| self.enabled).map(Rc::from);
		let track = match on_change {
			Some(on_change) => {
				let (press, moved, released, left) = (drag.clone(), drag.clone(), drag.clone(), drag.clone());
				let (changed, outside, lost, keys) =
					(on_change.clone(), on_change.clone(), on_change.clone(), on_change);
				track
					.on_mouse_down(MouseButton::Left, move |event, _, cx| {
						press.update(cx, |drag, cx| {
							drag.begin(f32::from(event.position.x), position);
							cx.notify();
						});
					})
					// The knob keeps following the pointer after it leaves the switch.
					.on_mouse_move_all(move |event, phase, _, window, cx| {
						if phase != DispatchPhase::Bubble {
							return;
						}
						// A release that never reached the switch (outside the window, lost focus) ends the gesture here.
						if event.pressed_button != Some(MouseButton::Left) {
							settle(&moved, on, &lost, window, cx);
							return;
						}
						moved.update(cx, |drag, cx| {
							if drag.move_to(f32::from(event.position.x), travel) {
								cx.notify();
							}
						});
					})
					.on_mouse_up(MouseButton::Left, move |_, window, cx| {
						settle(&released, on, &changed, window, cx)
					})
					.on_mouse_up_out(MouseButton::Left, move |_, window, cx| {
						settle(&left, on, &outside, window, cx)
					})
					// A mouse click was settled by the press and release above; keyboard clicks flip it here.
					.on_click(move |event, window, cx| {
						if !matches!(event, ClickEvent::Mouse(_)) {
							keys(&!on, window, cx);
						}
					})
			}
			None => track,
		};
		track.scaled(frame.scale)
	}
}

/// An accent switch in the elongated Liquid Glass shape of macOS 26 and 27: a capsule thumb
/// that turns into a clear glass lens while pressed and stretches as it travels.
#[derive(IntoElement)]
pub struct Toggle {
	id: ElementId,
	on: bool,
	label: SharedString,
	size: Size,
	enabled: bool,
	glow: Option<bool>,
	theme: Option<Theme>,
	on_change: Option<ChangeHandler<bool>>,
}

/// A switch showing `on`. Give it a `.label(..)`: it is the accessible name.
pub fn toggle(id: impl Into<ElementId>, on: bool) -> Toggle {
	Toggle {
		id: id.into(),
		on,
		label: SharedString::default(),
		size: Size::Regular,
		enabled: true,
		glow: None,
		theme: None,
		on_change: None,
	}
}

impl SwitchMetrics {
	fn of(size: Size) -> Self {
		match size {
			Size::Large => Self {
				track: (64., 28.),
				thumb: (39., 24.),
			},
			Size::Regular => Self {
				track: (48., 22.),
				thumb: (29., 18.),
			},
			Size::Compact => Self {
				track: (38., 18.),
				thumb: (23., 14.),
			},
		}
	}
}

/// Track and thumb geometry. Large is the 64 × 28 track and 39 × 24 thumb of Apple's
/// iOS 26 and macOS 26 design kits; the other sizes keep its proportions.
struct SwitchMetrics {
	track: (f32, f32),
	thumb: (f32, f32),
}

/// Ends a switch gesture and reports the new value once through `on_change`, if it changed.
fn settle(
	drag: &Entity<ToggleDrag>,
	on: bool,
	on_change: &SharedChangeHandler<bool>,
	window: &mut Window,
	cx: &mut App,
) {
	let Some(progress) = drag.update(cx, |drag, cx| {
		let ended = drag.end();
		if ended.is_some() {
			cx.notify();
		}
		ended
	}) else {
		return;
	};
	let next = release_on(on, progress);
	if next != on {
		on_change(&next, window, cx);
	}
}
