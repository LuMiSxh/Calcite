// SPDX-License-Identifier: MPL-2.0

//! The launcher home screen with a simulated install, download, verification, and game
//! session. It contains no launcher logic and starts no game.
//!
//! Run: `cargo run --release --bin home` (optionally `-- /path/to/artwork.jpg`).

// Release builds open no console window next to the app on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
	path::PathBuf,
	time::{Duration, SystemTime, UNIX_EPOCH},
};

use calcite::{UI_FONT, app_icon::AppIcon, artwork::cover_image, chrome, prelude::*, tokens::hud};
use calcite_exec::prelude::*;
use calcite_showcase::artwork::{Artwork, SLASH_ASSET, SLASH_SVG, procedural, readability_field, wordmark};
use gpui::{
	App, Bounds, Context, FocusHandle, FontWeight, IntoElement, MouseButton, SharedString, Task, Window,
	WindowOptions, actions, div, px, rgba, size,
};

actions!(
	home,
	[
		PrimaryAction,
		CycleAccent,
		CycleGlass,
		ToggleReduceMotion,
		ToggleUpdateButton,
		ToggleMusicPill,
		ToggleVersionPill,
		ToggleStatusPill,
		ToggleArtwork,
		ToggleLegend,
		Quit,
	]
);

const DOWNLOAD_BYTES: f64 = 3.42e9;
const DOWNLOAD_RATE: f64 = 38.6e6;
/// The simulated transfer runs faster than the displayed rate so the demo stays short.
const DEMO_SPEEDUP: f64 = 9.;

/// The exclusive activity, as in `LauncherLifecycleStore`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Activity {
	#[default]
	Idle,
	Downloading,
	Verifying,
	Running,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
	NotInstalled,
	Downloading,
	Paused,
	Verifying,
	Ready,
	Running,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pill {
	Music,
	Version,
	Status,
}

/// Name and daily reset hour in UTC of each server.
const REGIONS: [(&str, u64); 5] = [
	("Global", 11),
	("Japan", 19),
	("Korea", 19),
	("China", 20),
	("China (Bilibili)", 20),
];

/// Accent presets that stand in for colors sampled from artwork. `None` is the fallback.
fn accents() -> [Option<gpui::Rgba>; 5] {
	[
		None,
		Some(rgba(0xF59E38FF)),
		Some(rgba(0xF05C85FF)),
		Some(rgba(0x9E80FAFF)),
		Some(rgba(0x5CDB94FF)),
	]
}

struct Home {
	focus: FocusHandle,
	phase: Phase,
	progress: f64,
	accent_index: usize,
	shows_update_button: bool,
	expanded: Option<Pill>,
	region: usize,
	halo_generation: usize,
	shows_legend: bool,
	artwork: Artwork,
	custom_artwork: Option<PathBuf>,
	music_playing: bool,
	activity: ActivityStore<Activity>,
	lease: Option<ActivityLease>,
	transfer: TaskSlot,
	_clock: Task<()>,
}

impl Home {
	/// Dynamic Theme: the accent and the HUD tint hue follow the artwork; the crossfade comes
	/// from the theme. A result for artwork that is no longer shown is dropped.
	fn apply_artwork_theme(&mut self, cx: &mut Context<Self>) {
		let Artwork::Image(path) = self.artwork.clone() else {
			Theme::set_artwork_tint(cx, None);
			return;
		};
		cx.spawn(async move |this, cx| {
			let source = path.clone();
			let tint = cx
				.background_executor()
				.spawn(async move { artwork_tint(&source) })
				.await;
			this.update(cx, |this, cx| {
				if this.artwork == Artwork::Image(path) {
					Theme::set_artwork_tint(cx, tint);
				}
			})
			.ok();
		})
		.detach();
	}

	fn new(custom_artwork: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) -> Self {
		let focus = cx.focus_handle();
		window.focus(&focus, cx);
		cx.redraw_on_window_activation(window);
		// Redraws the reset countdown once per second.
		let clock = cx.spawn(async move |this, cx| {
			loop {
				cx.background_executor().timer(Duration::from_secs(1)).await;
				if this.update(cx, |_, cx| cx.notify()).is_err() {
					break;
				}
			}
		});
		Self {
			focus,
			phase: Phase::NotInstalled,
			progress: 0.,
			accent_index: 0,
			shows_update_button: false,
			expanded: None,
			region: 0,
			halo_generation: 0,
			shows_legend: true,
			artwork: custom_artwork.clone().map_or(Artwork::Procedural, Artwork::Image),
			custom_artwork,
			music_playing: true,
			activity: ActivityStore::new(),
			lease: None,
			transfer: TaskSlot::new(),
			_clock: clock,
		}
	}

