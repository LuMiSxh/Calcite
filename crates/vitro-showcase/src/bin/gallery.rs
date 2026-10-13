// SPDX-License-Identifier: MPL-2.0

//! Component gallery: every Vitro component and glass style once, on one page.
//!
//! Keys: G next glass style for the component sections, A next accent, M Reduce Motion.

use gpui::{
	AnyElement, App, Bounds, Context, FocusHandle, FontWeight, IntoElement, KeyBinding, PathBuilder, Pixels,
	Rgba, SharedString, Styled, TitlebarOptions, Window, WindowBounds, WindowOptions, actions, canvas, div,
	linear_color_stop, linear_gradient, point, prelude::*, px, size,
};
use vitro_showcase::artwork::{SLASH_ASSET, SLASH_SVG};
use vitro_ui::{
	GlassStyle, Theme, Transparency, UI_FONT,
	controls::{CapsuleButton, IconButton, Size, Tone, accent_halo},
	glass::{Elevation, GlassExt},
	hud::{ControlBar, HudPill, pill_row, status_text},
	icons::{Assets, Icon},
	motion::{Curve, swiftui_spring},
	progress::OutlineMode,
	tokens::{self, hud, radius, rgba, scale_alpha, white},
};

actions!(gallery, [CycleGlass, CycleAccent, ToggleReduceMotion, Quit]);

const WIDTH: f32 = 1480.;
const HEIGHT: f32 = 1640.;
const ACCENTS: [Option<Rgba>; 4] = [
	None,
	Some(rgba(0.96, 0.62, 0.22, 1.)),
	Some(rgba(0.94, 0.36, 0.52, 1.)),
	Some(rgba(0.62, 0.50, 0.98, 1.)),
];
/// Fixed position of the pointer-light demo, so the gallery can show the light without a
/// real cursor.
const POINTER_DEMO_ORIGIN: (f32, f32) = (700., 592.);

struct Gallery {
	focus: FocusHandle,
	glass: GlassStyle,
	accent_index: usize,
	reduce_motion: bool,
}

impl Gallery {
	fn theme(&self) -> Theme {
		Theme {
			glass: self.glass,
			reduce_motion: self.reduce_motion,
			..Theme::new(ACCENTS[self.accent_index])
		}
	}
}

/// A bright, detailed backdrop over the whole page, so every glass surface has something
/// to refract.
fn backdrop(accent: Rgba) -> impl IntoElement {
	let blob = |left: f32, top: f32, size: f32, color: Rgba, blur: f32| {
		div()
			.absolute()
			.left(px(left))
			.top(px(top))
			.size(px(size))
			.rounded_full()
			.bg(color)
			.filter(vec![gpui::Filter::Blur(px(blur))])
	};
	div()
		.absolute()
		.size_full()
		.overflow_hidden()
		.bg(linear_gradient(
			170.,
			linear_color_stop(rgba(0.07, 0.10, 0.16, 1.), 0.),
			linear_color_stop(tokens::ARTWORK_FALLBACK, 1.),
		))
		.child(blob(900., -160., 700., scale_alpha(accent, 0.6), 130.))
		.child(blob(-200., 380., 640., rgba(0.96, 0.52, 0.18, 0.45), 120.))
		.child(blob(500., 820., 560., rgba(0.82, 0.22, 0.52, 0.42), 110.))
		.child(blob(1000., 1240., 520., rgba(0.30, 0.86, 0.62, 0.34), 100.))
		.child(blob(100., 1500., 520., scale_alpha(accent, 0.45), 110.))
		.children((0..(WIDTH as i32 / 46)).map(|index| {
			div()
				.absolute()
				.left(px(18. + index as f32 * 46.))
				.top_0()
				.w(px(if index % 4 == 0 { 6. } else { 2. }))
				.h_full()
				.bg(white(if index % 4 == 0 { 0.22 } else { 0.10 }))
		}))
		.children((0..(HEIGHT as i32 / 90)).map(|index| {
			div()
				.absolute()
				.top(px(40. + index as f32 * 90.))
				.left_0()
				.w_full()
				.h(px(1.))
				.bg(white(0.16))
		}))
		.child(
			div()
				.absolute()
				.left(px(600.))
				.top(px(-60.))
				.text_size(px(520.))
				.font_weight(FontWeight::BLACK)
				.text_color(white(0.06))
				.child("A"),
		)
		.child(
			div().absolute().left(px(560.)).top(px(40.)).child(
				gpui::svg()
					.path(SLASH_ASSET)
					.size(px(700.))
					.text_color(scale_alpha(accent, 0.45)),
			),
		)
}

