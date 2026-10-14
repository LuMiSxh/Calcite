// SPDX-License-Identifier: MPL-2.0

//! Component preview: every Calcite component and glass style once, on one page.
//!
//! Keys: see [`SHORTCUTS`]; the panel at the top right lists them (`?` folds it). Tab moves focus.

mod cli;
mod coverage;
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
	EditableTextState, UI_FONT,
	app_icon::AppIcon,
	chrome, keymap,
	prelude::*,
	tokens::{rgba as color, scale_alpha},
};
use calcite_showcase::artwork::{SLASH_ASSET, SLASH_SVG};
use gpui::{
	App, Bounds, Context, Entity, FocusHandle, IntoElement, ParentElement, Rgba, ScrollHandle,
	StyleRefinement, Styled, Window, actions, div, point, prelude::*, px, rgba, size,
};
use motion_lab::MotionLab;
use section_view::{Group, Kind, PageView};
use sections::{BackdropView, label};

actions!(
	preview,
	[
		CycleGlass,
		CycleAccent,
		CyclePalette,
		ToggleReduceMotion,
		ToggleShortcuts,
		TogglePill,
		NextSection,
		PreviousSection,
		Quit
	]
);

/// Declares the preview shortcuts once: the legend list and the key bindings come from the
/// same rows, so they cannot drift apart.
macro_rules! shortcuts {
	($($key:literal => $action:expr, $label:literal;)*) => {
		/// `(key, what it does)` for every preview shortcut.
		pub const SHORTCUTS: &[(&str, &str)] = &[$(($key, $label)),*];

		fn bind_shortcuts(cx: &mut App) {
			keymap!(cx, { "Preview" => { $($key => $action),* } });
		}
	};
}

