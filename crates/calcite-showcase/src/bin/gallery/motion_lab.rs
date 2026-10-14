// SPDX-License-Identifier: MPL-2.0

//! Motion lab: tune the open and close springs of a demo surface live. The surface follows the same
//! kind of velocity-keeping spring as `attached_surface`, so interrupting a motion shows how the
//! path bends. All state is this view's own; nothing here is global.

use std::time::Duration;

use calcite::{
	glass::{Elevation, GlassExt},
	motion::{self, Spring},
	prelude::*,
	tokens::{self, radius, white},
};
use gpui::{
	Bounds, Context, IntoElement, ParentElement, Pixels, Render, Styled, Task, Window, canvas, div, px,
};

use super::sections::{label, plot};

/// The demo surface at rest (the trigger) and fully open.
const TRIGGER: (f32, f32) = (130., 36.);
const OPEN: (f32, f32) = (300., 118.);
/// How long the interrupt button waits before reversing the motion.
const INTERRUPT_AFTER: Duration = Duration::from_millis(150);
/// How long the loop holds each end.
const LOOP_HOLD: Duration = Duration::from_millis(1400);

/// A lab spring from slider values: response 0.1..=1.0 and bounce -0.2..=0.6, both in steps of 0.01
/// so the plot cache stays small.
pub fn lab_spring(response: f32, bounce: f32) -> Spring {
	let snap = |value: f32| (value * 100.).round() / 100.;
	Spring::new(snap(response.clamp(0.1, 1.)), snap(bounce.clamp(-0.2, 0.6)))
}

/// The first overshoot of `spring`'s step response, in percent.
pub fn overshoot_percent(spring: Spring) -> f32 {
	spring.overshoot() * 100.
}

fn readout(open: Spring, close: Spring, peak: f32) -> String {
	format!(
		"overshoot  open {:.1}%  close {:.1}%  ·  this run peaked at {:.1}%",
		overshoot_percent(open),
		overshoot_percent(close),
		(peak - 1.).max(0.) * 100.
	)
}

fn lerp(from: f32, to: f32, t: f32) -> f32 {
	from + (to - from) * t
}

/// The surface size at spring value `t`: past 1 it grows a little, below 0 it stays the trigger.
fn demo_size(t: f32) -> (f32, f32) {
	let t = t.max(0.);
	(lerp(TRIGGER.0, OPEN.0, t), lerp(TRIGGER.1, OPEN.1, t))
}

pub struct MotionLab {
	open_spring: Spring,
	close_spring: Spring,
	open: bool,
	looping: bool,
	/// The highest spring value since the last command, for the measured overshoot.
	peak: f32,
	/// The pending interrupt or the running loop; replacing it cancels the old one.
	task: Option<Task<()>>,
}

impl MotionLab {
	pub fn new() -> Self {
		Self {
			open_spring: calcite::motion::tokens::MORPH_OPEN,
			close_spring: calcite::motion::tokens::MORPH_FOLD,
			open: false,
			looping: false,
			peak: 0.,
			task: None,
		}
	}

	fn go(&mut self, open: bool, cx: &mut Context<Self>) {
		self.task = None;
		self.looping = false;
		self.open = open;
		self.peak = 0.;
		cx.notify();
	}

	fn interrupt(&mut self, cx: &mut Context<Self>) {
		self.go(true, cx);
		self.task = Some(cx.spawn(async move |this, cx| {
			cx.background_executor().timer(INTERRUPT_AFTER).await;
			this.update(cx, |this, cx| {
				this.open = false;
				cx.notify();
			})
			.ok();
		}));
	}

	fn toggle_loop(&mut self, cx: &mut Context<Self>) {
		if self.looping {
			self.task = None;
			self.looping = false;
			cx.notify();
			return;
		}
		self.looping = true;
		self.peak = 0.;
		self.task = Some(cx.spawn(async move |this, cx| {
			loop {
				let alive = this.update(cx, |this, cx| {
					this.open = !this.open;
					this.peak = 0.;
					cx.notify();
				});
				if alive.is_err() {
					break;
				}
				cx.background_executor().timer(LOOP_HOLD).await;
			}
		}));
	}

	fn parameter(
		&self,
		id: &'static str,
		title: &'static str,
		value: f32,
		range: std::ops::RangeInclusive<f32>,
		cx: &mut Context<Self>,
		apply: fn(&mut Self, f32),
	) -> impl IntoElement + use<> {
		div()
			.flex()
			.flex_row()
			.items_center()
			.gap(px(10.))
			.child(div().w(px(110.)).child(label(format!("{title} {value:.2}"))))
			.child(
				slider(id, value)
					.range(range)
					.step(0.01)
					.width(px(180.))
					.on_change(cx.listener(move |this, value: &f32, _, cx| {
						apply(this, *value);
						cx.notify();
					})),
			)
	}

	fn plot_box(&self, spring: Spring, title: &'static str, accent: gpui::Rgba) -> impl IntoElement + use<> {
		div()
			.w(px(200.))
			.flex()
			.flex_col()
			.gap(px(4.))
			.child(
				div()
					.h(px(86.))
					.rounded(radius::CONTROL)
					.bg(tokens::black(0.45))
					.border_1()
					.border_color(white(0.08))
					.child(
						canvas(
							|_, _, _| {},
							move |bounds: Bounds<Pixels>, _, window, _| {
								plot(bounds, Some((spring.response, spring.bounce)), accent, window)
							},
						)
						.size_full(),
					),
			)
			.child(label(format!(
				"{title} · spring {}s bounce {}",
				spring.response, spring.bounce
			)))
	}
}

