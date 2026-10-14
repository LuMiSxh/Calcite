// SPDX-License-Identifier: MPL-2.0

//! Component gallery: every Calcite component and glass style once, on one page.
//!
//! Keys: G next glass style, A next accent, P next palette, M Reduce Motion, Tab moves focus.

mod interactive;
mod sections;
mod setup;

use calcite::{
	EditableTextState, UI_FONT, app_icon::AppIcon, chrome, keymap, prelude::*, tokens::rgba as color,
};
use calcite_showcase::artwork::{SLASH_ASSET, SLASH_SVG};
use gpui::{
	App, Bounds, Context, Entity, FocusHandle, IntoElement, ParentElement, Rgba, Styled, Window, actions,
	div, prelude::*, px, rgba, size,
};

actions!(
	gallery,
	[CycleGlass, CycleAccent, CyclePalette, ToggleReduceMotion, Quit]
);

/// A second look, picked at compile time: warm neutrals and an amber fallback accent.
const EMBER: Palette = Palette {
	accent: color(0.96, 0.62, 0.30, 1.),
	control: color(0.78, 0.72, 0.66, 1.),
	quiet_control: color(0.64, 0.58, 0.52, 1.),
	backdrop: color(0.05, 0.035, 0.03, 1.),
	panel_background: color(0.09, 0.07, 0.06, 1.),
	navigation_rail: color(0.10, 0.05, 0.02, 0.32),
	..Palette::ARKNIGHTS
};

pub const WIDTH: f32 = 1480.;
pub const HEIGHT: f32 = 4450.;

/// The search tags the chip demo starts with.
pub const TAGS: [&str; 3] = ["Amiya", "Summer", "Animated"];

/// The Dock, task bar, and window icon outside a bundle.
static ICON: AppIcon = calcite::app_icon!("assets/icon.png");

fn accents() -> [Option<Rgba>; 4] {
	[
		None,
		Some(rgba(0xF59E38FF)),
		Some(rgba(0xF05C85FF)),
		Some(rgba(0x9E80FAFF)),
	]
}

#[derive(Clone, Copy, PartialEq)]
pub enum Display {
	Windowed,
	Borderless,
	Fullscreen,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Rendering {
	Retina,
	MetalFx,
	Standard,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Page {
	Game,
	Appearance,
	Audio,
	Updates,
	Installation,
	Storage,
	About,
}

impl Page {
	pub const ALL: [Page; 7] = [
		Page::Game,
		Page::Appearance,
		Page::Audio,
		Page::Updates,
		Page::Installation,
		Page::Storage,
		Page::About,
	];

	pub fn title(self) -> &'static str {
		match self {
			Page::Game => "Game",
			Page::Appearance => "Appearance",
			Page::Audio => "Audio",
			Page::Updates => "Updates",
			Page::Installation => "Installation",
			Page::Storage => "Storage",
			Page::About => "About",
		}
	}

	pub fn icon(self) -> Icon {
		match self {
			Page::Game => Icon::Play,
			Page::Appearance => Icon::Sparkles,
			Page::Audio => Icon::Music,
			Page::Updates => Icon::ArrowClockwise,
			Page::Installation => Icon::ArrowDown,
			Page::Storage => Icon::Folder,
			Page::About => Icon::Info,
		}
	}
}

pub struct Gallery {
	focus: FocusHandle,
	accent_index: usize,
	pub auto_update: bool,
	pub game_mode: bool,
	pub volume: f32,
	pub display: Display,
	pub rendering: Rendering,
	pub search: Entity<EditableTextState>,
	pub page: Page,
	pub panel_open: bool,
	pub region: usize,
	pub modal_open: bool,
	pub artwork_choice: usize,
	pub colors_choice: usize,
	pub extras: [bool; 2],
	pub chip_selected: Option<usize>,
	pub tags: Vec<&'static str>,
	pub confirm_open: bool,
}

impl Gallery {
	fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
		let focus = cx.focus_handle();
		window.focus(&focus, cx);
		Self {
			focus,
			accent_index: 0,
			auto_update: true,
			game_mode: false,
			volume: 0.6,
			display: Display::Windowed,
			rendering: Rendering::Retina,
			search: text_state("", cx),
			page: Page::Updates,
			panel_open: true,
			region: 0,
			modal_open: false,
			artwork_choice: 0,
			colors_choice: 0,
			extras: [true, false],
			chip_selected: None,
			tags: TAGS.to_vec(),
			confirm_open: false,
		}
	}
}

