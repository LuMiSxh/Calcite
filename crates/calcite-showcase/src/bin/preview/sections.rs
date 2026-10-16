// SPDX-License-Identifier: MPL-2.0

//! Static preview sections: the backdrop, the glass styles, icons, tokens, and motion.

use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};

use calcite::{
	glass::{Elevation, GlassExt},
	motion::{Spring, swiftui_spring, tokens as spring_tokens},
	prelude::*,
	tokens::{rgba, scale_alpha, white},
};
use calcite_showcase::artwork::SLASH_ASSET;
use gpui::{
	AnyElement, App, Bounds, ClipboardItem, Context, FontWeight, Image, ImageFormat, ImageSource,
	IntoElement, ParentElement, PathBuilder, Pixels, Render, RenderOnce, Rgba, SharedString, Styled, Window,
	canvas, div, img, linear_color_stop, linear_gradient, point, prelude::FluentBuilder, px,
};

use super::{WIDTH, coverage::rows, section_view::Kind};

pub fn section(
	title: &'static str,
	note: impl Into<SharedString>,
	sources: impl IntoElement,
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
		.child(sources)
		.child(content)
}

/// The Rust call that builds a component, as plain monospace text.
#[derive(IntoElement)]
pub struct SourceLine(SharedString);

pub fn source_line(text: impl Into<SharedString>) -> SourceLine {
	SourceLine(text.into())
}

impl RenderOnce for SourceLine {
	fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = cx.theme();
		div()
			.font_family(theme.mono())
			.text_size(px(10.))
			.text_color(theme.palette.secondary_text)
			.child(self.0)
	}
}

/// A section's `Source` block: one monospace line per component in its [`COVERAGE`] rows.
pub fn source_block(kind: Kind) -> impl IntoElement {
	div().flex().flex_col().gap(px(1.)).children(
		rows(kind)
			.filter(|(_, _, source)| !source.is_empty())
			.map(|(component, _, source)| source_line(format!("Source · {component}: {source}"))),
	)
}

/// Renders an expression and shows the source it was built from, under it. `stringify!` reads
/// the tokens that build the element, so the snippet cannot go stale:
/// `demo!(button(id, "Save").tone(Tone::Accent))`.
macro_rules! demo {
	($expr:expr) => {
		crate::sections::shown(stringify!($expr), $expr)
	};
}
pub(crate) use demo;

/// An element and the source text it was built from, shown with a copy button.
#[derive(IntoElement)]
pub struct Shown {
	source: &'static str,
	content: AnyElement,
}

/// The element of a [`demo!`]: use the macro, which supplies `source`.
pub fn shown(source: &'static str, content: impl IntoElement) -> Shown {
	Shown {
		source,
		content: content.into_any_element(),
	}
}

impl RenderOnce for Shown {
	fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
		// `stringify!` keeps the line breaks of multi-line calls; the snippet is one line.
		let source: SharedString = self
			.source
			.split_whitespace()
			.collect::<Vec<_>>()
			.join(" ")
			.into();
		let copied = source.clone();
		div()
			.flex()
			.flex_col()
			.items_start()
			.gap(px(6.))
			.child(self.content)
			.child(
				div()
					.flex()
					.flex_row()
					.items_center()
					.gap(px(8.))
					.child(source_line(source.clone()))
					.child(
						button(SharedString::from(format!("copy-{source}")), "Copy")
							.compact()
							.on_click(move |_, _, cx| {
								cx.write_to_clipboard(ClipboardItem::new_string(copied.to_string()));
							}),
					),
			)
	}
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

/// The backdrop as a view of its own: it never changes with the page, so it is cached.
pub struct BackdropView;

impl Render for BackdropView {
	fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
		let theme = cx.theme();
		backdrop(&theme).into_any_element()
	}
}

