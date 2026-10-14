// SPDX-License-Identifier: MPL-2.0

//! Live controls for `Theme::style` and the cursor glow comparison.

use calcite::prelude::*;
use gpui::{Context, IntoElement, ParentElement, Styled, div, px};

use super::{
	Gallery,
	sections::label,
	states::{cell, stack, state_row},
};

/// The radius presets the segmented control offers.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum RadiusPreset {
	Sharp,
	Default,
	Round,
}

impl RadiusPreset {
	pub const ALL: [(Self, &'static str); 3] = [
		(Self::Sharp, "Sharp"),
		(Self::Default, "Default"),
		(Self::Round, "Round"),
	];

	pub fn radii(self) -> Radii {
		match self {
			Self::Sharp => Radii::SHARP,
			Self::Default => Radii::DEFAULT,
			Self::Round => Radii::ROUND,
		}
	}

	/// The preset whose radii are `radii`; anything else (mid-crossfade) counts as Default.
	pub fn of(radii: Radii) -> Self {
		Self::ALL
			.iter()
			.map(|(preset, _)| *preset)
			.find(|preset| preset.radii() == radii)
			.unwrap_or(Self::Default)
	}
}

/// Glass strength range of the slider.
pub const STRENGTH_RANGE: std::ops::RangeInclusive<f32> = 0.0..=1.5;

impl Gallery {
	/// Radius preset, glass strength, glass style, and glow: all of `Theme::style`.
	pub fn style_section(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
		let style = cx.target_theme().style;
		stack(vec![
			state_row(
				"Radii",
				"Sharp · Default · Round",
				vec![cell(
					"every corner follows the preset",
					segmented("style-radii", RadiusPreset::of(style.radii), RadiusPreset::ALL).on_change(
						cx.listener(|_, preset: &RadiusPreset, _, cx| {
							let radii = preset.radii();
							Theme::update(cx, |theme| theme.style.radii = radii);
						}),
					),
				)],
			),
			state_row(
				"Glass strength",
				"0 is flat, 1 the system look",
				vec![cell(
					format!("{:.2}", style.glass_strength),
					slider("style-strength", style.glass_strength)
						.range(STRENGTH_RANGE)
						.step(0.05)
						.width(px(240.))
						.on_change(cx.listener(|_, value: &f32, _, cx| {
							let mut theme = cx.target_theme();
							theme.style.glass_strength = *value;
							Theme::set_immediately(cx, theme);
						})),
				)],
			),
			state_row(
				"Glass style",
				"G cycles it too",
				vec![cell(
					style.glass.label(),
					menu_picker(
						"style-glass",
						style.glass,
						GlassStyle::ALL.map(|glass| (glass, glass.label())),
					)
					.on_change(cx.listener(|_, glass: &GlassStyle, _, cx| {
						let glass = *glass;
						Theme::update(cx, |theme| theme.style.glass = glass);
					})),
				)],
			),
			state_row(
				"Pointer glow",
				"the default for every control",
				vec![cell(
					if style.pointer_glow { "on" } else { "off" },
					toggle("style-glow", style.pointer_glow)
						.label("Cursor glow")
						.on_change(cx.listener(|_, on: &bool, _, cx| {
							let on = *on;
							Theme::update(cx, |theme| theme.style.pointer_glow = on);
						})),
				)],
			),
			state_row(
				"Reset",
				"back to macOS 27 Default",
				vec![cell(
					"Style::default()",
					button("style-reset", "Reset style")
						.icon(Lucide::RotateCcw)
						.on_click(cx.listener(|_, _, _, cx| {
							Theme::update(cx, |theme| theme.style = Style::default());
						})),
				)],
			),
		])
	}

	/// The same controls with the pointer light forced on and off.
	pub fn glow_section(&self) -> impl IntoElement + use<> {
		let column = |glow: bool| {
			let tag = if glow { "on" } else { "off" };
			let id = move |name: &str| gpui::SharedString::from(format!("glow-{tag}-{name}"));
			div()
				.flex()
				.flex_col()
				.gap(px(14.))
				.child(label(if glow {
					"glow(true): the light follows the cursor"
				} else {
					"glow(false): flat"
				}))
				.child(
					div()
						.flex()
						.flex_row()
						.flex_wrap()
						.items_center()
						.gap(px(14.))
						.child(
							button(id("accent"), "Accent")
								.accent()
								.large()
								.icon(Lucide::MousePointer2)
								.glow(glow),
						)
						.child(button(id("neutral"), "Neutral").glow(glow))
						.child(icon_button(id("icon"), Lucide::Sparkles, "Sparkles").glow(glow))
						.child(button(id("compact"), "Compact").compact().glow(glow)),
				)
				.child(
					div()
						.flex()
						.flex_row()
						.flex_wrap()
						.items_center()
						.gap(px(14.))
						.child(toggle(id("toggle"), glow).label("Toggle").glow(glow))
						.child(chip(id("chip"), "Chip").glow(glow))
						.child(
							div().w(px(240.)).child(
								choice_card(id("card"), "Choice card")
									.icon(Lucide::Layers)
									.detail("Hover me")
									.glow(glow),
							),
						),
				)
		};
		stack(vec![
			div()
				.flex()
				.flex_row()
				.gap(px(48.))
				.items_start()
				.child(column(true))
				.child(column(false))
				.into_any_element(),
			label("Move the cursor over it. Compact controls and Reduce Transparency never glow; the Style section sets the default.")
				.into_any_element(),
		])
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn presets_round_trip_through_their_radii() {
		for (preset, _) in RadiusPreset::ALL {
			assert_eq!(RadiusPreset::of(preset.radii()), preset);
		}
	}

	#[test]
	fn a_mixed_radii_counts_as_default() {
		let mid = Radii::SHARP.mix(Radii::ROUND, 0.5);
		assert_eq!(RadiusPreset::of(mid), RadiusPreset::Default);
	}
}
