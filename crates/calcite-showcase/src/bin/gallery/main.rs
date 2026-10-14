// SPDX-License-Identifier: MPL-2.0

//! Component gallery: every Calcite component and glass style once, on one page.
//!
//! Keys: see [`SHORTCUTS`]; the panel at the top right lists them (`?` folds it). Tab moves focus.

mod icon_search;
mod interactive;
mod menus;
mod motion_lab;
mod section_view;
mod sections;
mod selection;
mod setup;
mod states;
mod style_demo;
mod typography;

use calcite::{
	EditableTextState, UI_FONT, app_icon::AppIcon, chrome, keymap, prelude::*, tokens::rgba as color,
};
use calcite_showcase::artwork::{SLASH_ASSET, SLASH_SVG};
use gpui::{
	App, Bounds, Context, Entity, FocusHandle, IntoElement, ParentElement, Rgba, StyleRefinement, Styled,
	Window, actions, div, prelude::*, px, rgba, size,
};
use motion_lab::MotionLab;
use section_view::PageView;
use sections::BackdropView;

actions!(
	gallery,
	[
		CycleGlass,
		CycleAccent,
		CyclePalette,
		ToggleReduceMotion,
		ToggleShortcuts,
		TogglePill,
		Quit
	]
);

/// Declares the gallery shortcuts once: the legend list and the key bindings come from the
/// same rows, so they cannot drift apart.
macro_rules! shortcuts {
	($($key:literal => $action:expr, $label:literal;)*) => {
		/// `(key, what it does)` for every gallery shortcut.
		pub const SHORTCUTS: &[(&str, &str)] = &[$(($key, $label)),*];

		fn bind_shortcuts(cx: &mut App) {
			keymap!(cx, { "Gallery" => { $($key => $action),* } });
		}
	};
}

