// SPDX-License-Identifier: MPL-2.0

//! Preview sections whose components change preview state.

use calcite::{interaction::spring_value, prelude::*, tokens::hud};
use gpui::{
	AnyElement, Context, Div, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window, div, px,
};

use super::{
	Display, Page, Preview, Rendering,
	sections::label,
	states::{cell, stack, state_row},
};

const REGIONS: [&str; 5] = ["Global", "Japan", "Korea", "China", "China (Bilibili)"];
pub const ACCENTS: [&str; 4] = ["Default", "Amber", "Rose", "Violet"];

impl Preview {
	pub fn settings_section(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
		let theme = cx.theme();
		let rail = nav_rail("settings-rail")
			.pt(px(14.))
			// overflow_hidden clips to the frame's rectangle, so the rail's own fill takes the
			// frame's left corners.
			.rounded_tl(theme.style.radii.modal)
			.rounded_bl(theme.style.radii.modal)
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
		let body = match self.page {
			Page::Game => self.game_page(cx),
			Page::Appearance => self.appearance_page(),
			Page::Audio => self.audio_page(cx),
			Page::Updates => self.updates_page(cx),
			Page::Installation => self.installation_page(),
			Page::Storage => self.storage_page(),
			Page::About => self.about_page(),
		};
		let page = div()
			.flex_1()
			.flex()
			.flex_col()
			.gap(px(14.))
			.p(px(20.))
			.child(page_header(self.page.title(), self.page.subtitle()))
			.child(body);
		let frame = div()
			.h(px(660.))
			.flex()
			.flex_row()
			.overflow_hidden()
			.rounded(theme.style.radii.modal)
			.bg(calcite::tokens::stack(&[
				theme.palette.panel_background,
				theme.hud_tint,
			]))
			.border_1()
			.border_color(theme.palette.hairline)
			.child(rail)
			.child(page)
			.into_any_element();
		stack(vec![frame, Self::panel_variants(&theme)])
	}

