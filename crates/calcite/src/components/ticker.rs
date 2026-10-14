// SPDX-License-Identifier: MPL-2.0

//! A rolling number: text whose digits roll when they change, the way counters, percentages,
//! sizes, and timers tick over. Every digit has the width of the widest digit, so a changing
//! digit never shifts the text after it.

use std::{sync::Arc, time::Instant};

use gpui::{
	App, Font, FontFeatures, IntoElement, ParentElement, Pixels, RenderOnce, Rgba, SharedString, Styled,
	Window, div, prelude::*, px,
};

use crate::{
	controls::theme_override,
	motion::{self, secondary, tokens::TICKER},
	theme::{ActiveTheme, Theme},
};

/// The line box of a glyph, as a multiple of the font size. Glyphs roll inside it.
const LINE: f32 = 1.25;

/// A rolling number. Build one with [`ticker`].
#[derive(IntoElement)]
pub struct Ticker {
	id: gpui::ElementId,
	text: SharedString,
	size: Pixels,
	color: Option<Rgba>,
	mono_width: bool,
	theme: Option<Theme>,
}

/// A rolling number of `text` at 15 pt in the primary text color, with fixed-width digits.
///
/// When the text changes, each character that changed slides out upward while its
/// replacement slides in from below. The direction follows the value: a number that goes
/// up rolls up, one that goes down rolls down. Under Reduce Motion the swap is instant.
///
/// ```ignore
/// use calcite::prelude::*;
///
/// // `percent` is any number the view owns; it rolls whenever the text changes.
/// ticker("download-percent", format!("{percent}%"))
///     .size(px(28.))
///     .color(theme.palette.primary_text)
/// ```
pub fn ticker(id: impl Into<gpui::ElementId>, text: impl Into<SharedString>) -> Ticker {
	Ticker {
		id: id.into(),
		text: text.into(),
		size: px(15.),
		color: None,
		mono_width: true,
		theme: None,
	}
}

impl Ticker {
	/// The font size; the line box is 1.25 times this.
	pub fn size(mut self, size: Pixels) -> Self {
		self.size = size;
		self
	}

	/// The text color, `palette.primary_text` by default.
	pub fn color(mut self, color: Rgba) -> Self {
		self.color = Some(color);
		self
	}

	/// Gives every digit the width of the widest digit, and turns on tabular figures. On by
	/// default. Other characters (`%`, `.`, `,`, `:`) keep their natural width.
	pub fn mono_width(mut self, mono_width: bool) -> Self {
		self.mono_width = mono_width;
		self
	}
}

theme_override!(Ticker);

/// Which way a changed number moves. `Up` means the value rose: the old glyph leaves
/// upward and the new one enters from below.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
	Up,
	Down,
}

/// The numeric value of `text` when it reads as a number. Thousands separators and one
/// trailing `%` are ignored, so `"1,250"` and `"45%"` parse.
fn parse_number(text: &str) -> Option<f64> {
	let trimmed = text.trim().trim_end_matches('%').trim_end();
	trimmed
		.replace(',', "")
		.parse::<f64>()
		.ok()
		.filter(|value| value.is_finite())
}

/// The roll direction from `prev` to `next`: down only when both read as numbers and the
/// number fell. Anything else, including text that does not parse, rolls up.
fn direction(prev: &str, next: &str) -> Direction {
	match (parse_number(prev), parse_number(next)) {
		(Some(before), Some(after)) if after < before => Direction::Down,
		_ => Direction::Up,
	}
}

/// Pairs every character of `next` with the character at the same distance from the right
/// end of `prev`, so units such as `%` line up when the length changes. The pair is `None`
/// when `prev` has no character there. Index 0 is the leftmost character of `next`.
fn align_from_right(prev: &[char], next: &[char]) -> Vec<(Option<char>, char)> {
	next.iter()
		.enumerate()
		.map(|(index, &ch)| {
			let old = (index + prev.len())
				.checked_sub(next.len())
				.and_then(|prev_index| prev.get(prev_index).copied());
			(old, ch)
		})
		.collect()
}

/// Digits share one width when `mono` is on. Every other character keeps its natural width.
fn cell_width(ch: char, digit_width: f32, natural: f32, mono: bool) -> f32 {
	if mono && ch.is_ascii_digit() {
		digit_width
	} else {
		natural
	}
}

