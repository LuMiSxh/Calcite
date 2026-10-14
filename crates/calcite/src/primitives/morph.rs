// SPDX-License-Identifier: MPL-2.0

//! The math of a popup that morphs out of its trigger: one surface whose rect, corner radius,
//! fill, and border all follow a single spring value `t` (0 is exactly the trigger, 1 is exactly
//! the popup). Everything here is pure so the endpoints can be tested.

use gpui::GlassFilter;

use crate::{motion::secondary::GlassBoost, tokens::mix};

pub use gpui::Rgba;

/// How far past 1 the popup may grow while the spring overshoots on open.
pub const MAX_OVERSHOOT: f32 = 1.06;
/// Below this distance from 0 a closing spring counts as settled.
pub const SETTLE_EPSILON: f32 = 0.002;

/// Padding between a menu's edge and its rows.
pub const PANEL_PADDING: f32 = 5.;
/// Height of one row.
pub const ROW_HEIGHT: f32 = 26.;
/// Gap between children of the row list.
pub const ROW_GAP: f32 = 1.;
/// Height a divider takes in the list (line plus margins).
pub const DIVIDER_HEIGHT: f32 = 7.;
/// Border width of the surface at both ends.
pub const BORDER: f32 = 1.;
/// The narrowest a menu gets.
pub const MIN_WIDTH: f32 = 180.;

/// The surface at one instant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
	pub width: f32,
	pub height: f32,
	pub radius: f32,
	/// Opacity of the trigger's own label and chevron, drawn on the surface.
	pub header: f32,
	/// Opacity of the rows; they appear in the second half and leave first.
	pub rows: f32,
	/// Strength of the drop shadow, 0 at the trigger.
	pub shadow: f32,
	/// The clamped progress, for color gradients.
	pub unit: f32,
}

fn lerp(from: f32, to: f32, t: f32) -> f32 {
	from + (to - from) * t
}

/// The surface at spring value `t`. At 0 it is exactly the `trigger` rect with a fully round
/// capsule radius, at 1 exactly the `menu` rect with `menu_radius`. Overshoot grows the popup a
/// little (capped at [`MAX_OVERSHOOT`]); undershoot never goes below the trigger.
pub fn frame(t: f32, trigger: (f32, f32), menu: (f32, f32), menu_radius: f32) -> Frame {
	let menu = (menu.0.max(trigger.0), menu.1.max(trigger.1));
	let grow = t.clamp(0., MAX_OVERSHOOT);
	let unit = t.clamp(0., 1.);
	let width = lerp(trigger.0, menu.0, grow).max(trigger.0);
	let height = lerp(trigger.1, menu.1, grow).max(trigger.1);
	let capsule = trigger.0.min(trigger.1) / 2.;
	let radius = lerp(capsule, menu_radius, unit).min(width.min(height) / 2.);
	Frame {
		width,
		height,
		radius,
		header: (1. - unit / 0.55).clamp(0., 1.),
		rows: ((unit - 0.5) / 0.5).clamp(0., 1.),
		shadow: unit,
		unit,
	}
}

/// Whether the spring has come to rest closed, so only the trigger remains. `active` is the
/// spring's own verdict (see `SpringSample::active`): a value that is near 0 but still moving has
/// not settled, and removing the surface then would cut the last frames of the close.
pub fn settled_closed(t: f32, open: bool, active: bool) -> bool {
	!open && !active && t.abs() < SETTLE_EPSILON
}

/// `filter` with the extra light of fast-moving glass: brightness and specular added, refraction
/// multiplied (see `secondary::glass_boost`). [`GlassBoost::NONE`] returns it unchanged.
pub fn boost_filter(mut filter: GlassFilter, boost: GlassBoost) -> GlassFilter {
	filter.brightness += boost.brightness;
	filter.specular += boost.specular;
	filter.refraction = gpui::px(f32::from(filter.refraction) * boost.refraction);
	filter
}

/// A color between the trigger's and the menu's at spring value `t` (clamped to 0..1).
pub fn gradient(from: Rgba, to: Rgba, t: f32) -> Rgba {
	mix(from, to, t.clamp(0., 1.))
}

