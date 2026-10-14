// SPDX-License-Identifier: MPL-2.0

//! Selection and navigation components in their states: choice cards, chips, rows, links,
//! steps, tags, and badges.

use calcite::prelude::*;
use gpui::{AnyElement, IntoElement, ParentElement, SharedString, Styled, div, px};

use super::{
	Preview,
	states::{cell, matrix, matrix_row, stack, state_row, tones},
};

impl Preview {
	/// Choice cards, chips, badges and tags, in their states.
	pub fn choices_section(&self, theme: &Theme) -> impl IntoElement + use<> {
		let card = |id: String, kind: ChoiceKind, selected: bool, enabled: bool| -> AnyElement {
			div()
				.w(px(176.))
				.child(
					choice_card(SharedString::from(id), "Arknights")
						.icon(Lucide::Gamepad2)
						.detail("Detail text")
						.kind(kind)
						.selected(selected)
						.enabled(enabled),
				)
				.into_any_element()
		};
		let kind_row = |name: &'static str, index: usize, kind: ChoiceKind| {
			let id = move |column: &str| format!("card-{index}-{column}");
			matrix_row(
				name,
				"choice_card(id, title).kind(kind).selected(s).enabled(e)",
				vec![
					card(id("rest"), kind, false, true),
					card(id("selected"), kind, true, true),
					card(id("hover"), kind, false, true),
					card(id("focus"), kind, false, true),
					card(id("disabled"), kind, false, false),
					card(id("disabled-selected"), kind, true, false),
				],
			)
		};
		let chip_cells = vec![
			cell("rest", chip("chip-rest", "Operators")),
			cell("selected", chip("chip-selected", "Events").selected(true)),
			cell("hover: move here", chip("chip-hover", "Hover")),
			cell("focused: Tab here", chip("chip-focus", "Focus")),
			cell(
				"removable",
				chip("chip-removable", "Skins").on_remove(|_, _, _| {}),
			),
			cell(
				"removable, selected",
				chip("chip-removable-selected", "Skins")
					.selected(true)
					.on_remove(|_, _, _| {}),
			),
			cell("glow(true)", chip("chip-glow", "Glow").glow(true)),
			cell("glow(false)", chip("chip-flat", "Flat").glow(false)),
			cell(
				"long text",
				chip("chip-long", "An operator with a very long name").on_remove(|_, _, _| {}),
			),
		];

