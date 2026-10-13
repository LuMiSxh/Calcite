// SPDX-License-Identifier: MPL-2.0

//! Gallery sections whose components change gallery state.

use calcite::{prelude::*, tokens::hud};
use gpui::{
	AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*, px,
};

use super::{Display, Gallery, Page, Rendering, sections::label};

const REGIONS: [&str; 5] = ["Global", "Japan", "Korea", "China", "China (Bilibili)"];

impl Gallery {
	pub fn actions_section(&self) -> impl IntoElement + use<> {
		let tones = [
			("Accent", Tone::Accent),
			("Neutral", Tone::Neutral),
			("Warning", Tone::Warning),
			("Danger", Tone::Danger),
		];
		let sizes = [
			("Large", Size::Large),
			("Regular", Size::Regular),
			("Compact", Size::Compact),
		];
		let mut rows: Vec<AnyElement> = Vec::new();
		for (size_name, size) in sizes {
			rows.push(
				div()
					.flex()
					.flex_row()
					.items_center()
					.gap(px(10.))
					.child(div().w(px(56.)).child(label(size_name)))
					.children(tones.iter().map(|(tone_name, tone)| {
						button(
							SharedString::from(format!("tone-{size_name}-{tone_name}")),
							*tone_name,
						)
						.tone(*tone)
						.icon(Icon::ArrowDown)
						.size(size)
					}))
					.into_any_element(),
			);
		}
		rows.push(
			div()
				.flex()
				.flex_row()
				.items_center()
				.gap(px(10.))
				.child(div().w(px(56.)).child(label("Disabled")))
				.children(tones.iter().map(|(tone_name, tone)| {
					button(SharedString::from(format!("disabled-{tone_name}")), *tone_name)
						.tone(*tone)
						.icon(Icon::Pause)
						.disabled(true)
				}))
				.into_any_element(),
		);
		rows.push(
			div()
				.flex()
				.flex_row()
				.items_center()
				.gap(px(14.))
				.child(div().w(px(56.)).child(label("Specials")))
				.child(
					div()
						.id("gear-tooltip")
						.glass_tooltip("Settings")
						.child(icon_button("gear", Icon::Gear, "Settings")),
				)
				.child(halo(
					button("halo-play", "Play").accent().large().icon(Icon::Play),
				))
				.child(done_button("done"))
				.into_any_element(),
		);
		div().flex().flex_col().gap(px(12.)).children(rows).child(label(
			"Hover: 1.025 spring lift and pointer light · press: 0.95 and 0.86 opacity · surfaces scale without relayout",
		))
	}

