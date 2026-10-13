// SPDX-License-Identifier: MPL-2.0

//! Gallery sections for the setup assistant (step rail, choice cards) and for notices.

use calcite::prelude::*;
use gpui::{Context, IntoElement, ParentElement, Styled, div, px};

use super::{Gallery, sections::label};

const ARTWORK: [(&str, &str, Icon); 3] = [
	(
		"Default artwork",
		"Official artwork for your server.",
		Icon::ArrowClockwise,
	),
	("Pick a preset…", "Browse ready-made artwork.", Icon::Sparkles),
	("Choose Image…", "", Icon::Folder),
];

const COLORS: [(&str, &str, Icon); 2] = [
	(
		"Match the artwork",
		"Buttons and glass pick up the colors of your artwork.",
		Icon::Sparkles,
	),
	(
		"Keep it neutral",
		"Uses the standard launcher colors whatever the artwork.",
		Icon::Info,
	),
];

const EXTRAS: [(&str, &str); 2] = [
	("Server time", "Shows the server time and daily reset."),
	("Game version", "Shows the installed game version."),
];

impl Gallery {
	pub fn setup_section(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
		let rail = step_rail("SETUP ASSISTANT")
			.child(step("System Check", Icon::Info, StepState::Done))
			.child(step("Server", Icon::Globe, StepState::Done))
			.child(step("Look", Icon::Sparkles, StepState::Current))
			.child(step("Updates & Audio", Icon::Music, StepState::Upcoming))
			.child(step("Ready", Icon::Checkmark, StepState::Upcoming))
			.footer(label("Launcher vDevelopment"));

		let artwork = choice_group("Which artwork should fill the launcher?", Icon::Folder).children(
			ARTWORK.iter().enumerate().map(|(index, (title, detail, icon))| {
				let card = choice_card(("artwork", index), *title)
					.icon(*icon)
					.kind(ChoiceKind::Action)
					.on_click(cx.listener(move |this, _, _, cx| {
						this.artwork_choice = index;
						cx.notify();
					}));
				if detail.is_empty() {
					card
				} else {
					card.detail(*detail)
				}
			}),
		);

		let colors = choice_group("Which colors should the launcher use?", Icon::Sparkles).children(
			COLORS.iter().enumerate().map(|(index, (title, detail, icon))| {
				let card = choice_card(("colors", index), *title)
					.icon(*icon)
					.detail(*detail)
					.selected(self.colors_choice == index)
					.on_click(cx.listener(move |this, _, _, cx| {
						this.colors_choice = index;
						cx.notify();
					}));
				if index == 0 {
					card.accessory(tag("Recommended"))
				} else {
					card
				}
			}),
		);

		let extras = choice_group("What should the launcher show?", Icon::Info)
			.footnote("You can change these later in Settings.")
			.children(EXTRAS.iter().enumerate().map(|(index, (title, detail))| {
				choice_card(("extras", index), *title)
					.detail(*detail)
					.kind(ChoiceKind::Multiple)
					.selected(self.extras[index])
					.on_click(cx.listener(move |this, _, _, cx| {
						this.extras[index] = !this.extras[index];
						cx.notify();
					}))
			}));

		div()
			.flex()
			.flex_row()
			.gap(px(20.))
			.child(div().w(px(240.)).child(rail))
			.child(
				div()
					.flex_1()
					.flex()
					.flex_col()
					.gap(px(16.))
					.child(artwork)
					.child(colors)
					.child(extras),
			)
	}

	pub fn notices_section(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
		let banners = div()
			.flex()
			.flex_col()
			.gap(px(10.))
			.child(
				banner("banner-info", "Arknights Client 0.8.0 is ready")
					.detail("It installs when you quit the launcher."),
			)
			.child(banner("banner-success", "Game files verified").tone(BannerTone::Success))
			.child(
				banner("banner-warning", "Low disk space")
					.tone(BannerTone::Warning)
					.detail("3.4 GB free; the update needs 4.1 GB.")
					.action(button("banner-storage", "Open Storage").compact()),
			)
			.child(
				banner("banner-danger", "Download failed")
					.tone(BannerTone::Danger)
					.on_dismiss(cx.listener(|_, _, _, cx| cx.notify())),
			);

		let progress = div()
			.flex()
			.flex_col()
			.gap(px(12.))
			.child(label("Spinner · progress bar · indeterminate bar"))
			.child(
				div()
					.flex()
					.flex_row()
					.gap(px(16.))
					.items_center()
					.child(spinner())
					.child(spinner().id("spinner-large").size(px(28.))),
			)
			.child(
				progress_bar("gallery-progress", Some(self.volume))
					.label(format!("{:.0} %", self.volume * 100.)),
			)
			.child(progress_bar("gallery-indeterminate", None));

		let chips = div()
			.flex()
			.flex_col()
			.gap(px(10.))
			.child(label(
				"Chips: click a suggestion to select it, a tag to remove it",
			))
			.child(
				div().flex().flex_row().flex_wrap().gap(px(8.)).children(
					["Operators", "Events", "Skins"]
						.into_iter()
						.enumerate()
						.map(|(index, text)| {
							chip(("suggestion", index), text)
								.selected(self.chip_selected == Some(index))
								.on_click(cx.listener(move |this, _, _, cx| {
									this.chip_selected = Some(index);
									cx.notify();
								}))
						}),
				),
			)
			.child(div().flex().flex_row().flex_wrap().gap(px(8.)).children(
				self.tags.iter().enumerate().map(|(index, text)| {
					chip(("tag", index), *text).on_remove(cx.listener(move |this, _, _, cx| {
						if index < this.tags.len() {
							this.tags.remove(index);
						}
						cx.notify();
					}))
				}),
			));

		let empty = empty_state(Icon::Magnifier, "No presets match")
			.detail("Try another search or remove a tag.")
			.action(
				button("empty-reset", "Reset Search").on_click(cx.listener(|this, _, _, cx| {
					this.tags = super::TAGS.to_vec();
					cx.notify();
				})),
			);

		let confirm = button("open-confirm", "Uninstall Game…")
			.danger()
			.icon(Icon::XMark)
			.on_click(cx.listener(|this, _, _, cx| {
				this.confirm_open = true;
				cx.notify();
			}));

		div()
			.flex()
			.flex_row()
			.gap(px(28.))
			.child(div().w(px(520.)).child(banners))
			.child(
				div()
					.flex_1()
					.flex()
					.flex_col()
					.gap(px(20.))
					.child(progress)
					.child(chips)
					.child(
						div()
							.flex()
							.flex_row()
							.gap(px(16.))
							.items_start()
							.child(div().w(px(380.)).child(empty))
							.child(
								div()
									.flex()
									.flex_col()
									.gap(px(8.))
									.child(label("Confirmation dialog"))
									.child(confirm),
							),
					),
			)
	}
}
