// SPDX-License-Identifier: MPL-2.0

//! Typography: the bundled families at a few sizes, and a rolling number whose digits keep a
//! fixed width as they change.

use calcite::{prelude::*, theme::ActiveTheme};
use gpui::{ClickEvent, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, px};

use super::{
	Preview,
	states::{cell, stack, state_row},
};

impl Preview {
	pub fn typography_section(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
		let (sans, mono) = {
			let theme = cx.theme();
			(theme.sans(), theme.mono())
		};
		let specimen = |family: &SharedString, weight: FontWeight, size: f32| {
			div()
				.font_family(family.clone())
				.font_weight(weight)
				.text_size(px(size))
				.child("Calcite 0123456789")
		};
		let percent = self.ticker_percent;
		let percent_cell = div()
			.flex()
			.flex_col()
			.gap(px(10.))
			.child(ticker("typography-percent-value", format!("{:.0}%", percent * 100.)).font_size(px(28.)))
			.child(
				slider("typography-percent", percent)
					.step(0.01)
					.width(px(200.))
					.on_change(cx.listener(|this, value: &f32, _, cx| {
						this.ticker_percent = *value;
						cx.notify();
					})),
			);
		let counter_cell = div()
			.flex()
			.flex_row()
			.items_center()
			.gap(px(12.))
			.child(button("typography-count-down", "-").on_click(cx.listener(
				|this, _: &ClickEvent, _, cx| {
					this.ticker_count -= 1;
					cx.notify();
				},
			)))
			.child(ticker("typography-count-value", self.ticker_count.to_string()).font_size(px(28.)))
			.child(button("typography-count-up", "+").on_click(cx.listener(
				|this, _: &ClickEvent, _, cx| {
					this.ticker_count += 1;
					cx.notify();
				},
			)));

		stack(vec![
			state_row(
				"Sans",
				"theme.sans(): Plus Jakarta Sans by default; Regular, Medium, SemiBold",
				"theme.sans() · font_family(sans).font_weight(FontWeight::MEDIUM)",
				vec![
					cell("12 pt Regular", specimen(&sans, FontWeight::NORMAL, 12.)),
					cell("18 pt Medium", specimen(&sans, FontWeight::MEDIUM, 18.)),
					cell("28 pt SemiBold", specimen(&sans, FontWeight::SEMIBOLD, 28.)),
					cell(
						"Latin-only bundled sans: other scripts use the system font",
						div()
							.font_family(sans.clone())
							.text_size(px(18.))
							.child("Привет мир · Καλημέρα κόσμος"),
					),
				],
			),
			state_row(
				"Mono",
				"theme.mono(): Fira Code by default; Regular, Bold",
				"theme.mono() · font_family(mono).font_weight(FontWeight::BOLD)",
				vec![
					cell("12 pt Regular", specimen(&mono, FontWeight::NORMAL, 12.)),
					cell("18 pt Regular", specimen(&mono, FontWeight::NORMAL, 18.)),
					cell("28 pt Bold", specimen(&mono, FontWeight::BOLD, 28.)),
				],
			),
			state_row(
				"Rolling numbers",
				"digits keep a fixed width",
				"ticker(id, text).size(px(28.))",
				vec![
					cell("drag the slider", percent_cell),
					cell("press + and -", counter_cell),
				],
			),
		])
	}
}