/// The glass between two filters: every optic and the fill interpolate, so the lens deepens from
/// the trigger's look to the menu's. The pointer light stays the trigger's.
pub fn mix_filter(from: &GlassFilter, to: &GlassFilter, t: f32) -> GlassFilter {
	let t = t.clamp(0., 1.);
	let fill = mix(
		crate::tokens::rgba(from.fill[0], from.fill[1], from.fill[2], from.fill[3]),
		crate::tokens::rgba(to.fill[0], to.fill[1], to.fill[2], to.fill[3]),
		t,
	);
	let pixels = |a: gpui::Pixels, b: gpui::Pixels| gpui::px(lerp(f32::from(a), f32::from(b), t));
	GlassFilter {
		blur: pixels(from.blur, to.blur),
		bezel: pixels(from.bezel, to.bezel),
		refraction: pixels(from.refraction, to.refraction),
		dispersion: lerp(from.dispersion, to.dispersion, t),
		saturation: lerp(from.saturation, to.saturation, t),
		brightness: lerp(from.brightness, to.brightness, t),
		fill: [fill.red, fill.green, fill.blue, fill.alpha],
		rim: lerp(from.rim, to.rim, t),
		specular: lerp(from.specular, to.specular, t),
		light_angle: lerp(from.light_angle, to.light_angle, t),
		pointer: from.pointer,
		pointer_radius: from.pointer_radius,
		pointer_color: from.pointer_color,
	}
}

/// What the size of a row depends on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RowSpec {
	/// Estimated width of the label, see [`text_width`].
	pub label: f32,
	pub icon: bool,
	pub divider_before: bool,
}

/// A rough width of `text` at 12 pt: wide scripts count double.
pub fn text_width(text: &str) -> f32 {
	text.chars().map(|c| if c.is_ascii() { 6.4 } else { 12. }).sum()
}

/// The size of a menu with `rows`, at least as wide as its trigger.
pub fn menu_size(trigger_width: f32, rows: &[RowSpec]) -> (f32, f32) {
	let widest = rows
		.iter()
		.map(|row| 8. + 12. + 7. + if row.icon { 19. } else { 0. } + row.label + 8.)
		.fold(0., f32::max);
	let width = (widest + 2. * PANEL_PADDING + 2. * BORDER)
		.max(MIN_WIDTH)
		.max(trigger_width);
	let dividers = rows.iter().filter(|row| row.divider_before).count() as f32;
	let children = rows.len() as f32 + dividers;
	let list = rows.len() as f32 * ROW_HEIGHT + dividers * DIVIDER_HEIGHT + (children - 1.).max(0.) * ROW_GAP;
	(width, list + 2. * PANEL_PADDING + 2. * BORDER)
}

/// Which way the menu grows from its trigger.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
	/// Top edges aligned, growing downward like a macOS popup button.
	Down,
	/// Bottom edges aligned, growing upward.
	Up,
}

