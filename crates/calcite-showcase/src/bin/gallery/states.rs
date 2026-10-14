// SPDX-License-Identifier: MPL-2.0

//! State matrices: each component in a labelled row per state. Hover and press cannot be
//! forced, so those cells say "hover me" and "press and hold".

use calcite::prelude::*;
use gpui::{
	AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*, px,
};

use super::{Display, Gallery, sections::label};

/// A caption under a component.
pub fn cell(caption: impl Into<SharedString>, element: impl IntoElement) -> AnyElement {
	div()
		.flex()
		.flex_col()
		.items_start()
		.gap(px(6.))
		.child(element)
		.child(label(caption))
		.into_any_element()
}

/// One component, one row: the title and a note on the left, the state cells on the right.
pub fn state_row(title: &'static str, note: &'static str, cells: Vec<AnyElement>) -> AnyElement {
	div()
		.flex()
		.flex_row()
		.items_start()
		.gap(px(16.))
		.child(
			div()
				.w(px(150.))
				.flex_none()
				.flex()
				.flex_col()
				.gap(px(2.))
				.child(
					div()
						.text_size(px(12.))
						.font_weight(FontWeight::SEMIBOLD)
						.child(title),
				)
				.child(label(note)),
		)
		.child(
			div()
				.flex_1()
				.flex()
				.flex_row()
				.flex_wrap()
				.items_start()
				.gap_x(px(20.))
				.gap_y(px(12.))
				.children(cells),
		)
		.into_any_element()
}

/// The rows stacked.
pub fn stack(rows: Vec<AnyElement>) -> impl IntoElement {
	div().flex().flex_col().gap(px(16.)).children(rows)
}

