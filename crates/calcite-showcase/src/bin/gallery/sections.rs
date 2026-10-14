// SPDX-License-Identifier: MPL-2.0

//! Static gallery sections: the backdrop, the glass styles, icons, tokens, and motion.

use calcite::{
	glass::{Elevation, GlassExt},
	motion::swiftui_spring,
	prelude::*,
	tokens::{radius, rgba, scale_alpha, white},
};
use calcite_showcase::artwork::SLASH_ASSET;
use gpui::{
	App, Bounds, FontWeight, IntoElement, ParentElement, PathBuilder, Pixels, RenderOnce, Rgba, SharedString,
	Styled, Window, canvas, div, linear_color_stop, linear_gradient, point, prelude::FluentBuilder, px,
};

use super::{HEIGHT, WIDTH};

pub fn section(
	title: &'static str,
	note: impl Into<SharedString>,
	content: impl IntoElement,
) -> impl IntoElement {
	let note = label(note);
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
				.child(note),
		)
		.child(content)
}

/// A caption in the theme's secondary text color.
#[derive(IntoElement)]
pub struct Label(SharedString);

pub fn label(text: impl Into<SharedString>) -> Label {
	Label(text.into())
}

impl RenderOnce for Label {
	fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
		div()
			.text_size(px(10.))
			.text_color(cx.theme().palette.secondary_text)
			.child(self.0)
	}
}

/// A bright, detailed backdrop over the whole page, so every glass surface has something
/// to refract.
pub fn backdrop(theme: &Theme) -> impl IntoElement {
	let accent = theme.accent;
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
			linear_color_stop(theme.palette.backdrop, 1.),
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

/// One card per glass style, each with a HUD pill and an accent action in that style.
pub fn glass_styles(theme: Theme) -> impl IntoElement {
	div()
		.flex()
		.flex_row()
		.gap(px(16.))
		.children(GlassStyle::ALL.into_iter().map(move |style| {
			let styled = theme.with_glass(style);
			let key = style.label();
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
						.glass(styled.hud_surface().elevation(Elevation::Raised))
						.border_1()
						.border_color(scale_alpha(
							theme.palette.hairline,
							tokens::control::HUD_BORDER_OPACITY,
						))
						.child(
							div()
								.text_size(px(13.))
								.font_weight(FontWeight::SEMIBOLD)
								.child(key),
						)
						.child(label("Panel · radius 22 · raised")),
				)
				.child(
					div().flex().child(
						pill(
							SharedString::from(format!("style-pill-{key}")),
							Icon::Globe,
							div().font_weight(FontWeight::SEMIBOLD).child("Global 04:12:09"),
						)
						.theme(styled),
					),
				)
				.child(
					div().flex().child(
						button(SharedString::from(format!("style-play-{key}")), "Play")
							.accent()
							.large()
							.icon(Icon::Play)
							.theme(styled),
					),
				)
		}))
}

pub fn icons_section(theme: &Theme) -> impl IntoElement {
	let tile = |glyph: gpui::Svg, name: &'static str| {
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
			.child(glyph)
			.child(label(name))
	};
	// A sample of the full set; every icon is available as `Lucide::Name`.
	let lucide = [
		Lucide::Rocket,
		Lucide::Gamepad2,
		Lucide::Download,
		Lucide::HardDrive,
		Lucide::Shield,
		Lucide::Bell,
		Lucide::Wifi,
		Lucide::Cpu,
		Lucide::Clock,
		Lucide::Star,
		Lucide::Heart,
		Lucide::Terminal,
		Lucide::Wine,
		Lucide::Languages,
		Lucide::Moon,
	];
	div()
		.flex()
		.flex_col()
		.gap(px(10.))
		.child(label("Icon: the semantic set the components use"))
		.child(
			div().flex().flex_row().flex_wrap().gap(px(10.)).children(
				Icon::ALL
					.iter()
					.map(|icon| tile(icon.render(px(20.), theme.palette.primary_text), icon.name())),
			),
		)
		.child(label(format!("Lucide: {} icons, a sample", Lucide::ALL.len())))
		.child(
			div()
				.flex()
				.flex_row()
				.flex_wrap()
				.gap(px(10.))
				.children(lucide.into_iter().map(|icon| {
					tile(
						icon.render(px(20.), theme.palette.primary_text),
						IconSet::name(icon),
					)
				})),
		)
}

pub fn tokens_section(theme: Theme) -> impl IntoElement {
	let swatches = [
		("Accent", theme.accent),
		("Cyan fallback", theme.palette.accent),
		("Control tint", theme.palette.control),
		("Quiet tint", theme.palette.quiet_control),
		("Success", theme.palette.success),
		("Warning", theme.palette.warning),
		("Danger", theme.palette.danger),
		("Danger text", theme.palette.danger_text),
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
pub fn motion_section(accent: Rgba) -> impl IntoElement {
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

/// The title bar of each platform over a strip of backdrop. Only the native one moves the window.
pub fn chrome_section() -> impl IntoElement {
	let strip = |style: ChromeStyle, name: &'static str| {
		div().flex().flex_col().gap(px(8.)).child(label(name)).child(
			div()
				.relative()
				.w(px(440.))
				.h(px(52.))
				.rounded(radius::CONTROL)
				.overflow_hidden()
				.bg(white(0.05))
				.border_1()
				.border_color(white(0.08))
				// The system draws the traffic lights; the preview stands in for them.
				.when(style == ChromeStyle::MacOs, |strip| {
					strip.child(
						div()
							.absolute()
							.left(px(20.))
							.top(px(20.))
							.flex()
							.gap(px(8.))
							.children(
								[0xff5f57, 0xfebc2e, 0x28c840]
									.map(|color| div().size(px(12.)).rounded_full().bg(gpui::rgb(color))),
							),
					)
				})
				.child(
					title_bar(SharedString::from(format!("chrome-preview-{name}")))
						.preview(style)
						.child(
							icon_button(
								SharedString::from(format!("chrome-gear-{name}")),
								Icon::Gear,
								"Settings",
							)
							.diameter(px(36.)),
						),
				),
		)
	};
	div()
		.flex()
		.flex_row()
		.gap(px(24.))
		.child(strip(ChromeStyle::MacOs, "macOS: system traffic lights"))
		.child(strip(ChromeStyle::Windows, "Windows: caption buttons"))
		.child(strip(ChromeStyle::Linux, "Linux: client-side decorations"))
}