/// A bright, detailed backdrop over the whole page, so every glass surface has something
/// to refract.
pub fn backdrop(theme: &Theme) -> impl IntoElement {
	let accent = theme.accent;
	// A light palette gets a pale gradient and dark rules.
	let light = theme.palette.is_light();
	// Rules read as faint lines on pale artwork, so they are far lighter than on dark.
	let ink = |alpha: f32| {
		if light {
			tokens::black(alpha * 0.3)
		} else {
			white(alpha)
		}
	};
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
			linear_color_stop(
				if light {
					rgba(0.80, 0.86, 0.95, 1.)
				} else {
					rgba(0.07, 0.10, 0.16, 1.)
				},
				0.,
			),
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
				.bg(ink(if index % 4 == 0 { 0.22 } else { 0.10 }))
		}))
		// More rules than any page needs; the backdrop clips them to the content height.
		.children((0..120).map(|index| {
			div()
				.absolute()
				.top(px(40. + index as f32 * 90.))
				.left_0()
				.w_full()
				.h(px(1.))
				.bg(ink(0.16))
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
	// The surface at each elevation, in the current glass style.
	let elevations = div()
		.flex()
		.flex_col()
		.gap(px(8.))
		.child(label(format!(
			"Elevation, in {}: None sits inside another surface, Resting floats, Raised is an open HUD panel",
			theme.style.glass.label()
		)))
		.child(
			div().flex().flex_row().gap(px(16.)).children(
				[
					(Elevation::None, "None"),
					(Elevation::Resting, "Resting"),
					(Elevation::Raised, "Raised"),
				]
				.map(|(elevation, name)| {
					div()
						.w(px(212.))
						.h(px(72.))
						.p(px(12.))
						.rounded(theme.style.radii.hud_pill)
						.rounded_smoothing_ios()
						.glass(theme.hud_surface().elevation(elevation))
						.child(
							div()
								.text_size(px(12.))
								.font_weight(FontWeight::SEMIBOLD)
								.child(name),
						)
				}),
			),
		);
	div()
		.flex()
		.flex_col()
		.gap(px(22.))
		.child(glass_style_cards(theme))
		.child(elevations)
}

fn glass_style_cards(theme: Theme) -> impl IntoElement {
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
						.rounded(theme.style.radii.hud_pill)
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
						.child(label("Panel · HUD radius · raised")),
				)
				.child(
					div().flex().child(
						pill(
							SharedString::from(format!("style-pill-{key}")),
							Icon::Globe,
							|| div().font_weight(FontWeight::SEMIBOLD).child("Global 04:12:09"),
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

pub fn tokens_section(theme: Theme) -> impl IntoElement {
	let corner = theme.style.radii.control;
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
								.rounded(corner)
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
					("Control 10", Radii::DEFAULT.control),
					("Panel 16", Radii::DEFAULT.panel),
					("HUD 22", Radii::DEFAULT.hud_pill),
					("Modal 24", Radii::DEFAULT.modal),
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
pub fn motion_section(accent: Rgba, corner: Pixels) -> impl IntoElement {
	let params = |spring: Spring| Some((spring.response, spring.bounce));
	let curve = |curve: Curve| curve.spring().and_then(params);
	let curves = [
		("press", Curve::Press, curve(Curve::Press)),
		("release", Curve::Release, curve(Curve::Release)),
		("hover", Curve::Hover, curve(Curve::Hover)),
		("state", Curve::State, curve(Curve::State)),
		("morph open", Curve::State, params(spring_tokens::MORPH_OPEN)),
		("present", Curve::Present, curve(Curve::Present)),
		("dismiss", Curve::Dismiss, curve(Curve::Dismiss)),
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
						.rounded(corner)
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

/// Points along each plotted curve.
const SAMPLES: usize = 120;

/// The step response of a spring over one second, sampled at [`SAMPLES`] + 1 points. The
/// integration is the costliest part of a redraw and the curves never change, so each is
/// computed once.
fn spring_samples((duration, bounce): (f32, f32)) -> Rc<[f32]> {
	type Cache = HashMap<(u32, u32), Rc<[f32]>>;
	thread_local! {
		static CACHE: RefCell<Cache> = RefCell::default();
	}
	CACHE.with(|cache| {
		cache
			.borrow_mut()
			.entry((duration.to_bits(), bounce.to_bits()))
			.or_insert_with(|| {
				// Integrate the damped oscillator from rest toward 1.
				let config = swiftui_spring(duration, bounce);
				let (mut x, mut v) = (0f32, 0f32);
				let mut steps_done = 0;
				(0..=SAMPLES)
					.map(|index| {
						let steps = ((index as f32 / SAMPLES as f32) / 0.001) as usize;
						for _ in steps_done..steps {
							let a = (config.stiffness * (1. - x) - config.damping * v) / config.mass;
							v += a * 0.001;
							x += v * 0.001;
						}
						steps_done = steps;
						x
					})
					.collect()
			})
			.clone()
	})
}

pub(super) fn plot(bounds: Bounds<Pixels>, spring: Option<(f32, f32)>, accent: Rgba, window: &mut Window) {
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
	let samples = SAMPLES;
	let spring_samples = spring.map(spring_samples);
	let value_at = |index: usize| -> f32 {
		match &spring_samples {
			Some(values) => values[index],
			None => {
				let p = (index as f32 / samples as f32 / 0.48).min(1.);
				gpui::ease_in_out(p)
			}
		}
	};
	let mut curve = PathBuilder::stroke(px(2.));
	for index in 0..=samples {
		let t = index as f32 / samples as f32;
		let y = top + height * (1. - value_at(index));
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
/// The app icon's PNG, as the Dock, the window and the bundle use it.
static ICON_PNG: &[u8] = include_bytes!("../../../assets/icon.png");

/// `calcite::app_icon!` keeps the PNG and decodes it once for the window icon; here it is drawn
/// at two sizes from the same bytes.
fn app_icon_column() -> impl IntoElement {
	let source = ImageSource::Image(Arc::new(Image::from_bytes(ImageFormat::Png, ICON_PNG.to_vec())));
	div()
		.flex()
		.flex_col()
		.gap(px(8.))
		.child(label("app_icon! · the PNG the Dock and the window show"))
		.child(
			div()
				.flex()
				.flex_row()
				.items_end()
				.gap(px(16.))
				.child(img(source.clone()).size(px(128.)))
				.child(img(source).size(px(48.))),
		)
}

pub fn chrome_section(corner: Pixels) -> impl IntoElement {
	let strip = |style: ChromeStyle, name: &'static str| {
		div().flex().flex_col().gap(px(8.)).child(label(name)).child(
			div()
				.relative()
				.w(px(440.))
				.h(px(52.))
				.rounded(corner)
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
		.flex_wrap()
		.gap(px(24.))
		.child(app_icon_column())
		.child(strip(ChromeStyle::MacOs, "macOS: system traffic lights"))
		.child(strip(ChromeStyle::Windows, "Windows: round captions, red close"))
		.child(strip(
			ChromeStyle::Linux,
			"Linux: round captions, client-side frame",
		))
}