shortcuts! {
	"g" => CycleGlass, "Next glass style";
	"a" => CycleAccent, "Next accent";
	"p" => CyclePalette, "Arknights or Ember palette";
	"m" => ToggleReduceMotion, "Toggle Reduce Motion";
	"o" => TogglePill, "Open or close the morph pill";
	"?" => ToggleShortcuts, "Show or hide this list";
}

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

	pub fn subtitle(self) -> &'static str {
		match self {
			Page::Game => "Window, rendering and play options.",
			Page::Appearance => "Accent colour and interface look.",
			Page::Audio => "Volume for game and launcher sounds.",
			Page::Updates => "Keep the launcher and the game current.",
			Page::Installation => "Verify and repair the game install.",
			Page::Storage => "Game files, folders and removal.",
			Page::About => "Version, license and project links.",
		}
	}

	pub fn icon(self) -> Lucide {
		match self {
			Page::Game => Lucide::Gamepad2,
			Page::Appearance => Lucide::Palette,
			Page::Audio => Lucide::Volume2,
			Page::Updates => Lucide::RefreshCw,
			Page::Installation => Lucide::Download,
			Page::Storage => Lucide::HardDrive,
			Page::About => Lucide::Info,
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
	pub shortcuts_open: bool,
	pub morph_open: bool,
	pub fuse_spacing: f32,
	pub fuse_slid: bool,
	/// The live switches of the Toggle row.
	pub toggles: [bool; 4],
	/// The menu_picker with six options.
	pub locale: usize,
	/// A text field that starts filled.
	pub filled: Entity<EditableTextState>,
	pub icon_query: Entity<EditableTextState>,
	/// The icon search text as of the last change, for the section's fingerprint.
	pub icon_text: String,
	pub icon_page: usize,
	/// The percent of the Rolling numbers demo, from its slider (0..=1).
	pub ticker_percent: f32,
	/// The counter of the Rolling numbers demo.
	pub ticker_count: i64,
	/// The Motion lab's own state: its springs, the running demo, and the loop.
	pub motion_lab: Entity<MotionLab>,
	/// The sections, one view each, so an animation re-renders only its own section.
	layout: Entity<PageView>,
	backdrop: Entity<BackdropView>,
}

impl Gallery {
	fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
		let focus = cx.focus_handle();
		window.focus(&focus, cx);
		let gallery = cx.entity();
		let layout = PageView::new(&gallery, cx);
		let icon_query = text_state("", cx);
		// Typing in the search restarts at the first page.
		cx.observe(&icon_query, |this, query, cx| {
			let text = query.read(cx).as_str();
			if text != this.icon_text {
				this.icon_text = text.to_string();
				this.icon_page = 0;
				cx.notify();
			}
		})
		.detach();
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
			shortcuts_open: true,
			morph_open: false,
			fuse_spacing: 20.,
			fuse_slid: false,
			toggles: [false, true, false, true],
			locale: 0,
			filled: text_state("Amiya", cx),
			icon_query,
			icon_text: String::new(),
			icon_page: 0,
			ticker_percent: 0.42,
			ticker_count: 1280,
			motion_lab: cx.new(|_| MotionLab::new()),
			layout,
			backdrop: cx.new(|_| BackdropView),
		}
	}

	/// The always-on-top shortcut legend, pinned below the title bar. Folded, only the round toggle
	/// remains. Unfolded, the card grows out of the toggle as a second piece of glass fused with
	/// it, so the open legend is one surface and the toggle is just its icon. The toggle never
	/// moves or changes; clicks outside reach the page below.
	fn shortcut_legend(&self, theme: &Theme, cx: &mut Context<Self>) -> impl IntoElement + use<> {
		const TOGGLE: f32 = 28.;
		const CARD_WIDTH: f32 = 260.;
		const PADDING: f32 = 14.;
		let rows = SHORTCUTS.len() as f32;
		let card_height = 2. * PADDING + 24. + rows * 28. + (rows - 1.) * 2.;
		let open = self.shortcuts_open;
		let hud_radius: f32 = theme.style.radii.hud_pill.into();
		let toggle = cx.listener(|this, _, _, cx| {
			this.shortcuts_open = !this.shortcuts_open;
			cx.notify();
		});

		let legend = attached_surface("shortcut-legend")
			.open(open)
			.align(Align::End)
			.placement(Placement::Down)
			// The card covers the toggle: its top right corner is the toggle's.
			.offset(-TOGGLE)
			.trigger_radius(TOGGLE / 2.)
			.trigger(move |state, _, _| {
				// The hit target never moves: the button's 44 pt target is centered on the 28 pt toggle.
				div()
					.relative()
					.size(px(TOGGLE))
					.child(
						div().absolute().top(px(-8.)).right(px(-8.)).child(
							icon_button(
								"shortcuts-toggle",
								if state.unit > 0.5 {
									Icon::ChevronUp
								} else {
									Icon::Info
								},
								if open { "Hide shortcuts" } else { "Show shortcuts" },
							)
							.diameter(px(TOGGLE))
							.surface_opacity(state.rim)
							.on_click(toggle),
						),
					)
					.into_any_element()
			})
			.body((CARD_WIDTH, card_height), hud_radius, move |_, _| {
				div()
					.w(px(CARD_WIDTH))
					.p(px(PADDING))
					.flex()
					.flex_col()
					.gap(px(2.))
					.text_size(px(12.))
					.child(
						div()
							.pb(px(4.))
							.font_weight(gpui::FontWeight::SEMIBOLD)
							.child("Shortcuts"),
					)
					.children(SHORTCUTS.iter().map(|(key, label)| {
						list_row(format!("shortcut-{key}"), *label).value(key.to_uppercase())
					}))
					.into_any_element()
			});

		div()
			.absolute()
			.top(px(60.))
			.right(px(24.))
			.size(px(TOGGLE))
			.child(legend)
	}
}

impl Render for Gallery {
	fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
		let theme = cx.theme();

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
				Theme::update(cx, |theme| theme.style.glass = theme.style.glass.next())
			}))
			.on_action(cx.listener(|_, _: &ToggleReduceMotion, _, cx| {
				Theme::update(cx, |theme| theme.reduce_motion = !theme.reduce_motion)
			}))
			.on_action(cx.listener(|this, _: &ToggleShortcuts, _, cx| {
				this.shortcuts_open = !this.shortcuts_open;
				cx.notify();
			}))
			.on_action(cx.listener(|this, _: &TogglePill, _, cx| {
				this.morph_open = !this.morph_open;
				cx.notify();
			}))
			.child(
				div().id("gallery-scroll").size_full().overflow_y_scroll().child(
					// The container grows with the content; the backdrop fills it.
					div()
						.relative()
						.w_full()
						.child(
							self.backdrop
								.clone()
								.cached(StyleRefinement::default().absolute().size_full()),
						)
						.child(self.layout.clone()),
				),
			)
			.child(self.shortcut_legend(&theme, cx))
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
			bind_shortcuts(cx);
			keymap!(cx, { _ => { "cmd-q" => Quit, "ctrl-q" => Quit } });
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
				.map_or(px(900.), |display| display.bounds().size.height - px(80.));
			let bounds = Bounds::centered(None, size(px(WIDTH), display_height), cx);
			cx.open_window(
				ICON.window_options(chrome::window_options("Calcite Gallery", bounds)),
				|window, cx| cx.new(|cx| Gallery::new(window, cx)),
			)
			.expect("failed to open the gallery window");
			cx.activate(true);
		});
}