/// Grows downward unless the menu would leave the window, then upward if that fits or there is
/// more room above. Decided from the final menu height so it does not flip mid-animation.
pub fn direction(
	trigger_top: f32,
	trigger_height: f32,
	menu_height: f32,
	viewport_height: f32,
	margin: f32,
) -> Direction {
	if trigger_top + menu_height + margin <= viewport_height {
		return Direction::Down;
	}
	let above = trigger_top + trigger_height;
	let below = viewport_height - trigger_top;
	if above - menu_height >= margin || above > below {
		Direction::Up
	} else {
		Direction::Down
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::tokens::{black, white};

	const TRIGGER: (f32, f32) = (80., 24.);
	const MENU: (f32, f32) = (180., 120.);

	#[test]
	fn endpoints_are_exact() {
		let closed = frame(0., TRIGGER, MENU, 16.);
		assert_eq!((closed.width, closed.height, closed.radius), (80., 24., 12.));
		assert_eq!((closed.header, closed.rows, closed.shadow), (1., 0., 0.));
		let open = frame(1., TRIGGER, MENU, 16.);
		assert_eq!((open.width, open.height, open.radius), (180., 120., 16.));
		assert_eq!((open.header, open.rows, open.shadow), (0., 1., 1.));
	}

	#[test]
	fn undershoot_never_goes_below_the_trigger() {
		let s = frame(-0.3, TRIGGER, MENU, 16.);
		assert_eq!((s.width, s.height, s.radius), (80., 24., 12.));
	}

	#[test]
	fn overshoot_grows_but_is_capped() {
		let s = frame(1.04, TRIGGER, MENU, 16.);
		assert!(s.width > 180. && s.height > 120.);
		let capped = frame(9., TRIGGER, MENU, 16.);
		assert_eq!(capped.width, lerp(80., 180., MAX_OVERSHOOT));
		assert_eq!(capped.radius, 16.);
	}

	#[test]
	fn a_menu_smaller_than_the_trigger_keeps_the_trigger_size() {
		let s = frame(1., TRIGGER, (40., 10.), 16.);
		assert_eq!((s.width, s.height), (80., 24.));
	}

	#[test]
	fn radius_never_exceeds_half_the_short_side() {
		for step in 0..=120 {
			let s = frame(step as f32 / 100., TRIGGER, MENU, 40.);
			assert!(s.radius <= s.width.min(s.height) / 2. + 1e-4);
		}
	}

	#[test]
	fn rows_arrive_late_and_header_leaves_early() {
		let s = frame(0.4, TRIGGER, MENU, 16.);
		assert_eq!(s.rows, 0.);
		assert!(s.header > 0.2);
		let s = frame(0.6, TRIGGER, MENU, 16.);
		assert!(s.rows > 0. && s.header < 0.1);
		assert_eq!(frame(0.55, TRIGGER, MENU, 16.).header, 0.);
	}

	#[test]
	fn settled_needs_closed_target_and_tiny_distance() {
		assert!(settled_closed(0., false, false));
		assert!(settled_closed(-0.001, false, false));
		assert!(!settled_closed(0.01, false, false));
		assert!(!settled_closed(0., true, false));
		// Within the distance but still moving: not settled yet.
		assert!(!settled_closed(0.001, false, true));
	}

	#[test]
	fn gradient_hits_both_ends_and_clamps() {
		let (a, b) = (white(0.2), black(0.8));
		assert_eq!(gradient(a, b, 0.), a);
		assert_eq!(gradient(a, b, 1.), b);
		assert_eq!(gradient(a, b, -1.), a);
		assert_eq!(gradient(a, b, 2.), b);
		let mid = gradient(a, b, 0.5);
		assert!((mid.alpha - 0.5).abs() < 1e-6);
	}

	#[test]
	fn filters_interpolate_between_their_ends() {
		let style = crate::GlassStyle::MacOS26;
		let from = crate::glass::Glass::new(None, style)
			.elevation(crate::glass::Elevation::None)
			.filter()
			.unwrap();
		let to = crate::glass::Glass::new(Some(white(1.)), style)
			.elevation(crate::glass::Elevation::Raised)
			.filter()
			.unwrap();
		assert_eq!(mix_filter(&from, &to, 0.).fill, from.fill);
		assert_eq!(mix_filter(&from, &to, 1.).fill, to.fill);
		assert_eq!(mix_filter(&from, &to, 0.).blur, from.blur);
		assert_eq!(mix_filter(&from, &to, 1.).bezel, to.bezel);
		assert_eq!(mix_filter(&from, &to, -3.).rim, from.rim);
		let mid = mix_filter(&from, &to, 0.5);
		assert!(mid.blur > from.blur.min(to.blur) || from.blur == to.blur);
	}

	#[test]
	fn boosting_adds_light_and_deepens_the_lens() {
		let base = GlassFilter {
			brightness: 1.,
			specular: 0.5,
			refraction: gpui::px(20.),
			..GlassFilter::default()
		};
		assert_eq!(boost_filter(base, GlassBoost::NONE), base);
		let full = boost_filter(base, crate::motion::secondary::glass_boost(1., false));
		assert!((full.brightness - 1.06).abs() < 1e-5);
		assert!((full.specular - 0.85).abs() < 1e-5);
		assert!((f32::from(full.refraction) - 25.).abs() < 1e-4);
		assert_eq!(full.blur, base.blur);
	}

	fn row(label: f32, icon: bool, divider: bool) -> RowSpec {
		RowSpec {
			label,
			icon,
			divider_before: divider,
		}
	}

	#[test]
	fn menu_size_counts_rows_dividers_and_chrome() {
		let (_, one) = menu_size(60., &[row(30., false, false)]);
		assert_eq!(one, 26. + 10. + 2.);
		let (_, three) = menu_size(
			60.,
			&[
				row(30., false, false),
				row(30., false, true),
				row(30., false, false),
			],
		);
		// Three rows, one divider, four children with three gaps.
		assert_eq!(three, 78. + 7. + 3. + 10. + 2.);
	}

	#[test]
	fn menu_width_follows_the_trigger_and_labels() {
		assert_eq!(menu_size(60., &[row(10., false, false)]).0, MIN_WIDTH);
		assert_eq!(menu_size(300., &[row(10., false, false)]).0, 300.);
		assert!(menu_size(60., &[row(400., true, false)]).0 > 400.);
		assert!(text_width("日本") > text_width("ab"));
	}

	#[test]
	fn menus_grow_down_unless_they_would_overflow() {
		assert_eq!(direction(100., 24., 120., 800., 8.), Direction::Down);
		assert_eq!(direction(700., 24., 120., 800., 8.), Direction::Up);
		// Neither fits, but there is more room above.
		assert_eq!(direction(300., 24., 500., 600., 8.), Direction::Up);
		// Neither fits and the top is cramped.
		assert_eq!(direction(40., 24., 500., 400., 8.), Direction::Down);
	}
}