shortcuts! {
	"g" => CycleGlass, "Next glass style";
	"a" => CycleAccent, "Next accent";
	"p" => CyclePalette, "Next palette: Arknights, Ember, Light";
	"m" => ToggleReduceMotion, "Toggle Reduce Motion";
	"o" => TogglePill, "Open or close the morph pill";
	"]" => NextSection, "Next section in the sidebar";
	"[" => PreviousSection, "Previous section in the sidebar";
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

/// The library's light look.
const LIGHT: Palette = Palette::LIGHT;

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

pub struct Preview {
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
	pub toggles: [bool; 6],
	/// The live two-option and long-label segmented controls of the Forms section.
	pub segments: [usize; 2],
	/// The glass style to return to when Reduce Transparency goes off.
	glass_before: GlassStyle,
	/// Which confirm dialog the Notices buttons opened.
	pub confirm_role: ConfirmRole,
	/// The menu_picker with six options.
	pub locale: usize,
	/// A text field that starts filled.
	pub filled: Entity<EditableTextState>,
	/// Text fields of the Forms matrix: plain, overflowing, and invalid.
	pub plain: Entity<EditableTextState>,
	pub long: Entity<EditableTextState>,
	pub invalid: Entity<EditableTextState>,
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
	/// The section the sidebar shows; the page shows only this one.
	pub section: Kind,
	/// The last dark palette, so the Dark segment brings back Ember after Ember, not Arknights.
	dark_palette: Palette,
	/// The scroll position of the section page; back to the top on each change.
	scroll: ScrollHandle,
	/// The sections, one view each, so an animation re-renders only its own section.
	layout: Entity<PageView>,
	backdrop: Entity<BackdropView>,
}

impl Preview {
	fn new(window: &mut Window, cx: &mut Context<Self>, section: Kind) -> Self {
		let focus = cx.focus_handle();
		window.focus(&focus, cx);
		let preview = cx.entity();
		let layout = PageView::new(&preview, section, cx);
		let dark_palette = cx.theme().palette;
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
			shortcuts_open: false,
			morph_open: false,
			fuse_spacing: 20.,
			fuse_slid: false,
			toggles: [false, true, false, true, false, true],
			segments: [0, 1],
			glass_before: Style::default().glass,
			confirm_role: ConfirmRole::Destructive,
			locale: 0,
			filled: text_state("Amiya", cx),
			plain: text_state("", cx),
			long: text_state(
				"A search that is far longer than the field is wide, so the text scrolls",
				cx,
			),
			invalid: text_state("ab", cx),
			icon_query,
			icon_text: String::new(),
			icon_page: 0,
			ticker_percent: 0.42,
			ticker_count: 1280,
			motion_lab: cx.new(|_| MotionLab::new()),
			section,
			dark_palette: if dark_palette.is_light() {
				Palette::ARKNIGHTS
			} else {
				dark_palette
			},
			scroll: ScrollHandle::new(),
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

impl Preview {
	/// Shows `section` and scrolls the page back to its top.
	fn select(&mut self, section: Kind, cx: &mut Context<Self>) {
		if self.section != section {
			self.section = section;
			self.scroll.set_offset(point(px(0.), px(0.)));
			cx.notify();
		}
	}

	/// The appearance toolbar above the page: light or dark, Reduce Motion, Reduce Transparency,
	/// and the accent. Every section is shown in whatever these say, so any of them can be seen
	/// in each mode. It reads the target theme, so it follows the keyboard shortcuts too.
	fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
		let theme = cx.target_theme();
		let dark = theme.palette != LIGHT;
		div()
			.flex_none()
			.flex()
			.flex_row()
			.flex_wrap()
			.items_center()
			.gap_x(px(22.))
			.gap_y(px(6.))
			.pt(px(48.))
			.pb(px(8.))
			.pl(px(36.))
			.pr(px(80.))
			.bg(scale_alpha(theme.palette.backdrop, 0.9))
			.border_b_1()
			.border_color(theme.palette.hairline)
			.child(label("APPEARANCE"))
			.child(
				segmented("toolbar-mode", dark, [(true, "Dark"), (false, "Light")]).on_change(cx.listener(
					|this, dark: &bool, _, cx| {
						let palette = if *dark { this.dark_palette } else { LIGHT };
						Theme::update(cx, move |theme| *theme = theme.with_palette(palette));
					},
				)),
			)
			.child(
				div()
					.flex()
					.flex_row()
					.items_center()
					.gap(px(8.))
					.child(
						toggle("toolbar-motion", theme.reduce_motion)
							.label("Reduce Motion")
							.compact()
							.on_change(cx.listener(|_, on: &bool, _, cx| {
								let on = *on;
								Theme::update(cx, move |theme| theme.reduce_motion = on);
							})),
					)
					.child(label("Reduce Motion")),
			)
			.child(
				div()
					.flex()
					.flex_row()
					.items_center()
					.gap(px(8.))
					.child(
						toggle("toolbar-transparency", theme.reduce_transparency())
							.label("Reduce Transparency")
							.compact()
							.on_change(cx.listener(|this, on: &bool, _, cx| {
								let glass = if *on {
									let current = cx.target_theme().style.glass;
									if current != GlassStyle::ReducedTransparency {
										this.glass_before = current;
									}
									GlassStyle::ReducedTransparency
								} else {
									this.glass_before
								};
								Theme::update(cx, move |theme| theme.style.glass = glass);
							})),
					)
					.child(label("Reduce Transparency")),
			)
			.child(
				segmented(
					"toolbar-accent",
					self.accent_index,
					interactive::ACCENTS.iter().copied().enumerate(),
				)
				.on_change(cx.listener(|this, index: &usize, _, cx| {
					this.accent_index = *index;
					let accent = accents()[*index];
					Theme::update(cx, move |theme| *theme = theme.with_accent(accent));
				})),
			)
	}

	/// Moves `step` places along the sidebar order, wrapping at both ends.
	fn step_section(&mut self, step: isize, cx: &mut Context<Self>) {
		// The sidebar order: group by group, each group's sections in `Kind::ALL` order.
		let order: Vec<Kind> = Group::ALL
			.into_iter()
			.flat_map(|group| Kind::ALL.into_iter().filter(move |kind| kind.group() == group))
			.collect();
		let count = order.len() as isize;
		let index = order.iter().position(|kind| *kind == self.section).unwrap_or(0) as isize;
		let next = order[(index + step).rem_euclid(count) as usize];
		self.select(next, cx);
	}

	/// The sidebar: every section under its group. The shown section is raised; a click shows it.
	fn sidebar(&self, theme: &Theme, cx: &mut Context<Self>) -> impl IntoElement + use<> {
		let mut rail = nav_rail("preview-sidebar")
			.flex_none()
			.w(px(224.))
			.scrollable()
			.pt(px(56.))
			.gap(px(14.));
		for group in Group::ALL {
			rail = rail.child(
				div()
					.flex()
					.flex_col()
					.gap(px(2.))
					.child(
						div()
							.px(px(12.))
							.text_size(px(10.))
							.font_weight(gpui::FontWeight::SEMIBOLD)
							.text_color(theme.palette.secondary_text)
							.child(group.title().to_uppercase()),
					)
					.children(
						Kind::ALL
							.into_iter()
							.filter(|kind| kind.group() == group)
							.map(|kind| {
								nav_item(format!("section-{}", kind.title()), kind.title(), kind.icon())
									.selected(self.section == kind)
									.on_click(cx.listener(move |this, _, _, cx| this.select(kind, cx)))
							}),
					),
			);
		}
		rail
	}
}

impl Render for Preview {
	fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
		let theme = cx.theme();

		let root =
			div()
				.size_full()
				.relative()
				.overflow_hidden()
				.bg(theme.palette.backdrop)
				.font_family(UI_FONT)
				.text_color(theme.palette.primary_text)
				.track_focus(&self.focus)
				.key_context("Preview")
				.on_action(cx.listener(|this, _: &CycleAccent, _, cx| {
					this.accent_index = (this.accent_index + 1) % accents().len();
					let accent = accents()[this.accent_index];
					Theme::update(cx, |theme| *theme = theme.with_accent(accent));
				}))
				.on_action(cx.listener(|this, _: &CyclePalette, _, cx| {
					let current = cx.target_theme().palette;
					let next = if current == EMBER {
						LIGHT
					} else if current == LIGHT {
						Palette::ARKNIGHTS
					} else {
						EMBER
					};
					if next != LIGHT {
						this.dark_palette = next;
					}
					Theme::update(cx, |theme| *theme = theme.with_palette(next));
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
				.on_action(cx.listener(|this, _: &NextSection, _, cx| this.step_section(1, cx)))
				.on_action(cx.listener(|this, _: &PreviousSection, _, cx| this.step_section(-1, cx)))
				.on_action(cx.listener(|this, _: &TogglePill, _, cx| {
					this.morph_open = !this.morph_open;
					cx.notify();
				}))
				.child(
					div()
						.size_full()
						.flex()
						.flex_row()
						.child(self.sidebar(&theme, cx))
						.child(
							div()
								.flex_1()
								.min_w_0()
								.h_full()
								.flex()
								.flex_col()
								.child(self.toolbar(cx))
								.child(
									div()
										.id("preview-scroll")
										.flex_1()
										.min_h_0()
										.w_full()
										.overflow_y_scroll()
										.track_scroll(&self.scroll)
										.child(
											// The container grows with the content; the backdrop fills it.
											div()
												.relative()
												.w_full()
												.min_h_full()
												.child(self.backdrop.clone().cached(
													StyleRefinement::default().absolute().size_full(),
												))
												.child(self.layout.clone()),
										),
								),
						),
				)
				.child(self.shortcut_legend(&theme, cx))
				.child(
					modal_layer(
						"preview-modal",
						modal("preview-modal-sheet", "Reset launcher settings?")
							.dimensions(px(520.), px(320.))
							.child(div().child("This restores every launcher setting. Your game data stays."))
							.action(button("modal-cancel", "Cancel").on_click(cx.listener(
								|this, _, _, cx| {
									this.modal_open = false;
									cx.notify();
								},
							)))
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
					match self.confirm_role {
						ConfirmRole::Destructive => confirm_dialog(
							"preview-confirm",
							"Uninstall the game?",
							"The game for this server moves to the Trash. Settings and logins stay.",
						)
						.confirm("Uninstall", ConfirmRole::Destructive),
						ConfirmRole::Default => confirm_dialog(
							"preview-confirm",
							"Reset the layout?",
							"Panels and the sidebar return to their default places.",
						)
						.confirm("Reset", ConfirmRole::Default),
					}
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
				.child(title_bar("preview-title-bar"));
		window_frame(root)
	}
}

fn main() {
	let options = cli::Options::from_env();
	let section = options.section.unwrap_or(Kind::Glass);
	gpui_platform::application()
		.with_assets(Assets::default().with(SLASH_ASSET, SLASH_SVG))
		.run(move |cx: &mut App| {
			calcite::init_with(cx, options.theme(Theme::from_system(cx)));
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
			let display = cx.primary_display().map(|display| display.bounds().size);
			let width = display.map_or(px(WIDTH), |display| px(WIDTH).min(display.width - px(40.)));
			let height = display.map_or(px(900.), |display| display.height - px(80.));
			let bounds = Bounds::centered(None, size(width, height), cx);
			cx.open_window(
				ICON.window_options(chrome::window_options("Calcite - preview", bounds)),
				|window, cx| cx.new(|cx| Preview::new(window, cx, section)),
			)
			.expect("failed to open the preview window");
			cx.activate(true);
		});
}