fn section(
	title: &'static str,
	note: impl Into<SharedString>,
	content: impl IntoElement,
) -> impl IntoElement {
	div()
		.flex()
		.flex_col()
		.gap(px(10.))
		.child(
			div()
				.flex()
				.flex_row()
				.items_end()
				.gap(px(10.))
				.child(
					div()
						.text_size(px(15.))
						.font_weight(FontWeight::BOLD)
						.child(title),
				)
				.child(
					div()
						.text_size(px(11.))
						.text_color(tokens::SECONDARY_TEXT)
						.child(note.into()),
				),
		)
		.child(content)
}

fn label(text: impl Into<SharedString>) -> impl IntoElement {
	div()
		.text_size(px(10.))
		.text_color(tokens::SECONDARY_TEXT)
		.child(text.into())
}

/// One card per glass style, each with a HUD pill and an accent action in that style.
fn glass_styles(theme: Theme) -> impl IntoElement {
	div()
		.flex()
		.flex_row()
		.gap(px(16.))
		.children(GlassStyle::ALL.into_iter().map(move |style| {
			let styled = theme.with_glass(style);
			div()
				.w(px(212.))
				.flex()
				.flex_col()
				.gap(px(10.))
				.child(
					div()
						.h(px(116.))
						.p(px(14.))
						.flex()
						.flex_col()
						.justify_between()
						.rounded(radius::HUD_PILL)
						.rounded_smoothing_ios()
						.glass(styled.surface(Some(styled.hud_tint)).elevation(Elevation::Raised))
						.border_1()
						.border_color(scale_alpha(tokens::HAIRLINE, tokens::control::HUD_BORDER_OPACITY))
						.child(
							div()
								.text_size(px(13.))
								.font_weight(FontWeight::SEMIBOLD)
								.child(style.label()),
						)
						.child(label("Panel · radius 22 · raised")),
				)
				.child(
					div()
						.flex()
						.flex_row()
						.gap(px(8.))
						.items_center()
						.child(HudPill::new(
							SharedString::from(format!("style-pill-{}", style.label())),
							Icon::Globe,
							div().font_weight(FontWeight::SEMIBOLD).child("Global 04:12:09"),
							styled,
						)),
				)
				.child(
					div().flex().flex_row().gap(px(8.)).child(
						CapsuleButton::new(
							SharedString::from(format!("style-play-{}", style.label())),
							"Play",
							Tone::Accent(styled.accent),
							styled,
						)
						.icon(Icon::Play)
						.size(Size::Large),
					),
				)
		}))
}