/// Where the outgoing and incoming glyph of a roll sit, as offsets from the cell's rest
/// position. `progress` runs from 0 (the old glyph shown) to 1 (the new glyph at rest).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Slide {
	old_y: f32,
	old_opacity: f32,
	new_y: f32,
	new_opacity: f32,
}

/// The furthest past its rest position a rolling glyph may travel, as a roll progress.
const MAX_OVERSHOOT: f32 = 1.15;

/// Positions follow `progress` unclamped above 1, up to [`MAX_OVERSHOOT`], so a digit rolls a hair
/// past its rest position and settles back; opacities stay within 0..=1.
fn slide(progress: f32, direction: Direction, height: f32) -> Slide {
	let opacity = progress.clamp(0., 1.);
	let progress = progress.clamp(0., MAX_OVERSHOOT);
	let sign = match direction {
		Direction::Up => -1.,
		Direction::Down => 1.,
	};
	Slide {
		old_y: sign * progress * height,
		old_opacity: 1. - opacity,
		new_y: -sign * (1. - progress) * height,
		new_opacity: opacity,
	}
}

/// Start delays in seconds for the cells of a number, left to right. Changed cells start from the
/// right, each `step` after the one to its right; unchanged cells have no delay. A single changed
/// digit starts at once.
fn digit_delays(changed: &[bool], step: f32, reduced: bool) -> Vec<f32> {
	let mut order = 0;
	let mut delays = vec![0.; changed.len()];
	for (delay, &changed) in delays.iter_mut().zip(changed).rev() {
		if changed {
			*delay = secondary::stagger_delay(order, true, step, reduced);
			order += 1;
		}
	}
	delays
}

/// Between digits: right-to-left, 25 ms apart.
const DIGIT_STEP: f32 = 0.025;

/// The text the ticker showed last, and the direction of its latest change.
struct Shown {
	text: String,
	direction: Direction,
}

/// One character position, counted from the right, and its roll.
struct Cell {
	/// The character that rolls out, if there was one.
	from: Option<char>,
	to: char,
	direction: Direction,
	/// When the latest change arrived, and how long its roll waits behind the digits to its right.
	changed_at: Instant,
	delay: f32,
}

/// What a cell paints this frame.
struct CellFrame {
	width: f32,
	from: Option<char>,
	to: char,
	direction: Direction,
	/// 0 while `from` is shown, 1 once `to` is at rest (it may pass 1 while the spring settles).
	progress: f32,
	/// Whether the roll is still going, so both glyphs stay until it has settled.
	rolling: bool,
}

fn tabular() -> FontFeatures {
	FontFeatures(Arc::new(vec![("tnum".to_string(), 1)]))
}

impl RenderOnce for Ticker {
	fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let color = self.color.unwrap_or(theme.palette.primary_text);
		let reduce_motion = theme.reduce_motion;
		let size = self.size;
		let height = f32::from(size) * LINE;
		let mono = self.mono_width;
		let features = if mono { tabular() } else { FontFeatures::default() };
		let font = Font {
			features: features.clone(),
			..gpui::font(theme.sans())
		};
		let chars: Vec<char> = self.text.chars().collect();

		// Measure every character once per render; the text system caches the glyphs.
		let widths: Vec<f32> = {
			let text_system = window.text_system();
			let font_id = text_system.resolve_font(&font);
			let advance = |ch: char| {
				text_system
					.advance(font_id, size, ch)
					.map_or(f32::from(size) * 0.5, |advance| f32::from(advance.width))
			};
			let digit_width = ('0'..='9').map(&advance).fold(0., f32::max);
			chars
				.iter()
				.map(|&ch| cell_width(ch, digit_width, advance(ch), mono))
				.collect()
		};

		let text = self.text.clone();
		let shown = window.use_keyed_state((self.id.clone(), "shown"), cx, |_, _| Shown {
			text: text.to_string(),
			direction: Direction::Up,
		});
		let (previous, roll) = shown.update(cx, |shown, _| {
			let previous: Vec<char> = shown.text.chars().collect();
			if shown.text.as_str() != text.as_ref() {
				shown.direction = direction(&shown.text, &text);
				shown.text = text.to_string();
			}
			(previous, shown.direction)
		});

