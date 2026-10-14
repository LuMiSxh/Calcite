// SPDX-License-Identifier: MPL-2.0

//! Selection and navigation components in their states: choice cards, chips, rows, links,
//! steps, tags, and badges.

use calcite::prelude::*;
use gpui::{AnyElement, IntoElement, ParentElement, SharedString, Styled, div, px};

use super::{
	Gallery,
	states::{cell, stack, state_row, tones},
};

impl Gallery {
	pub fn selection_section(&self, theme: &Theme) -> impl IntoElement + use<> {
		let card = |id: SharedString, kind: ChoiceKind, selected: bool, enabled: bool| {
			cell(
				format!(
					"{}{}{}",
					match kind {
						ChoiceKind::Single => "single",
						ChoiceKind::Multiple => "multiple",
						ChoiceKind::Action => "action",
					},
					if selected { " · selected" } else { "" },
					if enabled { "" } else { " · disabled" },
				),
				div().w(px(300.)).child(
					choice_card(id, "Arknights")
						.icon(Lucide::Gamepad2)
						.detail("Hover me, press and hold")
						.kind(kind)
						.selected(selected)
						.enabled(enabled),
				),
			)
		};
		let cards: Vec<AnyElement> = [ChoiceKind::Single, ChoiceKind::Multiple, ChoiceKind::Action]
			.into_iter()
			.enumerate()
			.flat_map(|(kind_index, kind)| {
				[(false, true), (true, true), (false, false)].map(|(selected, enabled)| {
					let id = SharedString::from(format!("card-{kind_index}-{selected}-{enabled}"));
					card(id, kind, selected, enabled)
				})
			})
			.collect();

		let rail = |selected: bool, id: &'static str| {
			div()
				.w(px(190.))
				.h(px(60.))
				.child(nav_rail(theme).child(nav_item(id, "Appearance", Lucide::Palette).selected(selected)))
		};
		let steps = div().w(px(240.)).child(
			step_rail("STEPS")
				.child(step("Done", Lucide::Check, StepState::Done))
				.child(step("Current", Lucide::Sparkles, StepState::Current))
				.child(step("Upcoming", Lucide::Music, StepState::Upcoming)),
		);

		stack(vec![
			state_row("choice_card", "three kinds × rest, selected, disabled", cards),
			state_row(
				"chip",
				"click selects · the x removes · hover me",
				vec![
					cell("rest", chip("chip-rest", "Operators")),
					cell("selected", chip("chip-selected", "Events").selected(true)),
					cell(
						"removable",
						chip("chip-removable", "Skins").on_remove(|_, _, _| {}),
					),
					cell("glow on", chip("chip-glow", "Glow").glow(true)),
				],
			),
			state_row(
				"nav_item",
				"rail row, 40 px",
				vec![
					cell("rest · hover me", rail(false, "nav-rest")),
					cell("selected", rail(true, "nav-selected")),
				],
			),
			state_row(
				"list_row",
				"rest · hover me · selected",
				vec![
					cell(
						"rest",
						div()
							.w(px(240.))
							.child(list_row("row-rest", "Global").value("04:12:09")),
					),
					cell(
						"selected",
						div()
							.w(px(240.))
							.child(list_row("row-selected", "Japan").value("05:12:09").selected(true)),
					),
				],
			),
			state_row(
				"Links",
				"underline on hover",
				vec![
					cell("link", link("link-demo", "Privacy notice")),
					cell(
						"document_link",
						div().w(px(220.)).child(document_link(
							"doc-link",
							"Open log files",
							Lucide::FileText,
						)),
					),
				],
			),
			state_row(
				"step_rail",
				"Done · Current · Upcoming",
				vec![cell("states", steps)],
			),
			state_row(
				"badge · tag",
				"every tone",
				tones(theme)
					.into_iter()
					.map(|(name, tone)| cell(name, badge(name).tone(tone)))
					.chain([cell("tag", tag("Recommended"))])
					.collect(),
			),
		])
	}
}
