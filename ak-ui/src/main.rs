// SPDX-License-Identifier: MPL-2.0

//! Showcase of the UI kit: the launcher home screen with a simulated install, download,
//! verification, and game session. It contains no launcher logic and starts no game.
//!
//! Run: `cargo run --release` (optionally `-- /path/to/artwork.jpg`).

use std::{
	path::PathBuf,
	time::{Duration, SystemTime, UNIX_EPOCH},
};

use ak_ui::{
	Appearance, UI_FONT,
	artwork::{Artwork, artwork_layer, readability_field, wordmark},
	controls::{CapsuleButton, IconButton, Size, Tone, accent_halo},
	glass::{Glass, GlassExt},
	hud::{ControlBar, HudPill, pill_row, status_text},
	icons::{Assets, Icon},
	progress::OutlineMode,
	tokens::{self, hud, rgba, scale_alpha},
};
use gpui::{
	App, Bounds, Context, FocusHandle, FontWeight, IntoElement, KeyBinding, MouseButton, SharedString, Task,
	TitlebarOptions, Window, WindowBounds, WindowOptions, actions, div, prelude::*, px, size,
};

actions!(
	showcase,
	[
		PrimaryAction,
		CycleAccent,
		ToggleReduceTransparency,
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
const TICK: Duration = Duration::from_millis(100);
/// The simulated transfer runs faster than the displayed rate so the demo stays short.
const DEMO_SPEEDUP: f64 = 9.;

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
const ACCENTS: [Option<gpui::Rgba>; 5] = [
	None,
	Some(rgba(0.96, 0.62, 0.22, 1.)),
	Some(rgba(0.94, 0.36, 0.52, 1.)),
	Some(rgba(0.62, 0.50, 0.98, 1.)),
	Some(rgba(0.36, 0.86, 0.58, 1.)),
];

struct Showcase {
	focus: FocusHandle,
	phase: Phase,
	progress: f64,
	accent_index: usize,
	reduce_transparency: bool,
	reduce_motion: bool,
	shows_update_button: bool,
	expanded: Option<Pill>,
	region: usize,
	halo_generation: usize,
	shows_legend: bool,
	artwork: Artwork,
	custom_artwork: Option<PathBuf>,
	music_playing: bool,
	transfer: Option<Task<()>>,
	_clock: Task<()>,
}

impl Showcase {
	fn new(custom_artwork: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) -> Self {
		let focus = cx.focus_handle();
		window.focus(&focus, cx);
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
			reduce_transparency: false,
			reduce_motion: cx.reduce_motion(),
			shows_update_button: false,
			expanded: None,
			region: 0,
			halo_generation: 0,
			shows_legend: true,
			artwork: custom_artwork.clone().map_or(Artwork::Procedural, Artwork::Image),
			custom_artwork,
			music_playing: true,
			transfer: None,
			_clock: clock,
		}
	}

	fn appearance(&self) -> Appearance {
		Appearance {
			reduce_transparency: self.reduce_transparency,
			reduce_motion: self.reduce_motion,
			..Appearance::new(ACCENTS[self.accent_index])
		}
	}

	fn primary_action(&mut self, cx: &mut Context<Self>) {
		match self.phase {
			Phase::NotInstalled | Phase::Paused => self.start_transfer(cx),
			Phase::Downloading => {
				self.transfer = None;
				self.phase = Phase::Paused;
			}
			Phase::Verifying => {}
			Phase::Ready => self.phase = Phase::Running,
			Phase::Running => self.enter_ready(),
		}
		cx.notify();
	}

	fn enter_ready(&mut self) {
		self.phase = Phase::Ready;
		self.halo_generation += 1;
	}

	/// Simulates a download at 10 Hz, then a short verification.
	fn start_transfer(&mut self, cx: &mut Context<Self>) {
		self.phase = Phase::Downloading;
		self.transfer = Some(cx.spawn(async move |this, cx| {
			loop {
				cx.background_executor().timer(TICK).await;
				let finished = this.update(cx, |this, cx| {
					this.progress = (this.progress
						+ DOWNLOAD_RATE * TICK.as_secs_f64() * DEMO_SPEEDUP / DOWNLOAD_BYTES)
						.min(1.);
					cx.notify();
					this.progress >= 1.
				});
				match finished {
					Ok(true) => break,
					Ok(false) => continue,
					Err(_) => return,
				}
			}
			let _ = this.update(cx, |this, cx| {
				this.phase = Phase::Verifying;
				cx.notify();
			});
			cx.background_executor().timer(Duration::from_millis(2600)).await;
			let _ = this.update(cx, |this, cx| {
				this.transfer = None;
				this.enter_ready();
				cx.notify();
			});
		}));
	}

	fn toggle_pill(&mut self, pill: Pill, cx: &mut Context<Self>) {
		self.expanded = if self.expanded == Some(pill) {
			None
		} else {
			Some(pill)
		};
		cx.notify();
	}

	fn status(&self) -> (SharedString, Option<SharedString>, bool) {
		let region = REGIONS[self.region].0;
		let percent = (self.progress * 100.).floor() as u32;
		match self.phase {
			Phase::NotInstalled => (
				"Not installed".into(),
				Some(format!("{region} · {} download", gigabytes(DOWNLOAD_BYTES)).into()),
				false,
			),
			Phase::Downloading => {
				let done = self.progress * DOWNLOAD_BYTES;
				let remaining = ((DOWNLOAD_BYTES - done) / (DOWNLOAD_RATE * DEMO_SPEEDUP)).ceil() as u64;
				(
					format!("{percent}%").into(),
					Some(
						format!(
							"{} of {} · {:.1} MB/s · {} s left",
							gigabytes(done),
							gigabytes(DOWNLOAD_BYTES),
							DOWNLOAD_RATE / 1e6,
							remaining
						)
						.into(),
					),
					true,
				)
			}
			Phase::Paused => (
				format!("Paused · {percent}%").into(),
				Some("Resume continues from the partial files".into()),
				true,
			),
			Phase::Verifying => (
				"Verifying files".into(),
				Some("Checking 1 284 files".into()),
				false,
			),
			Phase::Ready => (
				"Ready to play".into(),
				Some(format!("{region} · Version 2.6.81").into()),
				false,
			),
			Phase::Running => (
				"Arknights is running".into(),
				Some("Stop closes the game and its Windows runtime".into()),
				false,
			),
		}
	}

	fn primary_button(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
		let appearance = self.appearance();
		let accent = Tone::Accent(appearance.accent);
		let (title, icon, tone, enabled) = match self.phase {
			Phase::NotInstalled => ("Install", Icon::ArrowDown, accent, true),
			Phase::Downloading => ("Pause", Icon::Pause, Tone::Neutral, true),
			Phase::Paused => ("Resume", Icon::ArrowClockwise, accent, true),
			Phase::Verifying => ("Pause", Icon::Pause, Tone::Neutral, false),
			Phase::Ready => ("Play", Icon::Play, accent, true),
			Phase::Running => ("Stop", Icon::Stop, Tone::Neutral, true),
		};
		let button = CapsuleButton::new("primary-action", title, tone, appearance)
			.icon(icon)
			.size(Size::Large)
			.enabled(enabled)
			.on_click(cx.listener(|this, _, _, cx| this.primary_action(cx)));
		if self.phase == Phase::Ready {
			accent_halo(appearance.accent, self.halo_generation, &appearance, button).into_any_element()
		} else {
			button.into_any_element()
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
		let appearance = self.appearance();
		let receded = |pill: Pill| self.expanded.is_some() && self.expanded != Some(pill);
		let now = SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.unwrap_or_default()
			.as_secs();

		let music = HudPill::new(
			"music-pill",
			Icon::Music,
			div()
				.max_w(px(180.))
				.truncate()
				.font_weight(FontWeight::MEDIUM)
				.child("Sanity Restored — Lobby Theme"),
			appearance,
		)
		.detail(
			div()
				.flex()
				.flex_col()
				.gap(px(8.))
				.child(
					div()
						.text_color(tokens::SECONDARY_TEXT)
						.child("Playing from YouTube · fades out when the game starts"),
				)
				.child(
					div()
						.flex()
						.flex_row()
						.gap(px(6.))
						.child(
							CapsuleButton::new(
								"music-play",
								if self.music_playing { "Pause" } else { "Play" },
								Tone::Neutral,
								appearance,
							)
							.icon(if self.music_playing {
								Icon::Pause
							} else {
								Icon::Play
							})
							.size(Size::Compact)
							.on_click(cx.listener(|this, _, _, cx| {
								this.music_playing = !this.music_playing;
								cx.notify();
							})),
						)
						.child(
							CapsuleButton::new("music-next", "Next", Tone::Neutral, appearance)
								.icon(Icon::Forward)
								.size(Size::Compact),
						),
				),
			hud::EXPANDED_MUSIC_WIDTH,
			px(116.),
		)
		.expanded(self.expanded == Some(Pill::Music))
		.receded(receded(Pill::Music))
		.on_toggle(cx.listener(|this, _, _, cx| this.toggle_pill(Pill::Music, cx)));

		let version = HudPill::new(
			"version-pill",
			Icon::Tag,
			div().font_weight(FontWeight::MEDIUM).child("2.6.81"),
			appearance,
		)
		.detail(
			div()
				.flex()
				.flex_col()
				.gap(px(8.))
				.child(
					div()
						.text_color(tokens::SECONDARY_TEXT)
						.child("Installed 2.6.81 · Latest 2.6.81"),
				)
				.child(
					CapsuleButton::new("check-updates", "Check for Updates", Tone::Neutral, appearance)
						.icon(Icon::ArrowClockwise)
						.size(Size::Compact),
				),
			hud::EXPANDED_VERSION_WIDTH,
			px(112.),
		)
		.expanded(self.expanded == Some(Pill::Version))
		.receded(receded(Pill::Version))
		.on_toggle(cx.listener(|this, _, _, cx| this.toggle_pill(Pill::Version, cx)));

		let (region_name, reset_hour) = REGIONS[self.region];
		let status = HudPill::new(
			"status-pill",
			Icon::Globe,
			div()
				.flex()
				.flex_row()
				.gap(px(6.))
				.child(div().font_weight(FontWeight::SEMIBOLD).child(region_name))
				.child(
					div()
						.text_color(tokens::SECONDARY_TEXT)
						.child(countdown(now, reset_hour)),
				),
			appearance,
		)
		.detail(
			div()
				.flex()
				.flex_col()
				.gap(px(2.))
				.children(REGIONS.iter().enumerate().map(|(index, (name, hour))| {
					pill_row(
						("region", index),
						*name,
						countdown(now, *hour),
						index == self.region,
						appearance,
						cx.listener(move |this, _, _, cx| {
							this.region = index;
							cx.notify();
						}),
					)
				})),
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
			.child(music)
			.child(version)
			.child(status)
	}

	fn legend(&self) -> impl IntoElement + use<> {
		let appearance = self.appearance();
		let row = |key: &'static str, label: SharedString| {
			div()
				.flex()
				.flex_row()
				.gap(px(8.))
				.child(
					div()
						.w(px(46.))
						.font_weight(FontWeight::SEMIBOLD)
						.text_color(tokens::contrast_tint(appearance.accent))
						.child(key),
				)
				.child(div().text_color(tokens::PRIMARY_TEXT).child(label))
		};
		let state = |on: bool| if on { "on" } else { "off" };
		div()
			.absolute()
			.top(px(64.))
			.right(px(20.))
			.w(px(260.))
			.p(px(14.))
			.flex()
			.flex_col()
			.gap(px(4.))
			.text_size(px(11.))
			.rounded(tokens::radius::PANEL)
			.rounded_smoothing_ios()
			.glass(Glass::new(Some(appearance.hud_tint), appearance.treatment()))
			.border_1()
			.border_color(scale_alpha(tokens::HAIRLINE, tokens::control::HUD_BORDER_OPACITY))
			.child(
				div()
					.pb(px(4.))
					.font_weight(FontWeight::SEMIBOLD)
					.text_color(tokens::PRIMARY_TEXT)
					.child("Showcase controls"),
			)
			.child(row("Space", "Primary action".into()))
			.child(row("A", "Next accent".into()))
			.child(row("1 2 3", "Music, version, status pill".into()))
			.child(row("U", "Show the neutral update button".into()))
			.child(row(
				"T",
				format!("Reduce Transparency ({})", state(self.reduce_transparency)).into(),
			))
			.child(row(
				"M",
				format!("Reduce Motion ({})", state(self.reduce_motion)).into(),
			))
			.when(self.custom_artwork.is_some(), |this| {
				this.child(row("O", "Own artwork or procedural".into()))
			})
			.child(row("H", "Hide this panel".into()))
	}
}

impl Render for Showcase {
	fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
		let appearance = self.appearance();
		let (title, detail, emphasized) = self.status();
		let (outline, outline_active) = self.outline();

		let mut bar = ControlBar::new(status_text(title, detail, emphasized), appearance)
			.progress(outline, outline_active);
		if self.shows_update_button {
			bar = bar.action(
				CapsuleButton::new("install-update", "Install Update", Tone::Neutral, appearance)
					.icon(Icon::ArrowDown),
			);
		}
		bar = bar.action(self.primary_button(cx));

		let top_bar = div()
			.absolute()
			.top_0()
			.left_0()
			.right_0()
			.pt(px(8.))
			.px(px(14.))
			.flex()
			.flex_row()
			.items_start()
			.justify_between()
			.child(div().pt(px(34.)).pl(px(6.)).child(wordmark(tokens::text::TITLE)))
			.child(
				IconButton::hud("settings", Icon::Gear, appearance).on_click(cx.listener(
					|this, _, _, cx| {
						this.shows_legend = !this.shows_legend;
						cx.notify();
					},
				)),
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

		div()
			.size_full()
			.relative()
			.overflow_hidden()
			.bg(gpui::black())
			.font_family(UI_FONT)
			.text_color(tokens::PRIMARY_TEXT)
			.track_focus(&self.focus)
			.key_context("Showcase")
			.on_action(cx.listener(|this, _: &PrimaryAction, _, cx| this.primary_action(cx)))
			.on_action(cx.listener(|this, _: &CycleAccent, _, cx| {
				this.accent_index = (this.accent_index + 1) % ACCENTS.len();
				this.halo_generation += 1;
				cx.notify();
			}))
			.on_action(cx.listener(|this, _: &ToggleReduceTransparency, _, cx| {
				this.reduce_transparency = !this.reduce_transparency;
				cx.notify();
			}))
			.on_action(cx.listener(|this, _: &ToggleReduceMotion, _, cx| {
				this.reduce_motion = !this.reduce_motion;
				cx.notify();
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
					cx.notify();
				}
			}))
			.on_action(cx.listener(|this, _: &ToggleLegend, _, cx| {
				this.shows_legend = !this.shows_legend;
				cx.notify();
			}))
			.child(artwork_layer(&self.artwork, appearance.accent))
			.child(readability_field(self.reduce_transparency))
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
			.child(top_bar)
			.child(hud)
			.when(self.shows_legend, |this| this.child(self.legend()))
	}
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

fn main() {
	let custom_artwork = std::env::args_os()
		.nth(1)
		.or_else(|| std::env::var_os("AK_ARTWORK"))
		.map(PathBuf::from);

	gpui_platform::application()
		.with_assets(Assets)
		.run(move |cx: &mut App| {
			cx.bind_keys([
				KeyBinding::new("space", PrimaryAction, Some("Showcase")),
				KeyBinding::new("a", CycleAccent, Some("Showcase")),
				KeyBinding::new("t", ToggleReduceTransparency, Some("Showcase")),
				KeyBinding::new("m", ToggleReduceMotion, Some("Showcase")),
				KeyBinding::new("u", ToggleUpdateButton, Some("Showcase")),
				KeyBinding::new("1", ToggleMusicPill, Some("Showcase")),
				KeyBinding::new("2", ToggleVersionPill, Some("Showcase")),
				KeyBinding::new("3", ToggleStatusPill, Some("Showcase")),
				KeyBinding::new("o", ToggleArtwork, Some("Showcase")),
				KeyBinding::new("h", ToggleLegend, Some("Showcase")),
				KeyBinding::new("cmd-q", Quit, None),
				KeyBinding::new("ctrl-q", Quit, None),
			]);
			cx.on_action(|_: &Quit, cx| cx.quit());
			cx.on_window_closed(|cx, _| {
				if cx.windows().is_empty() {
					cx.quit();
				}
			})
			.detach();

			let bounds = Bounds::centered(None, size(px(1040.), px(680.)), cx);
			cx.open_window(
				WindowOptions {
					window_bounds: Some(WindowBounds::Windowed(bounds)),
					window_min_size: Some(size(px(880.), px(560.))),
					titlebar: Some(TitlebarOptions {
						title: Some("Arknights Client".into()),
						appears_transparent: true,
						traffic_light_position: None,
					}),
					..Default::default()
				},
				move |window, cx| cx.new(|cx| Showcase::new(custom_artwork, window, cx)),
			)
			.expect("failed to open the showcase window");
			cx.activate(true);
		});
}