impl Render for Gallery {
	fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
		let theme = cx.theme();
		let content = div()
			.relative()
			.w_full()
			.pt(px(44.))
			.pb(px(48.))
			.px(px(36.))
			.flex()
			.flex_col()
			.gap(px(30.))
			.child(sections::section(
				"Glass styles",
				format!(
					"G cycles the glass · A the accent · P the palette · current: {}",
					theme.glass.label()
				),
				sections::glass_styles(theme),
			))
			.child(sections::section(
				"Actions",
				"button · icon_button · halo · done_button · glass_tooltip",
				self.actions_section(),
			))
			.child(sections::section(
				"Forms",
				"toggle · slider · segmented · menu_picker · text_field · action_menu",
				self.forms_section(cx),
			))
			.child(sections::section(
				"Settings",
				"nav_rail · page_header · settings_panel · action_row · danger_zone",
				self.settings_section(cx),
			))
			.child(sections::section(
				"HUD",
				"pill · control_bar · status · list_row",
				self.hud_section(cx),
			))
			.child(sections::section(
				"Overlay and feedback",
				"modal · floating_action_bar · link · badge · skeleton",
				self.overlay_section(cx),
			))
			.child(sections::section(
				"Setup assistant",
				"step_rail · step · choice_group · choice_card · tag",
				self.setup_section(cx),
			))
			.child(sections::section(
				"Notices",
				"banner · spinner · progress_bar · chip · empty_state · confirm_dialog",
				self.notices_section(cx),
			))
			.child(sections::section(
				"Window chrome",
				"window_frame · title_bar · ChromeStyle",
				sections::chrome_section(),
			))
			.child(sections::section(
				"Icons",
				"icons! macro · built-in Icon set",
				sections::icons_section(&theme),
			))
			.child(sections::section(
				"Tokens",
				"colors · radii · typography",
				sections::tokens_section(theme),
			))
			.child(sections::section(
				"Motion",
				"LauncherMotion curves as springs",
				sections::motion_section(theme.accent),
			));

		let root = div()
			.size_full()
			.relative()
			.overflow_hidden()
			.font_family(UI_FONT)
			.text_color(theme.palette.primary_text)
			.track_focus(&self.focus)
			.key_context("Gallery")
			.on_action(cx.listener(|this, _: &CycleAccent, _, cx| {
				this.accent_index = (this.accent_index + 1) % accents().len();
				let accent = accents()[this.accent_index];
				Theme::update(cx, |theme| *theme = theme.with_accent(accent));
			}))
			.on_action(cx.listener(|_, _: &CyclePalette, _, cx| {
				Theme::update(cx, |theme| {
					let next = if theme.palette == EMBER {
						Palette::ARKNIGHTS
					} else {
						EMBER
					};
					*theme = theme.with_palette(next);
				})
			}))
			.on_action(cx.listener(|_, _: &CycleGlass, _, cx| {
				Theme::update(cx, |theme| theme.glass = theme.glass.next())
			}))
			.on_action(cx.listener(|_, _: &ToggleReduceMotion, _, cx| {
				Theme::update(cx, |theme| theme.reduce_motion = !theme.reduce_motion)
			}))
			.child(
				div().id("gallery-scroll").size_full().overflow_y_scroll().child(
					div()
						.relative()
						.w_full()
						.h(px(HEIGHT))
						.child(sections::backdrop(&theme))
						.child(content),
				),
			)
			.child(
				modal_layer(
					"gallery-modal",
					modal("gallery-modal-sheet", "Reset launcher settings?")
						.size(px(520.), px(320.))
						.child(div().child("This restores every launcher setting. Your game data stays."))
						.action(
							button("modal-cancel", "Cancel").on_click(cx.listener(|this, _, _, cx| {
								this.modal_open = false;
								cx.notify();
							})),
						)
						.action(
							button("modal-reset", "Reset")
								.danger()
								.large()
								.on_click(cx.listener(|this, _, _, cx| {
									this.modal_open = false;
									cx.notify();
								})),
						),
				)
				.shown(self.modal_open)
				.on_dismiss(cx.listener(|this, _, _, cx| {
					this.modal_open = false;
					cx.notify();
				})),
			)
			.child(
				confirm_dialog(
					"gallery-confirm",
					"Uninstall the game?",
					"The game for this server moves to the Trash. Settings and logins stay.",
				)
				.confirm("Uninstall", ConfirmRole::Destructive)
				.shown(self.confirm_open)
				.on_confirm(cx.listener(|this, _, _, cx| {
					this.confirm_open = false;
					cx.notify();
				}))
				.on_cancel(cx.listener(|this, _, _, cx| {
					this.confirm_open = false;
					cx.notify();
				})),
			)
			.child(title_bar("gallery-title-bar"));
		window_frame(root)
	}
}

fn main() {
	gpui_platform::application()
		.with_assets(Assets::default().with(SLASH_ASSET, SLASH_SVG))
		.run(|cx: &mut App| {
			calcite::init(cx);
			ICON.install(cx);
			keymap!(cx, {
				"Gallery" => { "g" => CycleGlass, "a" => CycleAccent, "p" => CyclePalette, "m" => ToggleReduceMotion },
				_ => { "cmd-q" => Quit, "ctrl-q" => Quit },
			});
			cx.on_action(|_: &Quit, cx| cx.quit());
			cx.on_window_closed(|cx, _| {
				if cx.windows().is_empty() {
					cx.quit();
				}
			})
			.detach();
			// The page scrolls, so the window only needs to fit the display.
			let display_height = cx
				.primary_display()
				.map_or(px(HEIGHT), |display| display.bounds().size.height - px(80.));
			let bounds = Bounds::centered(None, size(px(WIDTH), px(HEIGHT).min(display_height)), cx);
			cx.open_window(
				ICON.window_options(chrome::window_options("Calcite Gallery", bounds)),
				|window, cx| cx.new(|cx| Gallery::new(window, cx)),
			)
			.expect("failed to open the gallery window");
			cx.activate(true);
		});
}