	fn end_activity(&mut self) {
		if let Some(lease) = self.lease.take() {
			self.activity.end(lease);
		}
	}

	fn primary_action(&mut self, cx: &mut Context<Self>) {
		match self.phase {
			Phase::NotInstalled | Phase::Paused => self.start_transfer(cx),
			Phase::Downloading => {
				// Invalidate first, so suspended work cannot republish stale progress.
				self.transfer.invalidate();
				self.end_activity();
				self.phase = Phase::Paused;
			}
			Phase::Verifying => {}
			Phase::Ready => {
				self.lease = self.activity.begin(Activity::Running);
				if self.lease.is_some() {
					self.phase = Phase::Running;
				}
			}
			Phase::Running => {
				self.end_activity();
				self.phase = Phase::Ready;
				self.halo_generation += 1;
			}
		}
		cx.notify();
	}

	/// Simulates a download whose raw progress is throttled to ~10 Hz, then a verification.
	fn start_transfer(&mut self, cx: &mut Context<Self>) {
		let Some(lease) = self.activity.begin(Activity::Downloading) else {
			return;
		};
		self.lease = Some(lease);
		self.phase = Phase::Downloading;
		let start = self.progress;
		self.transfer.run(
			cx,
			|home| &mut home.transfer,
			async move |job| {
				// The work also stops when its activity lease is no longer current.
				job.guard(move |home: &Home| home.activity.holds(lease));
				let mut throttle = Throttle::progress();
				let started_at = std::time::Instant::now();
				let rate = DOWNLOAD_RATE * DEMO_SPEEDUP / DOWNLOAD_BYTES;
				loop {
					job.sleep(Duration::from_millis(25)).await;
					let progress = (start + started_at.elapsed().as_secs_f64() * rate).min(1.);
					if progress >= 1. {
						break;
					}
					job.update_throttled(&mut throttle, |home, cx| {
						home.progress = progress;
						cx.notify();
					})?;
				}
				job.update(|home, cx| {
					home.progress = 1.;
					home.phase = Phase::Verifying;
					home.activity.update(lease, Activity::Verifying);
					cx.notify();
				})?;
				job.sleep(Duration::from_millis(2600)).await;
				job.update(|home, cx| {
					home.end_activity();
					home.phase = Phase::Ready;
					home.halo_generation += 1;
					cx.notify();
				})
			},
		);
	}

	fn toggle_pill(&mut self, pill: Pill, cx: &mut Context<Self>) {
		self.expanded = if self.expanded == Some(pill) {
			None
		} else {
			Some(pill)
		};
		cx.notify();
	}

	fn status(&self) -> impl IntoElement + use<> {
		let region = REGIONS[self.region].0;
		let percent = (self.progress * 100.).floor() as u32;
		match self.phase {
			Phase::NotInstalled => {
				status("Not installed").detail(format!("{region} · {} download", gigabytes(DOWNLOAD_BYTES)))
			}
			Phase::Downloading => {
				let done = self.progress * DOWNLOAD_BYTES;
				let remaining = ((DOWNLOAD_BYTES - done) / (DOWNLOAD_RATE * DEMO_SPEEDUP)).ceil() as u64;
				status(format!("{percent}%")).emphasized(true).detail(format!(
					"{} of {} · {:.1} MB/s · {remaining} s left",
					gigabytes(done),
					gigabytes(DOWNLOAD_BYTES),
					DOWNLOAD_RATE / 1e6
				))
			}
			Phase::Paused => status(format!("Paused · {percent}%"))
				.emphasized(true)
				.detail("Resume continues from the partial files"),
			Phase::Verifying => status("Verifying files").detail("Checking 1 284 files"),
			Phase::Ready => status("Ready to play").detail(format!("{region} · Version 2.6.81")),
			Phase::Running => {
				status("Arknights is running").detail("Stop closes the game and its Windows runtime")
			}
		}
	}