		let now = Instant::now();
		let total = chars.len();
		let pairs = align_from_right(&previous, &chars);
		let changed: Vec<bool> = pairs.iter().map(|&(old, new)| old != Some(new)).collect();
		let delays = digit_delays(&changed, DIGIT_STEP, reduce_motion);
		let mut rolling = false;
		let mut frames = Vec::with_capacity(total);
		for (index, ((old, new), width)) in pairs.into_iter().zip(widths).enumerate() {
			// Cells are keyed by their distance from the right, so a unit keeps its roll state
			// when the number grows or shrinks on the left.
			let rank = total - 1 - index;
			let cell = window.use_keyed_state((self.id.clone(), format!("cell-{rank}")), cx, |_, _| Cell {
				from: None,
				to: new,
				direction: Direction::Up,
				changed_at: now,
				delay: 0.,
			});
			let spring_id = (self.id.clone(), format!("roll-{rank}"));
			let changed_here = changed[index];
			let (target, waiting) = cell.update(cx, |cell, _| {
				if changed_here {
					cell.from = old;
					cell.to = new;
					cell.direction = roll;
					cell.changed_at = now;
					cell.delay = delays[index];
				}
				let waiting = now.saturating_duration_since(cell.changed_at).as_secs_f32() < cell.delay;
				(if waiting { 0. } else { 1. }, waiting)
			});
			if changed_here {
				// A new roll starts from the old glyph, whatever the last one was doing.
				motion::value::hold(spring_id.clone(), 0., window, cx);
			}
			let sample = motion::spring(spring_id, target, TICKER, reduce_motion, window, cx);
			let active = sample.active || waiting;
			rolling |= active;
			frames.push(cell.read_with(cx, |cell, _| CellFrame {
				width,
				from: cell.from,
				to: cell.to,
				direction: cell.direction,
				progress: sample.value,
				rolling: active,
			}));
		}
		if rolling {
			window.request_animation_frame();
		}

		let glyphs = Glyphs {
			size,
			height,
			color,
			features,
			family: theme.sans(),
		};
		let cells = frames.into_iter().map(|frame| {
			let cell = div()
				.relative()
				.flex_none()
				.w(px(frame.width))
				.h(px(height))
				.overflow_hidden();
			match frame.from {
				Some(from) if frame.rolling => {
					let slide = slide(frame.progress, frame.direction, height);
					cell.child(glyphs.paint(from, slide.old_y, slide.old_opacity, frame.width))
						.child(glyphs.paint(frame.to, slide.new_y, slide.new_opacity, frame.width))
				}
				_ => cell.child(glyphs.paint(frame.to, 0., 1., frame.width)),
			}
		});

		div()
			.id(self.id)
			.flex()
			.flex_row()
			.flex_none()
			.h(px(height))
			.aria_label(self.text)
			.children(cells)
	}
}

/// The text style shared by every glyph of a ticker.
struct Glyphs {
	size: Pixels,
	height: f32,
	color: Rgba,
	features: FontFeatures,
	family: SharedString,
}

