// SPDX-License-Identifier: MPL-2.0

//! Showcase of the UI kit: the launcher home screen with a simulated install, download,
//! verification, and game session. It contains no launcher logic and starts no game.
//!
//! Run: `cargo run --release` (optionally `-- /path/to/artwork.jpg`).

use std::{
	path::PathBuf,
	time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use gpui::{
	App, Bounds, Context, FocusHandle, FontWeight, IntoElement, KeyBinding, MouseButton, SharedString, Task,
	TitlebarOptions, Window, WindowBounds, WindowOptions, actions, div, prelude::*, px, size,
};
use vitro_exec::{ActivityLease, ActivityStore, TaskSlot, Throttle, publish};
use vitro_showcase::artwork::{Artwork, SLASH_ASSET, SLASH_SVG, artwork_layer, readability_field, wordmark};
use vitro_ui::{
	GlassStyle, Theme, UI_FONT,
	controls::{CapsuleButton, IconButton, Size, Tone, accent_halo},
	glass::GlassExt,
	hud::{ControlBar, HudPill, pill_row, status_text},
	icons::{Assets, Icon},
	progress::OutlineMode,
	tokens::{self, hud, rgba, scale_alpha},
};

actions!(
	showcase,
	[
		PrimaryAction,
		CycleAccent,
		CycleGlassStyle,
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
/// Raw progress arrives faster than the UI should redraw; `Throttle` bounds it to ~10 Hz.
const TICK: Duration = Duration::from_millis(25);
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
	glass: GlassStyle,
	reduce_motion: bool,
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
			glass: GlassStyle::MacOS27(vitro_ui::Transparency::Default),
			reduce_motion: cx.reduce_motion(),
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

	fn theme(&self) -> Theme {
		Theme {
			glass: self.glass,
			reduce_motion: self.reduce_motion,
			..Theme::new(ACCENTS[self.accent_index])
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
				self.enter_ready();
			}
		}
		cx.notify();
	}

	fn enter_ready(&mut self) {
		self.phase = Phase::Ready;
		self.halo_generation += 1;
	}

	/// Simulates a download whose raw progress is throttled to ~10 Hz, then a verification.
	fn start_transfer(&mut self, cx: &mut Context<Self>) {
		let Some(lease) = self.activity.begin(Activity::Downloading) else {
			return;
		};
		self.lease = Some(lease);
		self.phase = Phase::Downloading;
		let mut progress = self.progress;
		self.transfer.replace(cx, async move |token, this, cx| {
			let mut throttle = Throttle::progress();
			let (started_at, start) = (Instant::now(), progress);
			let rate = DOWNLOAD_RATE * DEMO_SPEEDUP / DOWNLOAD_BYTES;
			while progress < 1. {
				cx.background_executor().timer(TICK).await;
				let now = Instant::now();
				progress = (start + now.duration_since(started_at).as_secs_f64() * rate).min(1.);
				if progress < 1. && !throttle.admit(now) {
					continue;
				}
				let published = publish(
					&this,
					cx,
					|view| &view.transfer,
					token,
					|view, cx| {
						view.progress = progress;
						cx.notify();
					},
				);
				if !published {
					return;
				}
			}
			let verifying = publish(
				&this,
				cx,
				|view| &view.transfer,
				token,
				|view, cx| {
					view.progress = 1.;
					view.phase = Phase::Verifying;
					view.activity.update(lease, Activity::Verifying);
					cx.notify();
				},
			);
			if !verifying {
				return;
			}
			cx.background_executor().timer(Duration::from_millis(2600)).await;
			publish(
				&this,
				cx,
				|view| &view.transfer,
				token,
				|view, cx| {
					view.transfer.finish(token);
					view.end_activity();
					view.enter_ready();
					cx.notify();
				},
			);
		});
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
		let theme = self.theme();
		let accent = Tone::Accent(theme.accent);
		let (title, icon, tone, enabled) = match self.phase {
			Phase::NotInstalled => ("Install", Icon::ArrowDown, accent, true),
			Phase::Downloading => ("Pause", Icon::Pause, Tone::Neutral, true),
			Phase::Paused => ("Resume", Icon::ArrowClockwise, accent, true),
			Phase::Verifying => ("Pause", Icon::Pause, Tone::Neutral, false),
			Phase::Ready => ("Play", Icon::Play, accent, true),
			Phase::Running => ("Stop", Icon::Stop, Tone::Neutral, true),
		};
		let button = CapsuleButton::new("primary-action", title, tone, theme)
			.icon(icon)
			.size(Size::Large)
			.enabled(enabled)
			.on_click(cx.listener(|this, _, _, cx| this.primary_action(cx)));
		if self.phase == Phase::Ready {
			accent_halo(theme.accent, self.halo_generation, &theme, button).into_any_element()
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
		let theme = self.theme();
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
			theme,
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
								theme,
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
							CapsuleButton::new("music-next", "Next", Tone::Neutral, theme)
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
			theme,
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
					CapsuleButton::new("check-updates", "Check for Updates", Tone::Neutral, theme)
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
			theme,
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
						theme,
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
		let theme = self.theme();
		let row = |key: &'static str, label: SharedString| {
			div()
				.flex()
				.flex_row()
				.gap(px(8.))
				.child(
					div()
						.w(px(46.))
						.font_weight(FontWeight::SEMIBOLD)
						.text_color(tokens::contrast_tint(theme.accent))
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
			.glass(theme.surface(Some(theme.hud_tint)))
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
			.child(row("G", format!("Glass: {}", self.glass.label()).into()))
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
		let theme = self.theme();
		let (title, detail, emphasized) = self.status();
		let (outline, outline_active) = self.outline();

		let mut bar =
			ControlBar::new(status_text(title, detail, emphasized), theme).progress(outline, outline_active);
		if self.shows_update_button {
			bar = bar.action(
				CapsuleButton::new("install-update", "Install Update", Tone::Neutral, theme)
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
				IconButton::hud("settings", Icon::Gear, theme).on_click(cx.listener(|this, _, _, cx| {
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
			.on_action(cx.listener(|this, _: &CycleGlassStyle, _, cx| {
				this.glass = this.glass.next();
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
			.child(artwork_layer(&self.artwork, theme.accent))
			.child(readability_field(self.glass == GlassStyle::ReducedTransparency))
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
		.with_assets(Assets::default().with(SLASH_ASSET, SLASH_SVG))
		.run(move |cx: &mut App| {
			cx.bind_keys([
				KeyBinding::new("space", PrimaryAction, Some("Showcase")),
				KeyBinding::new("a", CycleAccent, Some("Showcase")),
				KeyBinding::new("g", CycleGlassStyle, Some("Showcase")),
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