impl Render for MotionLab {
	fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
		let theme = cx.theme();
		let target = if self.open { 1. } else { 0. };
		let spring = if self.open {
			self.open_spring
		} else {
			self.close_spring
		};
		let sample = motion::spring("motion-lab-demo", target, spring, theme.reduce_motion, window, cx);
		self.peak = self.peak.max(sample.value);
		let (width, height) = demo_size(sample.value);
		let radius = lerp(TRIGGER.1 / 2., 22., sample.value.clamp(0., 1.));

		let surface = div()
			.absolute()
			.left(px(0.))
			.top(px(0.))
			.w(px(width))
			.h(px(height))
			.rounded(px(radius))
			.glass_hud_at(&theme, Elevation::Raised)
			.flex()
			.items_start()
			.p(px(10.))
			.text_size(px(12.))
			.child(if self.open { "Open" } else { "Closed" });

		div()
			.flex()
			.flex_col()
			.gap(px(14.))
			.child(
				div()
					.flex()
					.flex_row()
					.flex_wrap()
					.gap(px(24.))
					.child(
						div()
							.relative()
							.w(px(OPEN.0 + 12.))
							.h(px(OPEN.1 + 12.))
							.child(surface),
					)
					.child(
						div()
							.flex()
							.flex_col()
							.gap(px(6.))
							.child(self.parameter(
								"lab-open-response",
								"open response",
								self.open_spring.response,
								0.1..=1.0,
								cx,
								|this, value| this.open_spring = lab_spring(value, this.open_spring.bounce),
							))
							.child(self.parameter(
								"lab-open-bounce",
								"open bounce",
								self.open_spring.bounce,
								-0.2..=0.6,
								cx,
								|this, value| this.open_spring = lab_spring(this.open_spring.response, value),
							))
							.child(self.parameter(
								"lab-close-response",
								"close response",
								self.close_spring.response,
								0.1..=1.0,
								cx,
								|this, value| this.close_spring = lab_spring(value, this.close_spring.bounce),
							))
							.child(self.parameter(
								"lab-close-bounce",
								"close bounce",
								self.close_spring.bounce,
								-0.2..=0.6,
								cx,
								|this, value| {
									this.close_spring = lab_spring(this.close_spring.response, value)
								},
							)),
					)
					.child(self.plot_box(self.open_spring, "open", theme.accent))
					.child(self.plot_box(self.close_spring, "close", theme.accent)),
			)
			.child(
				div()
					.flex()
					.flex_row()
					.items_center()
					.gap(px(10.))
					.child(
						button("lab-open", "Open")
							.compact()
							.on_click(cx.listener(|this, _, _, cx| this.go(true, cx))),
					)
					.child(
						button("lab-close", "Close")
							.compact()
							.on_click(cx.listener(|this, _, _, cx| this.go(false, cx))),
					)
					.child(
						button("lab-interrupt", "Interrupt at 150 ms")
							.compact()
							.on_click(cx.listener(|this, _, _, cx| this.interrupt(cx))),
					)
					.child(
						button("lab-loop", if self.looping { "Stop loop" } else { "Loop" })
							.compact()
							.on_click(cx.listener(|this, _, _, cx| this.toggle_loop(cx))),
					),
			)
			.child(label(readout(self.open_spring, self.close_spring, self.peak)))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn lab_springs_clamp_to_the_slider_ranges_in_hundredths() {
		assert_eq!(lab_spring(0.0, -1.), Spring::new(0.1, -0.2));
		assert_eq!(lab_spring(5., 2.), Spring::new(1., 0.6));
		let spring = lab_spring(0.4849, 0.3051);
		assert_eq!((spring.response, spring.bounce), (0.48, 0.31));
	}

	#[test]
	fn overshoot_percent_matches_the_token_table() {
		let morph_open = calcite::motion::tokens::MORPH_OPEN;
		assert!((overshoot_percent(morph_open) - 4.6).abs() < 0.06);
		assert_eq!(overshoot_percent(lab_spring(0.3, -0.2)), 0.);
		assert_eq!(overshoot_percent(lab_spring(0.3, 0.)), 0.);
	}

	#[test]
	fn the_readout_names_both_springs_and_the_measured_peak() {
		let text = readout(
			calcite::motion::tokens::MORPH_OPEN,
			calcite::motion::tokens::MORPH_FOLD,
			1.046,
		);
		assert!(text.contains("open 4.6%") && text.contains("close 0.0%") && text.contains("peaked at 4.6%"));
		// A run that never reached the target reports no overshoot.
		assert!(readout(Spring::new(0.3, 0.), Spring::new(0.3, 0.), 0.4).contains("peaked at 0.0%"));
	}

	#[test]
	fn the_surface_grows_with_the_spring_and_never_shrinks_below_the_trigger() {
		assert_eq!(demo_size(0.), TRIGGER);
		assert_eq!(demo_size(1.), OPEN);
		assert_eq!(demo_size(-0.05), TRIGGER);
		assert!(demo_size(1.05).0 > OPEN.0);
	}
}