	fn primary_button(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
		let action = cx.listener(|this, _, _, cx| this.primary_action(cx));
		let base = |title: &'static str, icon: Icon| button("primary-action", title).icon(icon).large();
		match self.phase {
			Phase::NotInstalled => base("Install", Icon::ArrowDown)
				.accent()
				.on_click(action)
				.into_any_element(),
			Phase::Downloading => base("Pause", Icon::Pause).on_click(action).into_any_element(),
			Phase::Paused => base("Resume", Icon::ArrowClockwise)
				.accent()
				.on_click(action)
				.into_any_element(),
			Phase::Verifying => base("Pause", Icon::Pause).disabled(true).into_any_element(),
			Phase::Ready => halo(base("Play", Icon::Play).accent().on_click(action))
				.generation(self.halo_generation)
				.into_any_element(),
			Phase::Running => base("Stop", Icon::Stop).on_click(action).into_any_element(),
		}
	}

	fn outline(&self) -> (Option<OutlineMode>, bool) {
		match self.phase {
			Phase::Downloading => (Some(OutlineMode::Determinate(self.progress as f32)), true),
			Phase::Paused => (Some(OutlineMode::Determinate(self.progress as f32)), false),
			Phase::Verifying => (Some(OutlineMode::Indeterminate), true),
			_ => (None, false),
		}
	}

	fn pill_row(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
		let theme = cx.theme();
		let receded = |pill: Pill| self.expanded.is_some() && self.expanded != Some(pill);
		let now = SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.unwrap_or_default()
			.as_secs();
		let playing = self.music_playing;

		let music = pill("music-pill", Icon::Music, || {
			div()
				.max_w(px(180.))
				.truncate()
				.font_weight(FontWeight::MEDIUM)
				.child("Sanity Restored — Lobby Theme")
		})
		.noun("music")
		.detail(
			div()
				.flex()
				.flex_col()
				.gap(px(8.))
				.child(
					div()
						.text_color(theme.palette.secondary_text)
						.child("Playing from YouTube · fades out when the game starts"),
				)
				.child(
					div()
						.flex()
						.flex_row()
						.gap(px(6.))
						.child(
							button("music-play", if playing { "Pause" } else { "Play" })
								.icon(if playing { Icon::Pause } else { Icon::Play })
								.compact()
								.on_click(cx.listener(|this, _, _, cx| {
									this.music_playing = !this.music_playing;
									cx.notify();
								})),
						)
						.child(button("music-next", "Next").icon(Icon::Forward).compact()),
				),
			hud::EXPANDED_MUSIC_WIDTH,
			px(116.),
		)
		.expanded(self.expanded == Some(Pill::Music))
		.receded(receded(Pill::Music))
		.on_toggle(cx.listener(|this, _, _, cx| this.toggle_pill(Pill::Music, cx)));

		let version = pill("version-pill", Icon::Tag, || {
			div().font_weight(FontWeight::MEDIUM).child("2.6.81")
		})
		.noun("version")
		.detail(
			div()
				.flex()
				.flex_col()
				.gap(px(8.))
				.child(
					div()
						.text_color(theme.palette.secondary_text)
						.child("Installed 2.6.81 · Latest 2.6.81"),
				)
				.child(
					button("check-updates", "Check for Updates")
						.icon(Icon::ArrowClockwise)
						.compact(),
				),
			hud::EXPANDED_VERSION_WIDTH,
			px(112.),
		)
		.expanded(self.expanded == Some(Pill::Version))
		.receded(receded(Pill::Version))
		.on_toggle(cx.listener(|this, _, _, cx| this.toggle_pill(Pill::Version, cx)));

		let (region_name, reset_hour) = REGIONS[self.region];
		let rows = REGIONS.iter().enumerate().map(|(index, (name, hour))| {
			list_row(("region", index), *name)
				.value(countdown(now, *hour))
				.selected(index == self.region)
				.on_click(cx.listener(move |this, _, _, cx| {
					this.region = index;
					cx.notify();
				}))
		});
		let status_pill = pill("status-pill", Icon::Globe, move || {
			div()
				.flex()
				.flex_row()
				.gap(px(6.))
				.child(div().font_weight(FontWeight::SEMIBOLD).child(region_name))
				.child(
					div()
						.text_color(theme.palette.secondary_text)
						.child(countdown(now, reset_hour)),
				)
		})
		.noun("server time")
		.detail(
			div().flex().flex_col().gap(px(2.)).children(rows),
			hud::EXPANDED_STATUS_WIDTH,
			px(28. + 8. + 16. + 5. * 28.),
		)
		.expanded(self.expanded == Some(Pill::Status))
		.receded(receded(Pill::Status))
		.on_toggle(cx.listener(|this, _, _, cx| this.toggle_pill(Pill::Status, cx)));

		div()
			.flex()
			.flex_row()
			.justify_end()
			.items_end()
			.gap(px(8.))
			.pr(px(2.))
			// The pills enter one after another, like the launcher's HUD.
			.child(div().child(music).enter("enter-music", 0, theme.reduce_motion))
			.child(
				div()
					.child(version)
					.enter("enter-version", 1, theme.reduce_motion),
			)
			.child(
				div()
					.child(status_pill)
					.enter("enter-status", 2, theme.reduce_motion),
			)
	}