	pub fn forms_section(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
		let row = |title: &'static str, control: AnyElement| {
			div()
				.flex()
				.flex_row()
				.items_center()
				.gap(px(12.))
				.child(div().w(px(110.)).child(label(title)))
				.child(control)
				.into_any_element()
		};
		div()
			.flex()
			.flex_col()
			.gap(px(12.))
			.child(row(
				"Toggle",
				div()
					.flex()
					.flex_row()
					.gap(px(12.))
					.items_center()
					.child(
						toggle("auto-update", self.auto_update)
							.label("Automatic updates")
							.on_change(cx.listener(|this, on: &bool, _, cx| {
								this.auto_update = *on;
								cx.notify();
							})),
					)
					.child(
						toggle("game-mode", self.game_mode)
							.label("Game Mode")
							.on_change(cx.listener(|this, on: &bool, _, cx| {
								this.game_mode = *on;
								cx.notify();
							})),
					)
					.child(toggle("locked", true).label("Locked").disabled(true))
					.child(label("Large · compact"))
					.child(
						toggle("large-toggle", self.game_mode)
							.large()
							.label("Large")
							.on_change(cx.listener(|this, on: &bool, _, cx| {
								this.game_mode = *on;
								cx.notify();
							})),
					)
					.child(
						toggle("compact-toggle", self.auto_update)
							.compact()
							.label("Compact")
							.on_change(cx.listener(|this, on: &bool, _, cx| {
								this.auto_update = *on;
								cx.notify();
							})),
					)
					.into_any_element(),
			))
			.child(row(
				"Slider",
				div()
					.flex()
					.flex_row()
					.gap(px(10.))
					.items_center()
					.child(
						slider("volume", self.volume)
							.step(0.05)
							.width(px(200.))
							.label("Volume")
							.on_change(cx.listener(|this, value: &f32, _, cx| {
								this.volume = *value;
								cx.notify();
							})),
					)
					.child(label(format!("{:.0} %", self.volume * 100.)))
					.into_any_element(),
			))
			.child(row(
				"Segmented",
				segmented(
					"display",
					self.display,
					[
						(Display::Windowed, "Windowed"),
						(Display::Borderless, "Borderless"),
						(Display::Fullscreen, "Fullscreen"),
					],
				)
				.on_change(cx.listener(|this, display: &Display, _, cx| {
					this.display = *display;
					cx.notify();
				}))
				.into_any_element(),
			))
			.child(row(
				"Text field",
				text_field("search", &self.search)
					.placeholder("Search operators")
					.icon(Icon::Magnifier)
					.width(px(260.))
					.into_any_element(),
			))
			.child(row(
				"Menu picker",
				menu_picker(
					"rendering",
					self.rendering,
					[
						(Rendering::Retina, "Retina"),
						(Rendering::MetalFx, "MetalFX"),
						(Rendering::Standard, "Standard"),
					],
				)
				.divider_before(Rendering::Standard)
				.on_change(cx.listener(|this, rendering: &Rendering, _, cx| {
					this.rendering = *rendering;
					cx.notify();
				}))
				.into_any_element(),
			))
			.child(row(
				"Action menu",
				action_menu("logs", "Logs", Icon::Document)
					.item("Show in Finder", Icon::Folder, |_, _| {})
					.item("Copy Path", Icon::Document, |_, _| {})
					.divider()
					.item("Clear Logs", Icon::XMark, |_, _| {})
					.into_any_element(),
			))
	}

	pub fn settings_section(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
		let theme = cx.theme();
		let rail = nav_rail()
			.pt(px(14.))
			.children(Page::ALL.iter().enumerate().map(|(index, page)| {
				nav_item(
					SharedString::from(format!("nav-{index}")),
					page.title(),
					page.icon(),
				)
				.selected(self.page == *page)
				.on_click(cx.listener(move |this, _, _, cx| {
					this.page = Page::ALL[index];
					cx.notify();
				}))
			}));
		let panel_open = self.panel_open;
		let page = div()
			.flex_1()
			.flex()
			.flex_col()
			.gap(px(14.))
			.p(px(20.))
			.child(page_header(
				self.page.title(),
				"Keep the launcher and the game current.",
				&theme,
			))
			.child(
				settings_panel("launcher-panel", "Launcher", Icon::Sparkles)
					.child(
						action_row("Automatic updates")
							.detail("Checks once a day and installs on quit")
							.action(
								toggle("panel-auto", self.auto_update)
									.label("Automatic updates")
									.on_change(cx.listener(|this, on: &bool, _, cx| {
										this.auto_update = *on;
										cx.notify();
									})),
							)
							.action(button("check-now", "Check Now").accent().compact()),
					)
					.child(hairline())
					.child(
						action_row("Release channel")
							.detail("Canary features stay opt-in")
							.action(badge("Stable").tone(Tone::Neutral)),
					),
			)
			.child(
				settings_panel("advanced-panel", "Advanced", Icon::Gear)
					.collapsible(
						panel_open,
						cx.listener(|this, _, _, cx| {
							this.panel_open = !this.panel_open;
							cx.notify();
						}),
					)
					.child(document_link("open-logs", "Open log files", Icon::Document))
					.child(document_link("open-folder", "Show game folder", Icon::Folder)),
			)
			.child(
				danger_zone("danger").child(
					action_row("Remove game files")
						.detail("Settings and the Wine prefix stay")
						.action(button("remove", "Remove…").danger().compact()),
				),
			);
		div()
			.h(px(660.))
			.flex()
			.flex_row()
			.overflow_hidden()
			.rounded(tokens::radius::MODAL)
			.bg(calcite::tokens::stack(&[
				tokens::control::MODAL_BACKGROUND,
				theme.hud_tint,
			]))
			.border_1()
			.border_color(tokens::HAIRLINE)
			.child(rail)
			.child(page)
	}

	pub fn hud_section(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
		let rows = REGIONS.iter().enumerate().map(|(index, name)| {
			list_row(SharedString::from(format!("gallery-region-{index}")), *name)
				.value("04:12:09")
				.selected(index == self.region)
				.on_click(cx.listener(move |this, _, _, cx| {
					this.region = index;
					cx.notify();
				}))
		});
		let collapsed = div()
			.flex()
			.flex_col()
			.gap(px(10.))
			.child(label("Collapsed capsules"))
			.child(
				div().flex().child(
					pill(
						"music",
						Icon::Music,
						div()
							.font_weight(FontWeight::MEDIUM)
							.child("Sanity Restored — Lobby Theme"),
					)
					.detail(div(), px(330.), px(100.)),
				),
			)
			.child(
				div().flex().child(
					pill(
						"version",
						Icon::Tag,
						div().font_weight(FontWeight::MEDIUM).child("2.6.81"),
					)
					.detail(div(), px(260.), px(100.)),
				),
			)
			.child(
				div().flex().child(
					pill("receded", Icon::Tag, div().child("Receded sibling"))
						.detail(div(), px(260.), px(100.))
						.receded(true),
				),
			);
		let expanded = pill(
			"expanded-status",
			Icon::Globe,
			div().font_weight(FontWeight::SEMIBOLD).child("Global 04:12:09"),
		)
		.noun("server time")
		.detail(
			div().flex().flex_col().gap(px(2.)).children(rows),
			hud::EXPANDED_STATUS_WIDTH,
			px(28. + 8. + 16. + 5. * 28.),
		)
		.expanded(true);
		let bars = div()
			.flex_1()
			.flex()
			.flex_col()
			.gap(px(12.))
			.child(label("Control bar states"))
			.child(
				control_bar(status("Ready to play").detail("Global · Version 2.6.81"))
					.action(halo(button("bar-play", "Play").accent().large().icon(Icon::Play))),
			)
			.child(
				control_bar(
					status("42%")
						.emphasized(true)
						.detail("1.44 GB of 3.42 GB · 38.6 MB/s · 51 s left"),
				)
				.progress(Some(OutlineMode::Determinate(0.42)), true)
				.action(button("bar-pause", "Pause").large().icon(Icon::Pause)),
			)
			.child(
				control_bar(status("Verifying files").detail("Checking 1 284 files"))
					.progress(Some(OutlineMode::Indeterminate), true)
					.action(
						button("bar-verify", "Pause")
							.large()
							.icon(Icon::Pause)
							.disabled(true),
					),
			);
		div()
			.flex()
			.flex_row()
			.gap(px(24.))
			.items_start()
			.child(collapsed)
			.child(
				div()
					.flex()
					.flex_col()
					.gap(px(10.))
					.child(label("Expanded panel"))
					.child(expanded),
			)
			.child(bars)
	}

	pub fn overlay_section(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
		let theme = cx.theme();
		div()
			.flex()
			.flex_row()
			.gap(px(24.))
			.items_start()
			.child(
				modal("sample-modal", "Update available")
					.size(px(520.), px(330.))
					.child(
						div().child(
							"Arknights Client 0.8.0 is ready. It installs when you quit the launcher.",
						),
					)
					.child(
						div()
							.flex()
							.flex_row()
							.gap(px(8.))
							.child(badge("Recommended"))
							.child(badge("Delta update").tone(Tone::Neutral)),
					)
					.child(
						div()
							.flex()
							.flex_col()
							.gap(px(6.))
							.child(skeleton("line-1").size(px(320.), px(12.)).pulses(true))
							.child(skeleton("line-2").size(px(240.), px(12.)).pulses(true)),
					)
					.action(button("later", "Later"))
					.action(done_button("install").icon(Icon::ArrowDown)),
			)
			.child(
				div()
					.flex()
					.flex_col()
					.gap(px(14.))
					.child(label("Floating action bar"))
					.child(
						div().flex().child(
							floating_action_bar(&theme)
								.child(button("fab-cancel", "Cancel"))
								.child(done_button("fab-done")),
						),
					)
					.child(label("Links"))
					.child(
						div()
							.flex()
							.flex_row()
							.gap(px(16.))
							.child(link("privacy", "Privacy notice"))
							.child(link("support", "Publisher support")),
					)
					.child(label("Badges"))
					.child(
						div()
							.flex()
							.flex_row()
							.gap(px(8.))
							.child(badge("Canary"))
							.child(badge("Stable").tone(Tone::Neutral))
							.child(badge("Beta OS").tone(Tone::Warning))
							.child(badge("Failed").tone(Tone::Danger)),
					)
					.child(label("Skeleton placeholders"))
					.child(
						div()
							.flex()
							.flex_row()
							.gap(px(8.))
							.child(skeleton("sk-1").size(px(64.), px(14.)))
							.child(skeleton("sk-2").size(px(96.), px(14.)).pulses(true)),
					)
					.child(label(
						"Modal layer: open with the button, close by clicking outside",
					))
					.child(
						div()
							.flex()
							.child(button("open-modal", "Open modal").accent().on_click(cx.listener(
								|this, _, _, cx| {
									this.modal_open = true;
									cx.notify();
								},
							))),
					),
			)
	}
}