/// The pointer light (`PointerSheen`) at a fixed position, in the current style.
fn pointer_demo(theme: Theme) -> impl IntoElement {
	let (left, top) = POINTER_DEMO_ORIGIN;
	let tile = |offset_x: f32, tint: Option<Rgba>, title: &'static str, pointer_tint: Rgba| {
		let center = point(px(left + offset_x + 70.), px(top + 52.));
		div()
			.absolute()
			.left(px(left + offset_x))
			.top(px(top + 34.))
			.w(px(150.))
			.h(px(40.))
			.rounded_full()
			.flex()
			.items_center()
			.justify_center()
			.text_size(tokens::text::BODY)
			.font_weight(FontWeight::SEMIBOLD)
			.text_color(tokens::contrast_tint(pointer_tint))
			.glass(
				theme
					.surface(tint)
					.elevation(Elevation::None)
					.pointer(Some(center), pointer_tint),
			)
			.border_1()
			.border_color(scale_alpha(pointer_tint, 0.4))
			.child(title)
	};
	div()
		.absolute()
		.size_full()
		.child(div().absolute().left(px(left)).top(px(top)).child(label(
			"Pointer light under the cursor (fixed here for the gallery)",
		)))
		.child(tile(
			0.,
			Some(scale_alpha(theme.accent, 0.14)),
			"Play",
			theme.accent,
		))
		.child(tile(
			170.,
			Some(tokens::black(0.52)),
			"Install Update",
			tokens::CONTROL_TINT,
		))
}