	fn legend(&self, theme: &Theme) -> impl IntoElement + use<> {
		let accent = tokens::contrast_tint(theme.accent);
		let row = move |key: &'static str, label: SharedString| {
			div()
				.flex()
				.flex_row()
				.gap(px(8.))
				.child(
					div()
						.w(px(46.))
						.font_weight(FontWeight::SEMIBOLD)
						.text_color(accent)
						.child(key),
				)
				.child(div().text_color(theme.palette.primary_text).child(label))
		};
		glass_panel()
			.theme(*theme)
			.absolute()
			.top(px(64.))
			.right(px(20.))
			.w(px(270.))
			.flex()
			.flex_col()
			.gap(px(4.))
			.text_size(px(11.))
			.child(
				div()
					.pb(px(4.))
					.font_weight(FontWeight::SEMIBOLD)
					.child("Showcase controls"),
			)
			.child(row("Space", "Primary action".into()))
			.child(row("A", "Next accent".into()))
			.child(row("G", format!("Glass: {}", theme.style.glass.label()).into()))
			.child(row(
				"M",
				format!(
					"Reduce Motion ({})",
					if theme.reduce_motion { "on" } else { "off" }
				)
				.into(),
			))
			.child(row("1 2 3", "Music, version, status pill".into()))
			.child(row("U", "Show the neutral update button".into()))
			.when(self.custom_artwork.is_some(), |this| {
				this.child(row("O", "Own artwork or procedural".into()))
			})
			.child(row("H", "Hide this panel".into()))
	}
}

impl Render for Home {
	fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
		let theme = cx.theme();
		let (outline, outline_active) = self.outline();
		let bar = control_bar(self.status())
			.progress(outline, outline_active)
			.when(self.shows_update_button, |bar| {
				bar.action(button("install-update", "Install Update").icon(Icon::ArrowDown))
			})
			.action(self.primary_button(cx));

		// The wordmark sits under the traffic lights, as in the SwiftUI launcher.
		let brand = div()
			.absolute()
			.top(px(42.))
			.left(px(20.))
			.child(wordmark(tokens::text::TITLE));
		let title_bar = title_bar("home-title-bar").child(
			icon_button("settings", Icon::Gear, "Settings").on_click(cx.listener(|this, _, _, cx| {
				this.shows_legend = !this.shows_legend;
				cx.notify();
			})),
		);

		let hud = div()
			.absolute()
			.bottom_0()
			.left_0()
			.right_0()
			.p(px(20.))
			.flex()
			.flex_col()
			.gap(px(10.))
			.child(self.pill_row(cx))
			.child(bar);

		let root = div()
			.size_full()
			.relative()
			.overflow_hidden()
			.font_family(UI_FONT)
			.text_color(theme.palette.primary_text)
			.track_focus(&self.focus)
			.key_context("Home")
			.on_action(cx.listener(|this, _: &PrimaryAction, _, cx| this.primary_action(cx)))
			.on_action(cx.listener(|this, _: &CycleAccent, _, cx| {
				this.accent_index = (this.accent_index + 1) % accents().len();
				this.halo_generation += 1;
				let accent = accents()[this.accent_index];
				Theme::update(cx, |theme| *theme = theme.with_accent(accent));
			}))
			.on_action(cx.listener(|_, _: &CycleGlass, _, cx| {
				Theme::update(cx, |theme| theme.style.glass = theme.style.glass.next())
			}))
			.on_action(cx.listener(|_, _: &ToggleReduceMotion, _, cx| {
				Theme::update(cx, |theme| theme.reduce_motion = !theme.reduce_motion)
			}))
			.on_action(cx.listener(|this, _: &ToggleUpdateButton, _, cx| {
				this.shows_update_button = !this.shows_update_button;
				cx.notify();
			}))
			.on_action(cx.listener(|this, _: &ToggleMusicPill, _, cx| this.toggle_pill(Pill::Music, cx)))
			.on_action(cx.listener(|this, _: &ToggleVersionPill, _, cx| this.toggle_pill(Pill::Version, cx)))
			.on_action(cx.listener(|this, _: &ToggleStatusPill, _, cx| this.toggle_pill(Pill::Status, cx)))
			.on_action(cx.listener(|this, _: &ToggleArtwork, _, cx| {
				if let Some(path) = &this.custom_artwork {
					this.artwork = match this.artwork {
						Artwork::Procedural => Artwork::Image(path.clone()),
						Artwork::Image(_) => Artwork::Procedural,
					};
					this.apply_artwork_theme(cx);
					cx.notify();
				}
			}))
			.on_action(cx.listener(|this, _: &ToggleLegend, _, cx| {
				this.shows_legend = !this.shows_legend;
				cx.notify();
			}))
			.child(
				cover_image(match &self.artwork {
					Artwork::Procedural => None,
					Artwork::Image(path) => Some(path.clone()),
				})
				.fallback(procedural(
					theme.accent,
					theme.palette.backdrop,
					chrome::corner_radius(window),
				))
				.corner(chrome::corner_radius(window)),
			)
			.child(readability_field(theme.reduce_transparency()))
			// Like the SwiftUI HUD, a click outside an expanded pill collapses it.
			.when(self.expanded.is_some(), |this| {
				this.child(div().absolute().size_full().on_mouse_down(
					MouseButton::Left,
					cx.listener(|this, _, _, cx| {
						this.expanded = None;
						cx.notify();
					}),
				))
			})
			.child(brand)
			.child(hud)
			.when(self.shows_legend, |this| this.child(self.legend(&theme)))
			.child(title_bar);
		window_frame(root)
	}
}