impl Glyphs {
	/// One character, offset `y` from the cell's rest position.
	fn paint(&self, ch: char, y: f32, opacity: f32, width: f32) -> impl IntoElement {
		div()
			.absolute()
			.top(px(y))
			.left_0()
			.w(px(width))
			.h(px(self.height))
			.flex()
			.justify_center()
			.font_family(self.family.clone())
			.font_features(self.features.clone())
			.text_size(self.size)
			.line_height(px(self.height))
			.text_color(self.color)
			.opacity(opacity)
			.child(ch.to_string())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn chars(text: &str) -> Vec<char> {
		text.chars().collect()
	}

	#[test]
	fn cells_align_from_the_right_when_the_length_changes() {
		// "99%" grows to "100%": the % keeps its partner, the new leading 1 has none.
		let pairs = align_from_right(&chars("99%"), &chars("100%"));
		assert_eq!(
			pairs,
			vec![(None, '1'), (Some('9'), '0'), (Some('9'), '0'), (Some('%'), '%')]
		);
	}

	#[test]
	fn a_shorter_text_pairs_the_surviving_cells_on_the_right() {
		let pairs = align_from_right(&chars("1234%"), &chars("99%"));
		assert_eq!(pairs, vec![(Some('3'), '9'), (Some('4'), '9'), (Some('%'), '%')]);
	}

	#[test]
	fn equal_text_pairs_every_cell_with_itself() {
		let text = chars("12:03");
		let pairs = align_from_right(&text, &text);
		assert!(pairs.iter().zip(&text).all(|(pair, &ch)| *pair == (Some(ch), ch)));
	}

	#[test]
	fn an_empty_previous_text_has_no_old_cells() {
		let pairs = align_from_right(&[], &chars("42"));
		assert_eq!(pairs, vec![(None, '4'), (None, '2')]);
	}

	#[test]
	fn a_rising_number_rolls_up() {
		assert_eq!(direction("9", "10"), Direction::Up);
		assert_eq!(direction("40%", "45%"), Direction::Up);
		assert_eq!(direction("1,000", "1,250"), Direction::Up);
	}

	#[test]
	fn a_falling_number_rolls_down() {
		assert_eq!(direction("10", "9"), Direction::Down);
		assert_eq!(direction("45%", "40%"), Direction::Down);
		assert_eq!(direction("1,250", "1,000"), Direction::Down);
	}

	#[test]
	fn text_that_is_not_a_number_rolls_up() {
		assert_eq!(direction("alpha", "omega"), Direction::Up);
		assert_eq!(direction("12", "twelve"), Direction::Up);
		assert_eq!(direction("", "5"), Direction::Up);
		assert_eq!(direction("5", "inf"), Direction::Up);
	}

	#[test]
	fn an_unchanged_number_rolls_up() {
		assert_eq!(direction("7", "7"), Direction::Up);
	}

	#[test]
	fn digits_share_the_widest_width_when_mono() {
		assert_eq!(cell_width('1', 9., 4., true), 9.);
		assert_eq!(cell_width('8', 9., 4., true), 9.);
	}

	#[test]
	fn other_characters_keep_their_natural_width() {
		for ch in ['%', '.', ',', ':', ' '] {
			assert_eq!(cell_width(ch, 9., 4., true), 4., "{ch:?}");
		}
	}

	#[test]
	fn digits_keep_their_natural_width_without_mono() {
		assert_eq!(cell_width('1', 9., 4., false), 4.);
	}

	#[test]
	fn a_roll_starts_on_the_old_glyph_and_ends_on_the_new_one() {
		let start = slide(0., Direction::Up, 20.);
		assert_eq!((start.old_y, start.old_opacity), (0., 1.));
		assert_eq!((start.new_y, start.new_opacity), (20., 0.));

		let end = slide(1., Direction::Up, 20.);
		assert_eq!((end.old_y, end.old_opacity), (-20., 0.));
		assert_eq!((end.new_y, end.new_opacity), (0., 1.));
	}

	#[test]
	fn a_down_roll_moves_the_other_way() {
		let mid = slide(0.5, Direction::Down, 20.);
		assert_eq!(mid.old_y, 10.);
		assert_eq!(mid.new_y, -10.);
		assert_eq!(mid.old_opacity, mid.new_opacity);

		let up = slide(0.5, Direction::Up, 20.);
		assert_eq!(up.old_y, -10.);
		assert_eq!(up.new_y, 10.);
	}

	#[test]
	fn progress_before_the_roll_is_clamped() {
		assert_eq!(slide(-0.5, Direction::Up, 20.), slide(0., Direction::Up, 20.));
	}

	#[test]
	fn a_roll_may_overshoot_a_little_without_leaving_the_opacity_range() {
		let over = slide(1.04, Direction::Up, 20.);
		assert!((over.new_y - -0.8).abs() < 1e-4, "{over:?}");
		assert!((over.old_y - -20.8).abs() < 1e-4);
		assert_eq!((over.old_opacity, over.new_opacity), (0., 1.));
		// A runaway value stays near the cell.
		assert_eq!(
			slide(9., Direction::Up, 20.),
			slide(MAX_OVERSHOOT, Direction::Up, 20.)
		);
	}

	#[test]
	fn changed_digits_start_right_to_left_one_step_apart() {
		let delays = digit_delays(&[true, true, false, true], 0.025, false);
		assert!((delays[3]).abs() < 1e-6);
		assert!((delays[1] - 0.025).abs() < 1e-6);
		assert!((delays[0] - 0.05).abs() < 1e-6);
		assert_eq!(delays[2], 0.);
		assert_eq!(digit_delays(&[false, true], 0.025, false), vec![0., 0.]);
		assert_eq!(digit_delays(&[true, true], 0.025, true), vec![0., 0.]);
	}

	#[test]
	fn numbers_parse_through_separators_and_percent() {
		assert_eq!(parse_number("1,250"), Some(1250.));
		assert_eq!(parse_number(" 45% "), Some(45.));
		assert_eq!(parse_number("-3.5"), Some(-3.5));
		assert_eq!(parse_number("1:30"), None);
		assert_eq!(parse_number("NaN"), None);
	}
}