fn actions_section(theme: Theme) -> impl IntoElement {
	let tones = [
		("Accent", Tone::Accent(theme.accent)),
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
				.child(div().w(px(64.)).child(label(size_name)))
				.children(tones.iter().map(|(tone_name, tone)| {
					CapsuleButton::new(
						SharedString::from(format!("tone-{size_name}-{tone_name}")),
						*tone_name,
						*tone,
						theme,
					)
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
			.child(div().w(px(64.)).child(label("Disabled")))
			.children(tones.iter().map(|(tone_name, tone)| {
				CapsuleButton::new(
					SharedString::from(format!("disabled-{tone_name}")),
					*tone_name,
					*tone,
					theme,
				)
				.icon(Icon::Pause)
				.enabled(false)
			}))
			.into_any_element(),
	);
	rows.push(
		div()
			.flex()
			.flex_row()
			.items_center()
			.gap(px(18.))
			.child(div().w(px(64.)).child(label("Specials")))
			.child(IconButton::hud("gear", Icon::Gear, theme))
			.child(label("Icon button 40 / 44"))
			.child(accent_halo(
				theme.accent,
				0,
				&theme,
				CapsuleButton::new("halo-play", "Play", Tone::Accent(theme.accent), theme)
					.icon(Icon::Play)
					.size(Size::Large),
			))
			.child(label("Primary action with halo"))
			.into_any_element(),
	);
	div().flex().flex_col().gap(px(12.)).children(rows)
}

fn icons_section() -> impl IntoElement {
	div()
		.flex()
		.flex_row()
		.flex_wrap()
		.gap(px(10.))
		.children(Icon::ALL.into_iter().map(|icon| {
			div()
				.w(px(84.))
				.h(px(64.))
				.flex()
				.flex_col()
				.items_center()
				.justify_center()
				.gap(px(6.))
				.rounded(radius::CONTROL)
				.bg(white(0.05))
				.border_1()
				.border_color(white(0.08))
				.child(icon.render(px(20.), tokens::PRIMARY_TEXT))
				.child(label(format!("{icon:?}")))
		}))
}

fn hud_section(theme: Theme) -> impl IntoElement {
	let regions = ["Global", "Japan", "Korea", "China", "China (Bilibili)"];
	let expanded = HudPill::new(
		"expanded-status",
		Icon::Globe,
		div().font_weight(FontWeight::SEMIBOLD).child("Global 04:12:09"),
		theme,
	)
	.detail(
		div()
			.flex()
			.flex_col()
			.gap(px(2.))
			.children(regions.iter().enumerate().map(|(index, name)| {
				pill_row(
					("gallery-region", index),
					*name,
					"04:12:09",
					index == 0,
					theme,
					|_, _, _| {},
				)
			})),
		hud::EXPANDED_STATUS_WIDTH,
		px(28. + 8. + 16. + 5. * 28.),
	)
	.expanded(true);
	let collapsed = div()
		.flex()
		.flex_col()
		.gap(px(10.))
		.child(label("Collapsed capsules"))
		.child(
			HudPill::new(
				"music",
				Icon::Music,
				div()
					.font_weight(FontWeight::MEDIUM)
					.child("Sanity Restored — Lobby Theme"),
				theme,
			)
			.detail(div(), px(330.), px(100.)),
		)
		.child(
			HudPill::new(
				"version",
				Icon::Tag,
				div().font_weight(FontWeight::MEDIUM).child("2.6.81"),
				theme,
			)
			.detail(div(), px(260.), px(100.)),
		)
		.child(
			HudPill::new("receded", Icon::Tag, div().child("Receded sibling 0.62"), theme)
				.detail(div(), px(260.), px(100.))
				.receded(true),
		);
	let bar = |title: &'static str,
	           detail: &'static str,
	           emphasized: bool,
	           progress: Option<OutlineMode>,
	           active: bool,
	           action: AnyElement| {
		ControlBar::new(status_text(title, Some(detail.into()), emphasized), theme)
			.progress(progress, active)
			.action(action)
	};
	let accent = Tone::Accent(theme.accent);
	let bars = div()
		.flex_1()
		.flex()
		.flex_col()
		.gap(px(12.))
		.child(label("Control bar states"))
		.child(bar(
			"Ready to play",
			"Global · Version 2.6.81",
			false,
			None,
			false,
			CapsuleButton::new("bar-play", "Play", accent, theme)
				.icon(Icon::Play)
				.size(Size::Large)
				.into_any_element(),
		))
		.child(bar(
			"42%",
			"1.44 GB of 3.42 GB · 38.6 MB/s · 51 s left",
			true,
			Some(OutlineMode::Determinate(0.42)),
			true,
			CapsuleButton::new("bar-pause", "Pause", Tone::Neutral, theme)
				.icon(Icon::Pause)
				.size(Size::Large)
				.into_any_element(),
		))
		.child(bar(
			"Paused · 63%",
			"Resume continues from the partial files",
			true,
			Some(OutlineMode::Determinate(0.63)),
			false,
			CapsuleButton::new("bar-resume", "Resume", accent, theme)
				.icon(Icon::ArrowClockwise)
				.size(Size::Large)
				.into_any_element(),
		))
		.child(bar(
			"Verifying files",
			"Checking 1 284 files",
			false,
			Some(OutlineMode::Indeterminate),
			true,
			CapsuleButton::new("bar-verify", "Pause", Tone::Neutral, theme)
				.icon(Icon::Pause)
				.size(Size::Large)
				.enabled(false)
				.into_any_element(),
		));
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

fn tokens_section(theme: Theme) -> impl IntoElement {
	let swatches = [
		("Accent", theme.accent),
		("Cyan fallback", tokens::CYAN),
		("Control tint", tokens::CONTROL_TINT),
		("Quiet tint", tokens::QUIET_CONTROL_TINT),
		("Success", tokens::SUCCESS),
		("Warning", tokens::WARNING),
		("Danger", tokens::DANGER),
		("Danger text", tokens::DANGER_FOREGROUND),
		("HUD tint", theme.hud_tint),
	];
	let type_ramp = [
		("Title 22", tokens::text::TITLE),
		("Status percent 16", tokens::text::STATUS_PERCENT),
		("Status title 14", tokens::text::STATUS_TITLE),
		("Body 13", tokens::text::BODY),
		("Caption 10", tokens::text::CAPTION),
	];
	div()
		.flex()
		.flex_row()
		.gap(px(28.))
		.child(
			div()
				.flex()
				.flex_row()
				.flex_wrap()
				.w(px(560.))
				.gap(px(10.))
				.children(swatches.into_iter().map(|(name, color)| {
					div()
						.flex()
						.flex_col()
						.gap(px(4.))
						.child(
							div()
								.w(px(52.))
								.h(px(32.))
								.rounded(radius::CONTROL)
								.bg(color)
								.border_1()
								.border_color(white(0.14)),
						)
						.child(label(name))
				})),
		)
		.child(
			div()
				.flex()
				.flex_col()
				.gap(px(4.))
				.children(type_ramp.into_iter().map(|(name, size)| {
					div()
						.text_size(size)
						.font_weight(FontWeight::SEMIBOLD)
						.child(name)
				})),
		)
		.child(
			div().flex().flex_col().gap(px(8.)).children(
				[
					("Control 10", radius::CONTROL),
					("Panel 16", radius::PANEL),
					("HUD 22", radius::HUD_PILL),
					("Modal 24", radius::MODAL),
				]
				.into_iter()
				.map(|(name, corner)| {
					div()
						.flex()
						.flex_row()
						.items_center()
						.gap(px(8.))
						.child(
							div()
								.w(px(72.))
								.h(px(30.))
								.rounded(corner)
								.rounded_smoothing_ios()
								.bg(white(0.10))
								.border_1()
								.border_color(white(0.2)),
						)
						.child(label(name))
				}),
			),
		)
}

/// Step response of every motion curve over one second.
fn motion_section(accent: Rgba) -> impl IntoElement {
	let curves = [
		("press", Curve::Press, Some((0.20, 0.))),
		("release", Curve::Release, Some((0.46, 0.42))),
		("hover", Curve::Hover, Some((0.32, 0.22))),
		("state", Curve::State, Some((0.38, 0.16))),
		("morph", Curve::Morph, Some((0.52, 0.30))),
		("present", Curve::Present, Some((0.50, 0.24))),
		("dismiss", Curve::Dismiss, Some((0.24, 0.))),
		("crossfade", Curve::Crossfade, None),
	];
	div()
		.flex()
		.flex_row()
		.flex_wrap()
		.gap(px(12.))
		.children(curves.into_iter().map(move |(name, _curve, spring)| {
			div()
				.w(px(160.))
				.flex()
				.flex_col()
				.gap(px(4.))
				.child(
					div()
						.h(px(74.))
						.rounded(radius::CONTROL)
						.bg(tokens::black(0.45))
						.border_1()
						.border_color(white(0.08))
						.child(
							canvas(
								|_, _, _| {},
								move |bounds, _, window, _| plot(bounds, spring, accent, window),
							)
							.size_full(),
						),
				)
				.child(label(match spring {
					Some((duration, bounce)) => format!("{name} · spring {duration}s bounce {bounce}"),
					None => format!("{name} · ease in-out 0.48s"),
				}))
		}))
}

fn plot(bounds: Bounds<Pixels>, spring: Option<(f32, f32)>, accent: Rgba, window: &mut Window) {
	let left = f32::from(bounds.origin.x) + 6.;
	let top = f32::from(bounds.origin.y) + 10.;
	let width = f32::from(bounds.size.width) - 12.;
	let height = f32::from(bounds.size.height) - 20.;
	let mut target = PathBuilder::stroke(px(1.));
	target.move_to(point(px(left), px(top)));
	target.line_to(point(px(left + width), px(top)));
	if let Ok(path) = target.build() {
		window.paint_path(path, white(0.18));
	}
	let samples = 120;
	let value_at = |t: f32| -> f32 {
		match spring {
			Some((duration, bounce)) => {
				// Integrate the damped oscillator from rest toward 1.
				let config = swiftui_spring(duration, bounce);
				let (mut x, mut v) = (0f32, 0f32);
				let steps = (t / 0.001) as usize;
				for _ in 0..steps {
					let a = (config.stiffness * (1. - x) - config.damping * v) / config.mass;
					v += a * 0.001;
					x += v * 0.001;
				}
				x
			}
			None => {
				let p = (t / 0.48).min(1.);
				gpui::ease_in_out(p)
			}
		}
	};
	let mut curve = PathBuilder::stroke(px(2.));
	for index in 0..=samples {
		let t = index as f32 / samples as f32;
		let y = top + height * (1. - value_at(t));
		let p = point(px(left + width * t), px(y));
		if index == 0 {
			curve.move_to(p)
		} else {
			curve.line_to(p)
		}
	}
	if let Ok(path) = curve.build() {
		window.paint_path(path, accent);
	}
}

impl Render for Gallery {
	fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
		let theme = self.theme();
		div()
			.size_full()
			.relative()
			.overflow_hidden()
			.bg(gpui::black())
			.font_family(UI_FONT)
			.text_color(tokens::PRIMARY_TEXT)
			.track_focus(&self.focus)
			.key_context("Gallery")
			.on_action(cx.listener(|this, _: &CycleGlass, _, cx| {
				this.glass = this.glass.next();
				cx.notify();
			}))
			.on_action(cx.listener(|this, _: &CycleAccent, _, cx| {
				this.accent_index = (this.accent_index + 1) % ACCENTS.len();
				cx.notify();
			}))
			.on_action(cx.listener(|this, _: &ToggleReduceMotion, _, cx| {
				this.reduce_motion = !this.reduce_motion;
				cx.notify();
			}))
			.child(backdrop(theme.accent))
			.child(
				div()
					.absolute()
					.size_full()
					.p(px(36.))
					.pt(px(44.))
					.flex()
					.flex_col()
					.gap(px(30.))
					.child(
						div()
							.flex()
							.flex_col()
							.gap(px(4.))
							.child(
								div()
									.text_size(px(26.))
									.font_weight(FontWeight::BOLD)
									.child("Vitro component gallery"),
							)
							.child(label(format!(
								"Components below use {} · G next style · A next accent · M Reduce Motion ({})",
								theme.glass.label(),
								if theme.reduce_motion { "on" } else { "off" }
							))),
					)
					.child(section(
						"Glass styles",
						"Same tint, panel, pill, and accent action in each system look",
						glass_styles(theme),
					))
					.child(section(
						"Actions",
						"CapsuleActionButton tones and sizes, IconActionButton, accent halo",
						actions_section(theme),
					))
					.child(section(
						"HUD",
						"ExpandableHUDPill, HUDPillSurface, control bar with CapsuleProgressOutline",
						hud_section(theme),
					))
					.child(section(
						"Icons",
						"Project-owned SVG set (no SF Symbols)",
						icons_section(),
					))
					.child(section(
						"Tokens",
						"LauncherVisuals colors, type ramp, continuous corner radii",
						tokens_section(theme),
					))
					.child(section(
						"Motion",
						"LauncherMotion curves: step response over one second",
						motion_section(theme.accent),
					)),
			)
			.child(pointer_demo(theme))
	}
}

fn main() {
	gpui_platform::application()
		.with_assets(Assets::default().with(SLASH_ASSET, SLASH_SVG))
		.run(|cx: &mut App| {
			cx.bind_keys([
				KeyBinding::new("g", CycleGlass, Some("Gallery")),
				KeyBinding::new("a", CycleAccent, Some("Gallery")),
				KeyBinding::new("m", ToggleReduceMotion, Some("Gallery")),
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
			let bounds = Bounds::centered(None, size(px(WIDTH), px(HEIGHT)), cx);
			cx.open_window(
				WindowOptions {
					window_bounds: Some(WindowBounds::Windowed(bounds)),
					titlebar: Some(TitlebarOptions {
						title: Some("Vitro Gallery".into()),
						appears_transparent: true,
						traffic_light_position: None,
					}),
					..Default::default()
				},
				|window, cx| {
					cx.new(|cx| {
						let focus = cx.focus_handle();
						window.focus(&focus, cx);
						Gallery {
							focus,
							glass: GlassStyle::MacOS27(Transparency::Default),
							accent_index: 0,
							reduce_motion: cx.reduce_motion(),
						}
					})
				},
			)
			.expect("failed to open the gallery window");
			cx.activate(true);
		});
}