/// The tones a button offers, success included as a palette color.
pub fn tones(theme: &Theme) -> [(&'static str, Tone); 5] {
	[
		("Neutral", Tone::Neutral),
		("Accent", Tone::Accent),
		("Success", Tone::Color(theme.palette.success)),
		("Warning", Tone::Warning),
		("Danger", Tone::Danger),
	]
}

impl Gallery {
	pub fn actions_section(&self, theme: &Theme) -> impl IntoElement + use<> {
		let tones = tones(theme);
		let by_tone = |size: Size, prefix: &'static str, enabled: bool| -> Vec<AnyElement> {
			tones
				.iter()
				.map(|(name, tone)| {
					cell(
						*name,
						button(SharedString::from(format!("{prefix}-{name}")), *name)
							.tone(*tone)
							.size(size)
							.disabled(!enabled),
					)
				})
				.collect()
		};
		let icon_tones: Vec<AnyElement> = tones
			.iter()
			.map(|(name, tone)| {
				cell(
					*name,
					button(SharedString::from(format!("icon-{name}")), *name)
						.tone(*tone)
						.icon(Lucide::Download),
				)
			})
			.collect();
		stack(vec![
			state_row(
				"Button · tones",
				"rest · hover me · press and hold (scale 0.95)",
				by_tone(Size::Regular, "tone", true),
			),
			state_row("Large", "same tones", by_tone(Size::Large, "large", true)),
			state_row("Compact", "never glows", by_tone(Size::Compact, "compact", true)),
			state_row("With icon", "leading glyph", icon_tones),
			state_row(
				"Disabled",
				"no hover, no press",
				by_tone(Size::Regular, "disabled", false),
			),
			state_row(
				"Focused",
				"Tab moves focus; the ring shows for the keyboard only",
				vec![
					cell(
						"Tab here",
						button("focus-a", "First").accent().icon(Lucide::Keyboard),
					),
					cell("then here", button("focus-b", "Second")),
					cell("Return presses", button("focus-c", "Third").danger()),
				],
			),
			state_row(
				"Busy",
				"no loading state: disable and add a spinner",
				vec![cell(
					"disabled + spinner",
					div()
						.flex()
						.flex_row()
						.items_center()
						.gap(px(8.))
						.child(
							button("busy", "Verifying…")
								.accent()
								.disabled(true)
								.icon(Lucide::LoaderCircle),
						)
						.child(spinner().id("busy-spinner")),
				)],
			),
			state_row(
				"icon_button",
				"rest · hover me · press and hold",
				vec![
					cell(
						"default 40",
						div()
							.id("gear-tooltip")
							.glass_tooltip("Settings")
							.child(icon_button("gear", Icon::Gear, "Settings")),
					),
					cell(
						"diameter 32",
						icon_button("gear-small", Lucide::Bell, "Notifications").diameter(px(32.)),
					),
					cell(
						"diameter 52",
						icon_button("gear-large", Lucide::Rocket, "Launch").diameter(px(52.)),
					),
				],
			),
			state_row(
				"Specials",
				"halo · done_button · glass_tooltip",
				vec![
					cell(
						"halo",
						halo(button("halo-play", "Play").accent().large().icon(Icon::Play)),
					),
					cell("done_button", done_button("done")),
					cell(
						"glass_tooltip: hover",
						div()
							.id("tooltip-demo")
							.glass_tooltip("A glass tooltip")
							.child(button("tooltip-button", "Hover me")),
					),
				],
			),
		])
	}

	pub fn forms_section(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
		let switch = |index: usize, id: &'static str| {
			toggle(id, self.toggles[index]).on_change(cx.listener(move |this, on: &bool, _, cx| {
				this.toggles[index] = *on;
				cx.notify();
			}))
		};
		let displays = [
			(Display::Windowed, "Windowed"),
			(Display::Borderless, "Borderless"),
			(Display::Fullscreen, "Fullscreen"),
		];
		let display_change = cx.listener(|this, display: &Display, _, cx| {
			this.display = *display;
			cx.notify();
		});
		let volume = |id: &'static str| {
			slider(id, self.volume)
				.step(0.05)
				.leading_icon(Lucide::Volume1)
				.trailing_icon(Lucide::Volume2)
		};
		stack(vec![
			state_row(
				"Toggle",
				"press, hold and drag the knob; release settles on the nearer side",
				vec![
					cell("off · hover me", switch(0, "toggle-off").label("Off")),
					cell("on", switch(1, "toggle-on").label("On")),
					cell(
						"disabled off",
						toggle("toggle-d-off", false).label("Disabled").disabled(true),
					),
					cell(
						"disabled on",
						toggle("toggle-d-on", true).label("Locked").disabled(true),
					),
					cell("large", switch(2, "toggle-large").large().label("Large")),
					cell("compact", switch(3, "toggle-compact").compact().label("Compact")),
					cell(
						"hint",
						div().pt(px(4.)).child(label("press, hold and drag, or click")),
					),
				],
			),
			state_row(
				"Slider",
				"drag the knob · arrow keys after Tab step it",
				vec![
					cell(
						"rest · value label",
						volume("slider-rest")
							.width(px(220.))
							.label("Volume")
							.value_label(true)
							.on_change(cx.listener(|this, value: &f32, _, cx| {
								this.volume = *value;
								cx.notify();
							})),
					),
					cell(
						"ticks",
						slider("slider-ticks", self.volume)
							.step(0.1)
							.width(px(200.))
							.ticks(11)
							.on_change(cx.listener(|this, value: &f32, _, cx| {
								this.volume = *value;
								cx.notify();
							})),
					),
					cell(
						"disabled",
						volume("slider-disabled").width(px(200.)).disabled(true),
					),
				],
			),
			state_row(
				"Segmented",
				"selected segment is raised",
				vec![
					cell(
						"rest",
						segmented("seg-rest", self.display, displays).on_change(display_change),
					),
					cell(
						"disabled",
						segmented("seg-disabled", Display::Borderless, displays).disabled(true),
					),
				],
			),
			state_row(
				"Text field",
				"click to focus; no invalid state, pair with a danger banner",
				vec![
					cell(
						"empty + icon",
						text_field("tf-empty", &self.search)
							.placeholder("Search operators")
							.icon(Lucide::Search)
							.width(px(240.)),
					),
					cell(
						"filled",
						text_field("tf-filled", &self.filled)
							.icon(Lucide::Pencil)
							.width(px(240.)),
					),
				],
			),
		])
	}
}