		stack(vec![
			matrix(
				176.,
				&[
					"Rest",
					"Selected",
					"Hover: move here",
					"Focused: Tab here",
					"Disabled",
					"Disabled, selected",
				],
				vec![
					kind_row("Single (radio)", 0, ChoiceKind::Single),
					kind_row("Multiple (checkbox)", 1, ChoiceKind::Multiple),
					kind_row("Action (chevron)", 2, ChoiceKind::Action),
				],
			),
			state_row(
				"choice_card · content",
				"accessory, no icon, no detail, long text, glow",
				"choice_card(id, title).accessory(tag(text)).glow(false)",
				vec![
					cell(
						"accessory",
						div().w(px(300.)).child(
							choice_card("card-accessory", "Match the artwork")
								.icon(Lucide::Sparkles)
								.detail("Buttons and glass pick up the colors.")
								.accessory(tag("Recommended")),
						),
					),
					cell(
						"title only",
						div()
							.w(px(220.))
							.child(choice_card("card-title-only", "Just a title")),
					),
					cell(
						"long text",
						div().w(px(300.)).child(
							choice_card(
								"card-long",
								"A title that is long enough to need more than one line",
							)
							.icon(Lucide::FileText)
							.detail(
								"The detail text also runs long, so the card shows how both lines wrap and grow.",
							),
						),
					),
					cell(
						"glow(false)",
						div().w(px(220.)).child(
							choice_card("card-flat", "No pointer light")
								.icon(Lucide::Layers)
								.glow(false),
						),
					),
				],
			),
			state_row(
				"chip",
				"click selects · the x removes",
				"chip(id, text).selected(true).on_remove(handler).glow(false)",
				chip_cells,
			),
			state_row(
				"badge · tag",
				"every tone, then long text",
				"badge(text).tone(tone) · tag(text)",
				tones(theme)
					.into_iter()
					.map(|(name, tone)| cell(name, badge(name).tone(tone)))
					.chain([
						cell("tag", tag("Recommended")),
						cell("badge, long", badge("A badge with a long label")),
						cell("tag, long", tag("A recommended tag with a long label")),
					])
					.collect(),
			),
		])
	}

	/// The navigation rail's rows, list rows, links, and the setup steps.
	pub fn selection_section(&self) -> impl IntoElement + use<> {
		let rail = |selected: bool, id: &'static str, title: &'static str| {
			div().w(px(190.)).h(px(60.)).child(
				nav_rail(format!("rail-{id}")).child(nav_item(id, title, Lucide::Palette).selected(selected)),
			)
		};
		let group = div().w(px(190.)).h(px(190.)).child(
			nav_rail("rail-group")
				.child(nav_item("nav-g-1", "Game", Lucide::Gamepad2))
				.child(nav_item("nav-g-2", "Appearance", Lucide::Palette).selected(true))
				.child(nav_item("nav-g-3", "Audio", Lucide::Volume2)),
		);
		let steps = div().w(px(240.)).child(
			step_rail("STEPS")
				.child(step("Done", Lucide::Check, StepState::Done))
				.child(step("Current", Lucide::Sparkles, StepState::Current))
				.child(step("Upcoming", Lucide::Music, StepState::Upcoming))
				.child(step("Clickable", Lucide::Info, StepState::Upcoming).on_click(|_, _, _| {}))
				.footer(div().child("Footer")),
		);
		let row = |id: &'static str, title: &'static str| div().w(px(240.)).child(list_row(id, title));

		stack(vec![
			state_row(
				"nav_item",
				"rail row, 40 px",
				"nav_item(id, title, Lucide::Palette).selected(true)",
				vec![
					cell("rest", rail(false, "nav-rest", "Appearance")),
					cell("selected", rail(true, "nav-selected", "Appearance")),
					cell("hover: move here", rail(false, "nav-hover", "Hover")),
					cell("focused: Tab here", rail(false, "nav-focus", "Focus")),
					cell(
						"long title",
						rail(false, "nav-long", "A navigation title that is long"),
					),
					cell("nav_rail with three items", group),
				],
			),
			state_row(
				"list_row",
				"rest · value · selected · long",
				"list_row(id, label).value(text).selected(true)",
				vec![
					cell("rest", row("row-rest", "Global")),
					cell(
						"value",
						div()
							.w(px(240.))
							.child(list_row("row-value", "Global").value("04:12:09")),
					),
					cell(
						"selected",
						div()
							.w(px(240.))
							.child(list_row("row-selected", "Japan").value("05:12:09").selected(true)),
					),
					cell("hover: move here", row("row-hover", "Korea")),
					cell("focused: Tab here", row("row-focus", "China")),
					cell(
						"long label + value",
						div().w(px(240.)).child(
							list_row("row-long", "A region with a rather long name").value("06:12:09"),
						),
					),
				],
			),
			state_row(
				"Links",
				"underline on hover",
				"link(id, title) · document_link(id, title, icon)",
				vec![
					cell("link", link("link-demo", "Privacy notice")),
					cell("link, hover: move here", link("link-hover", "Support")),
					cell("link, focused: Tab here", link("link-focus", "Release notes")),
					cell(
						"document_link",
						div().w(px(220.)).child(document_link(
							"doc-link",
							"Open log files",
							Lucide::FileText,
						)),
					),
					cell(
						"document_link, long",
						div().w(px(220.)).child(document_link(
							"doc-link-long",
							"Open the diagnostic log files folder",
							Lucide::FileText,
						)),
					),
				],
			),
			state_row(
				"step_rail",
				"Done · Current · Upcoming · clickable · footer",
				"step_rail(header).child(step(title, icon, StepState::Done)).footer(el)",
				vec![cell("states", steps)],
			),
		])
	}
}