/// The artwork's tint from a small thumbnail, or `None` when it has no hue or fails to load.
fn artwork_tint(path: &std::path::Path) -> Option<calcite::tint::Tint> {
	let image = match image::open(path) {
		Ok(image) => image.thumbnail(64, 64).to_rgba8(),
		Err(error) => {
			eprintln!("artwork tint: {}: {error}", path.display());
			return None;
		}
	};
	let (width, height) = image.dimensions();
	let view = calcite::tint::ImageView::packed_rgba(width as usize, height as usize, image.as_raw())
		.inspect_err(|error| eprintln!("artwork tint: {}: {error}", path.display()))
		.ok()?;
	calcite::tint::Extractor::default().extract(view)
}

fn gigabytes(bytes: f64) -> String {
	format!("{:.2} GB", bytes / 1e9)
}

/// Time until the next daily reset at `reset_hour` UTC, as `HH:MM:SS`.
fn countdown(now: u64, reset_hour: u64) -> String {
	let day = 86_400;
	let remaining = (reset_hour * 3_600 + day - now % day) % day;
	format!(
		"{:02}:{:02}:{:02}",
		remaining / 3_600,
		remaining % 3_600 / 60,
		remaining % 60
	)
}

/// The Dock, task bar, and window icon outside a bundle.
static ICON: AppIcon = calcite::app_icon!("assets/icon.png");

fn main() {
	let custom_artwork = std::env::args_os()
		.nth(1)
		.or_else(|| std::env::var_os("AK_ARTWORK"))
		.map(PathBuf::from);

	gpui_platform::application()
		.with_assets(Assets::default().with(SLASH_ASSET, SLASH_SVG))
		.run(move |cx: &mut App| {
			calcite::init(cx);
			ICON.install(cx);
			keymap!(cx, {
				"Home" => {
					"space" => PrimaryAction,
					"a" => CycleAccent,
					"g" => CycleGlass,
					"m" => ToggleReduceMotion,
					"u" => ToggleUpdateButton,
					"1" => ToggleMusicPill,
					"2" => ToggleVersionPill,
					"3" => ToggleStatusPill,
					"o" => ToggleArtwork,
					"h" => ToggleLegend,
				},
				_ => { "cmd-q" => Quit, "ctrl-q" => Quit },
			});
			cx.on_action(|_: &Quit, cx| cx.quit());
			cx.on_window_closed(|cx, _| {
				if cx.windows().is_empty() {
					cx.quit();
				}
			})
			.detach();

			let bounds = Bounds::centered(None, size(px(1040.), px(680.)), cx);
			cx.open_window(
				ICON.window_options(WindowOptions {
					window_min_size: Some(size(px(880.), px(560.))),
					..chrome::window_options("Arknights Client", bounds)
				}),
				move |window, cx| {
					cx.new(|cx| {
						let mut home = Home::new(custom_artwork, window, cx);
						home.apply_artwork_theme(cx);
						home
					})
				},
			)
			.expect("failed to open the home window");
			cx.activate(true);
		});
}