	/// The settings parts the live page does not reach: panel tones, collapsed panels, a page
	/// of its own, the header, the hairline, and the glass panel.
	fn panel_variants(theme: &Theme) -> AnyElement {
		let panel = |id: &'static str, tone: Tone, title: &'static str| {
			div().w(px(260.)).child(
				settings_panel(id, title, Lucide::Settings).tone(tone).child(
					action_row("Row title")
						.detail("Row detail")
						.action(badge("Value").tone(Tone::Neutral)),
				),
			)
		};
		let collapsible = |id: &'static str, expanded: bool| {
			div().w(px(260.)).child(
				settings_panel(id, "Advanced", Icon::Gear)
					.collapsible(expanded, |_, _, _| {})
					.child(document_link(
						SharedString::from(format!("{id}-link")),
						"Open log files",
						Icon::Document,
					)),
			)
		};
		let page = div()
			.w(px(360.))
			.h(px(230.))
			.overflow_hidden()
			.rounded(theme.style.radii.panel)
			.border_1()
			.border_color(theme.palette.hairline)
			.bg(theme.palette.panel_background)
			.child(
				settings_page(
					"preview-settings-page",
					"Page title",
					"A page scrolls and ends in a panel.",
				)
				.child(
					settings_panel("settings-page-panel", "Panel", Lucide::Settings)
						.child(action_row("Row").action(toggle("settings-page-toggle", true))),
				),
			);
		stack(vec![
			state_row(
				"settings_panel · tone",
				"Neutral · Success · Warning · Danger",
				"settings_panel(id, title, icon).tone(Tone::Warning)",
				vec![
					cell("Neutral", panel("tone-neutral", Tone::Neutral, "Neutral")),
					cell("Success", panel("tone-success", Tone::Success, "Success")),
					cell("Warning", panel("tone-warning", Tone::Warning, "Warning")),
					cell("Danger", panel("tone-danger", Tone::Danger, "Danger")),
				],
			),
			state_row(
				"settings_panel · collapsible",
				"expanded and collapsed (static here; the live page above toggles)",
				"settings_panel(id, title, icon).collapsible(expanded, handler)",
				vec![
					cell("expanded", collapsible("collapsible-open", true)),
					cell("collapsed", collapsible("collapsible-closed", false)),
					cell(
						"danger_zone",
						div().w(px(260.)).child(
							danger_zone("variant-danger-zone").child(
								action_row("Remove files")
									.action(button("variant-remove", "Remove…").danger().compact()),
							),
						),
					),
				],
			),
			state_row(
				"settings_page · page_header",
				"the header, the rule under it, and a hairline",
				"settings_page(id, title, subtitle) · page_header(title, subtitle) · hairline()",
				vec![
					cell("settings_page", page),
					cell(
						"page_header",
						div()
							.w(px(320.))
							.child(page_header("Header", "Subtitle under the title.")),
					),
					cell("hairline", div().w(px(200.)).child(hairline())),
				],
			),
			state_row(
				"glass_panel",
				"a raised glass panel with the HUD radius",
				"glass_panel().child(content)",
				vec![cell(
					"glass_panel",
					div().w(px(260.)).child(
						glass_panel()
							.flex()
							.flex_col()
							.gap(px(8.))
							.child(div().font_weight(FontWeight::SEMIBOLD).child("Glass panel"))
							.child(label("Children go inside; the radius is fixed.")),
					),
				)],
			),
		])
		.into_any_element()
	}

	fn game_page(&self, cx: &mut Context<Self>) -> Div {
		div().flex().flex_col().gap(px(14.)).child(
			settings_panel("game-panel", "Game", Lucide::Gamepad2)
				.child(
					action_row("Display").detail("Window mode").action(
						segmented(
							"game-display",
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
						})),
					),
				)
				.child(
					action_row("Rendering").detail("Graphics backend").action(
						menu_picker(
							"game-rendering",
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
						})),
					),
				)
				.child(
					action_row("Game Mode")
						.detail("Pauses launcher tasks while playing")
						.action(
							toggle("panel-game-mode", self.game_mode)
								.label("Game Mode")
								.on_change(cx.listener(|this, on: &bool, _, cx| {
									this.game_mode = *on;
									cx.notify();
								})),
						),
				),
		)
	}

	fn appearance_page(&self) -> Div {
		div().flex().flex_col().gap(px(14.)).child(
			settings_panel("accent-panel", "Accent", Lucide::Palette).child(
				action_row("Accent colour")
					.detail("Changes with the accent shortcut")
					.action(badge(ACCENTS[self.accent_index]).tone(Tone::Accent)),
			),
		)
	}

	fn audio_page(&self, cx: &mut Context<Self>) -> Div {
		div().flex().flex_col().gap(px(14.)).child(
			settings_panel("audio-panel", "Audio", Lucide::Volume2).child(
				action_row("Volume").detail("Game and launcher sounds").action(
					slider("panel-volume", self.volume)
						.step(0.05)
						.width(px(200.))
						.leading_icon(Lucide::Volume1)
						.trailing_icon(Lucide::Volume2)
						.value_label(true)
						.on_change(cx.listener(|this, value: &f32, _, cx| {
							this.volume = *value;
							cx.notify();
						})),
				),
			),
		)
	}

	fn updates_page(&self, cx: &mut Context<Self>) -> Div {
		div()
			.flex()
			.flex_col()
			.gap(px(14.))
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
							.action(
								button("check-now", "Check Now")
									.accent()
									.compact()
									.icon(Lucide::RefreshCw),
							),
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
						self.panel_open,
						cx.listener(|this, _, _, cx| {
							this.panel_open = !this.panel_open;
							cx.notify();
						}),
					)
					.child(document_link("open-logs", "Open log files", Icon::Document))
					.child(document_link("open-folder", "Show game folder", Icon::Folder)),
			)
	}

	fn installation_page(&self) -> Div {
		div().flex().flex_col().gap(px(14.)).child(
			settings_panel("install-panel", "Installation", Lucide::Download)
				.child(
					action_row("Verify game files")
						.detail("Checks every file against the manifest")
						.action(button("verify", "Verify").compact()),
				)
				.child(
					action_row("Repair Wine prefix")
						.detail("Rebuilds the runtime and keeps your saves")
						.action(button("repair", "Repair").compact()),
				),
		)
	}

	fn storage_page(&self) -> Div {
		div()
			.flex()
			.flex_col()
			.gap(px(14.))
			.child(
				settings_panel("storage-panel", "Storage", Lucide::HardDrive)
					.child(document_link(
						"open-game-folder",
						"Show game folder",
						Icon::Folder,
					))
					.child(document_link("open-prefix", "Show Wine prefix", Icon::Folder)),
			)
			.child(
				danger_zone("danger").child(
					action_row("Remove game files")
						.detail("Settings and the Wine prefix stay")
						.action(
							button("remove", "Remove…")
								.danger()
								.compact()
								.icon(Lucide::Trash2),
						),
				),
			)
	}

	fn about_page(&self) -> Div {
		div()
			.flex()
			.flex_col()
			.gap(px(14.))
			.child(
				settings_panel("about-panel", "About", Icon::Info)
					.child(action_row("Version").action(badge(env!("CARGO_PKG_VERSION")).tone(Tone::Neutral)))
					.child(
						action_row("License")
							.detail("Mozilla Public License 2.0")
							.action(badge("MPL-2.0").tone(Tone::Neutral)),
					),
			)
			.child(
				settings_panel("links-panel", "Links", Icon::Document)
					.child(document_link("release-notes", "Release notes", Icon::Document))
					.child(document_link(
						"third-party",
						"Third-party licenses",
						Icon::Document,
					)),
			)
	}

	pub fn hud_section(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
		let rows = REGIONS.iter().enumerate().map(|(index, name)| {
			list_row(SharedString::from(format!("preview-region-{index}")), *name)
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
					pill("music", Icon::Music, || {
						div()
							.font_weight(FontWeight::MEDIUM)
							.child("Sanity Restored — Lobby Theme")
					})
					.detail(div(), px(330.), px(100.)),
				),
			)
			.child(
				div().flex().child(
					pill("version", Icon::Tag, || {
						div().font_weight(FontWeight::MEDIUM).child("2.6.81")
					})
					.detail(div(), px(260.), px(100.)),
				),
			)
			.child(
				div().flex().child(
					pill("receded", Icon::Tag, || div().child("Receded sibling"))
						.detail(div(), px(260.), px(100.))
						.receded(true),
				),
			);
		let expanded = pill("expanded-status", Icon::Globe, || {
			div().font_weight(FontWeight::SEMIBOLD).child("Global 04:12:09")
		})
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

	/// Pill morph: one pill opening and closing, and three pills in one glass group.
	pub fn morph_section(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
		const PILL_WIDTH: f32 = 72.;
		const REST: [f32; 3] = [0., 110., 220.];
		let theme = cx.theme();
		let single = div()
			.flex()
			.flex_col()
			.gap(px(10.))
			.child(label("One pill: button or O"))
			.child(
				div().h(px(130.)).w(px(330.)).child(
					pill("morph-pill", Icon::Sparkles, || {
						div().font_weight(FontWeight::MEDIUM).child("Morphing pill")
					})
					.noun("details")
					.detail(
						div().child("Size, corner and detail follow one spring."),
						px(330.),
						px(100.),
					)
					.expanded(self.morph_open)
					.on_toggle(cx.listener(|this, _, _, cx| {
						this.morph_open = !this.morph_open;
						cx.notify();
					})),
				),
			)
			.child(
				button(
					"morph-pill-toggle",
					if self.morph_open { "Close" } else { "Open" },
				)
				.on_click(cx.listener(|this, _, _, cx| {
					this.morph_open = !this.morph_open;
					cx.notify();
				})),
			);

		// The middle pill slides to its right neighbour; the glass between them fuses.
		let slide = spring_value(
			"morph-slide",
			if self.fuse_slid { 1. } else { 0. },
			Curve::State,
			theme.reduce_motion,
			window,
			cx,
		);
		let xs = [
			REST[0],
			REST[1] + (REST[2] - PILL_WIDTH - 2. - REST[1]) * slide,
			REST[2],
		];
		let mut group = glass_group("morph-group", px(self.fuse_spacing))
			.relative()
			.w(px(REST[2] + PILL_WIDTH))
			.h(px(28.));
		for (index, x) in xs.into_iter().enumerate() {
			group = group.member(
				px(14.),
				div()
					.absolute()
					.left(px(x))
					.top(px(0.))
					.w(px(PILL_WIDTH))
					.h(px(28.))
					.flex()
					.items_center()
					.justify_center()
					.text_size(tokens::text::CAPTION)
					.child(format!("Pill {}", index + 1)),
			);
		}
		let fused = div()
			.flex()
			.flex_col()
			.gap(px(12.))
			.child(label("Three pills in one glass_group: spacing fuses neighbours"))
			.child(group)
			.child(
				div()
					.flex()
					.flex_row()
					.items_center()
					.gap(px(12.))
					.child(label(format!("Spacing {:.0} px", self.fuse_spacing)))
					.child(
						slider("morph-spacing", self.fuse_spacing)
							.range(0.0..=40.0)
							.width(px(180.))
							.on_change(cx.listener(|this, value: &f32, _, cx| {
								this.fuse_spacing = *value;
								cx.notify();
							})),
					)
					.child(
						button(
							"morph-slide",
							if self.fuse_slid {
								"Slide back"
							} else {
								"Slide together"
							},
						)
						.on_click(cx.listener(|this, _, _, cx| {
							this.fuse_slid = !this.fuse_slid;
							cx.notify();
						})),
					),
			);
		div()
			.flex()
			.flex_row()
			.gap(px(48.))
			.items_start()
			.child(single)
			.child(fused)
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
					.dimensions(px(520.), px(330.))
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
							.child(skeleton("line-1").dimensions(px(320.), px(12.)).pulses(true))
							.child(skeleton("line-2").dimensions(px(240.), px(12.)).pulses(true)),
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
							floating_action_bar()
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
							.child(badge("Verified").tone(Tone::Color(theme.palette.success)))
							.child(badge("Failed").tone(Tone::Danger)),
					)
					.child(label("Skeleton placeholders"))
					.child(
						div()
							.flex()
							.flex_row()
							.gap(px(8.))
							.child(skeleton("sk-1").dimensions(px(64.), px(14.)))
							.child(skeleton("sk-2").dimensions(px(96.), px(14.)).pulses(true))
							.child(skeleton("sk-3").dimensions(px(96.), px(14.)).corner(px(7.)))
							.child(
								skeleton("sk-4")
									.dimensions(px(36.), px(36.))
									.corner(px(18.))
									.pulses(true),
							),
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
