// SPDX-License-Identifier: MPL-2.0

//! State matrices: a labelled row per variant, a column per state. The library has no way to
//! force hover, press or focus, so those columns hold a live component and the header says what
//! to do ("move here", "hold", "Tab here").

use calcite::{
	forms::{Slider, Toggle},
	prelude::*,
};
use gpui::{
	AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, prelude::*, px,
};

use super::{
	Display, Preview,
	sections::{label, source_line},
};

/// A caption under a component.
pub fn cell(caption: impl Into<SharedString>, element: impl IntoElement) -> AnyElement {
	div()
		.flex()
		.flex_col()
		.items_start()
		.gap(px(6.))
		.child(element)
		.child(label(caption))
		.into_any_element()
}

/// The left column of a row: the title, a note, and the Rust call.
fn row_title(title: &'static str, note: &'static str, source: &'static str) -> impl IntoElement {
	div()
		.w(px(190.))
		.flex_none()
		.flex()
		.flex_col()
		.gap(px(2.))
		.child(
			div()
				.text_size(px(12.))
				.font_weight(FontWeight::SEMIBOLD)
				.child(title),
		)
		.when(!note.is_empty(), |this| this.child(label(note)))
		.child(
			div()
				.pt(px(4.))
				.flex()
				.flex_col()
				.children(chain_lines(source).into_iter().map(source_line)),
		)
}

/// A call chain split before each `.method(..)` at the top level, so a long call wraps at its
/// links instead of overflowing the label column. The first line says `Source:`.
fn chain_lines(source: &str) -> Vec<String> {
	let mut lines = vec![String::from("Source: ")];
	let mut depth = 0i32;
	let mut previous = ' ';
	for ch in source.chars() {
		match ch {
			'(' | '[' => depth += 1,
			')' | ']' => depth -= 1,
			_ => {}
		}
		if ch == '.' && depth == 0 && previous == ')' {
			lines.push(String::new());
		}
		lines.last_mut().expect("never empty").push(ch);
		previous = ch;
	}
	lines
}

/// One component, one row: the title, a note and the Rust call on the left, the variant cells
/// on the right. The call is the copyable usage of the row's components.
pub fn state_row(
	title: &'static str,
	note: &'static str,
	source: &'static str,
	cells: Vec<AnyElement>,
) -> AnyElement {
	div()
		.flex()
		.flex_row()
		.items_start()
		.gap(px(16.))
		.child(row_title(title, note, source))
		.child(
			div()
				.flex_1()
				.flex()
				.flex_row()
				.flex_wrap()
				.items_start()
				.gap_x(px(20.))
				.gap_y(px(12.))
				.children(cells),
		)
		.into_any_element()
}

/// A row of a [`matrix`]: one variant, one element per column.
pub struct MatrixRow {
	title: &'static str,
	source: &'static str,
	cells: Vec<AnyElement>,
}

pub fn matrix_row(title: &'static str, source: &'static str, cells: Vec<AnyElement>) -> MatrixRow {
	MatrixRow { title, source, cells }
}

/// Variants down the side, states across the top, every column `width` wide.
pub fn matrix(width: f32, columns: &[&'static str], rows: Vec<MatrixRow>) -> AnyElement {
	let column = move |element: AnyElement| div().w(px(width)).flex_none().child(element);
	div()
		.flex()
		.flex_col()
		.gap(px(12.))
		.child(
			div()
				.flex()
				.flex_row()
				.gap(px(16.))
				.child(div().w(px(190.)).flex_none())
				.child(
					div()
						.flex()
						.flex_row()
						.gap_x(px(12.))
						.children(columns.iter().map(|name| column(label(*name).into_any_element()))),
				),
		)
		.children(rows.into_iter().map(move |row| {
			div()
				.flex()
				.flex_row()
				.items_center()
				.gap(px(16.))
				.child(row_title(row.title, "", row.source))
				.child(
					div()
						.flex()
						.flex_row()
						.items_center()
						.gap_x(px(12.))
						.children(row.cells.into_iter().map(column)),
				)
		}))
		.into_any_element()
}

/// The rows stacked.
pub fn stack(rows: Vec<AnyElement>) -> impl IntoElement {
	div().flex().flex_col().gap(px(20.)).children(rows)
}

/// The tones a button offers, success included as a palette color.
pub fn tones(theme: &Theme) -> [(&'static str, Tone); 5] {
	[
		("Neutral", Tone::Neutral),
		("Accent", Tone::Accent),
		("Success", Tone::Color(theme.palette.success)),
		("Warning", Tone::Warning),
		("Danger", Tone::Danger),
	]
}

/// A section's matrix header: what the library cannot force, said once.
const LIVE: &str = "hover, press and focus are real: use the pointer and Tab";

impl Preview {
	pub fn actions_section(&self, theme: &Theme) -> impl IntoElement + use<> {
		let tones = tones(theme);
		let id = |prefix: &str, name: &str| SharedString::from(format!("{prefix}-{name}"));
		let buttons = tones
			.iter()
			.map(|(name, tone)| {
				let (name, tone) = (*name, *tone);
				let make = move |prefix: &str| button(id(prefix, name), name).tone(tone);
				matrix_row(
					name,
					"button(id, label).tone(tone)",
					vec![
						make("rest").into_any_element(),
						make("hover").into_any_element(),
						make("press").into_any_element(),
						make("focus").into_any_element(),
						make("disabled").disabled(true).into_any_element(),
						make("large").large().into_any_element(),
						make("compact").compact().into_any_element(),
						make("icon").icon(Lucide::Download).into_any_element(),
					],
				)
			})
			.collect();
		let icon_buttons = matrix(
			118.,
			&[
				"Rest",
				"Hover: move here",
				"Pressed: hold",
				"Focused: Tab here",
				"Diameter 32",
				"Diameter 52",
			],
			vec![matrix_row(
				"icon_button",
				"icon_button(id, icon, label).diameter(px(40.))",
				vec![
					icon_button("ib-rest", Icon::Gear, "Settings").into_any_element(),
					icon_button("ib-hover", Lucide::Bell, "Notifications").into_any_element(),
					icon_button("ib-press", Lucide::Rocket, "Launch").into_any_element(),
					icon_button("ib-focus", Lucide::Search, "Search").into_any_element(),
					icon_button("ib-small", Lucide::Bell, "Notifications")
						.diameter(px(32.))
						.into_any_element(),
					icon_button("ib-large", Lucide::Rocket, "Launch")
						.diameter(px(52.))
						.into_any_element(),
				],
			)],
		);
		stack(vec![
			label(format!(
				"Columns are states. Rest is static; {LIVE}. The default pressed look scales to 0.95."
			))
			.into_any_element(),
			matrix(
				108.,
				&[
					"Rest",
					"Hover: move here",
					"Pressed: hold",
					"Focused: Tab here",
					"Disabled",
					"Large",
					"Compact",
					"With icon",
				],
				buttons,
			),
			state_row(
				"No glow",
				"turns the pointer light off",
				"button(id, label).tone(tone).glow(false)",
				vec![cell("glow(false)", button("glow-off", "Neutral").glow(false))],
			),
			state_row(
				"Long label",
				"the button sizes to its text",
				"button(id, label).accent()",
				vec![
					cell(
						"long label",
						button("long-label", "Export the full diagnostic archive for this server").accent(),
					),
					cell("short label", button("short-label", "Export").accent()),
					cell(
						"long, compact",
						button("long-compact", "Export the full diagnostic archive")
							.compact()
							.icon(Lucide::Download),
					),
				],
			),
			state_row(
				"Loading",
				"no loading state: disable and add a spinner",
				"button(id, label).disabled(true).icon(Lucide::LoaderCircle) + spinner()",
				vec![cell(
					"disabled + spinner",
					div()
						.flex()
						.flex_row()
						.items_center()
						.gap(px(8.))
						.child(
							button("busy", "Verifying…")
								.accent()
								.disabled(true)
								.icon(Lucide::LoaderCircle),
						)
						.child(spinner().id("busy-spinner")),
				)],
			),
			state_row(
				"Default action",
				"Return presses it when no focused control takes Return",
				"button(id, label).accent().default_action()",
				vec![cell(
					"default_action()",
					button("default-action", "Continue").accent().default_action(),
				)],
			),
			icon_buttons,
			state_row(
				"Specials",
				"halo · done_button · glass_tooltip",
				"halo(button(..)) · done_button(id) · div().id(id).glass_tooltip(text)",
				vec![
					cell(
						"halo",
						halo(button("halo-play", "Play").accent().large().icon(Icon::Play)),
					),
					cell("done_button", done_button("done")),
					cell(
						"done_button + icon",
						done_button("done-icon").icon(Icon::ArrowDown),
					),
					cell(
						"glass_tooltip: hover",
						div()
							.id("tooltip-demo")
							.glass_tooltip("A glass tooltip")
							.child(button("tooltip-button", "Hover me")),
					),
				],
			),
		])
	}

	pub fn forms_section(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
		let switch = |index: usize, id: String| {
			toggle(SharedString::from(id), self.toggles[index]).on_change(cx.listener(
				move |this, on: &bool, _, cx| {
					this.toggles[index] = *on;
					cx.notify();
				},
			))
		};
		// One row per size; `a` and `b` are the live switches the Off and On columns drive.
		let toggle_row =
			|name: &'static str, source: &'static str, size: fn(Toggle) -> Toggle, a: usize, b: usize| {
				let id = move |column: &str| format!("toggle-{name}-{column}");
				matrix_row(
					name,
					source,
					vec![
						size(switch(a, id("off"))).into_any_element(),
						size(switch(b, id("on"))).into_any_element(),
						size(switch(a, id("hover"))).into_any_element(),
						size(switch(a, id("press"))).into_any_element(),
						size(switch(a, id("focus"))).into_any_element(),
						size(toggle(SharedString::from(id("d-off")), false).disabled(true))
							.into_any_element(),
						size(toggle(SharedString::from(id("d-on")), true).disabled(true)).into_any_element(),
					],
				)
			};
		let displays = [
			(Display::Windowed, "Windowed"),
			(Display::Borderless, "Borderless"),
			(Display::Fullscreen, "Fullscreen"),
		];
		let display_change = cx.listener(|this, display: &Display, _, cx| {
			this.display = *display;
			cx.notify();
		});
		let two = [(0usize, "On"), (1, "Off")];
		let long = [(0usize, "Short"), (1, "A much longer segment label"), (2, "Mid")];
		let segment_change = |slot: usize| {
			cx.listener(move |this, value: &usize, _, cx| {
				this.segments[slot] = *value;
				cx.notify();
			})
		};
		let slider_cells =
			|name: &'static str, source: &'static str, decorate: &dyn Fn(Slider) -> Slider| -> MatrixRow {
				let make = |column: &str, disabled: bool| {
					let id = SharedString::from(format!("slider-{name}-{column}"));
					decorate(
						slider(id, self.volume)
							.step(0.05)
							.width(px(160.))
							.disabled(disabled)
							.on_change(cx.listener(|this, value: &f32, _, cx| {
								this.volume = *value;
								cx.notify();
							})),
					)
				};
				matrix_row(
					name,
					source,
					vec![
						make("rest", false).into_any_element(),
						make("drag", false).into_any_element(),
						make("focus", false).into_any_element(),
						make("disabled", true).into_any_element(),
					],
				)
			};
		let invalid = banner("tf-error-banner", "Enter at least 3 characters").tone(Tone::Danger);
		stack(vec![
			label(format!(
				"Columns are states; {LIVE}. A press or drag on the Pressed column shows the pressed look."
			))
			.into_any_element(),
			matrix(
				100.,
				&[
					"Off",
					"On",
					"Hover: move here",
					"Pressed: hold, drag",
					"Focused: Tab here",
					"Disabled off",
					"Disabled on",
				],
				vec![
					toggle_row("Regular", "toggle(id, on)", |toggle| toggle, 0, 1),
					toggle_row("Large", "toggle(id, on).large()", Toggle::large, 2, 3),
					toggle_row("Compact", "toggle(id, on).compact()", Toggle::compact, 4, 5),
				],
			),
			state_row(
				"Toggle · label",
				"the label follows the switch; glow(false) turns the pointer light off",
				"toggle(id, on).label(text).glow(false)",
				vec![
					cell(
						"label",
						switch(0, "toggle-label".into()).label("Automatic updates"),
					),
					cell(
						"label, disabled",
						toggle("toggle-label-d", true)
							.label("Locked by policy")
							.disabled(true),
					),
					cell(
						"glow(false)",
						switch(1, "toggle-noglow".into()).label("Flat").glow(false),
					),
				],
			),
			matrix(
				226.,
				&["Rest", "Drag the knob", "Focused: Tab, arrow keys", "Disabled"],
				vec![
					slider_cells("Plain", "slider(id, value).step(0.05)", &|slider| slider),
					slider_cells(
						"Value label",
						"slider(id, value).label(text).value_label(true)",
						&|slider| slider.label("Volume").value_label(true),
					),
					slider_cells("Ticks", "slider(id, value).step(0.1).ticks(11)", &|slider| {
						slider.step(0.1).ticks(11)
					}),
					slider_cells(
						"Icons",
						"slider(id, value).leading_icon(..).trailing_icon(..)",
						&|slider| {
							slider
								.leading_icon(Lucide::Volume1)
								.trailing_icon(Lucide::Volume2)
						},
					),
					slider_cells("Range", "slider(id, value).range(0.0..=1.0)", &|slider| {
						slider.range(0.0..=1.0)
					}),
				],
			),
			// Segmented controls size to their labels, so each state is a captioned cell that wraps.
			state_row(
				"2 options",
				"",
				"segmented(id, value, [(value, label)])",
				vec![
					cell(
						"rest: hover, press",
						segmented("seg-two-rest", self.segments[0], two).on_change(segment_change(0)),
					),
					cell("other segment selected", segmented("seg-two-other", 1usize, two)),
					cell(
						"focused: Tab, arrows",
						segmented("seg-two-focus", self.segments[0], two).on_change(segment_change(0)),
					),
					cell(
						"disabled",
						segmented("seg-two-disabled", 0usize, two).disabled(true),
					),
				],
			),
			state_row(
				"3 options",
				"",
				"segmented(id, value, [(value, label)])",
				vec![
					cell(
						"rest: hover, press",
						segmented("seg-three-rest", self.display, displays).on_change(display_change),
					),
					cell(
						"other segment selected",
						segmented("seg-three-other", Display::Fullscreen, displays),
					),
					cell(
						"focused: Tab, arrows",
						segmented("seg-three-focus", Display::Borderless, displays),
					),
					cell(
						"disabled",
						segmented("seg-three-disabled", Display::Borderless, displays).disabled(true),
					),
				],
			),
			state_row(
				"Long labels",
				"",
				"segmented(id, value, [(value, label)])",
				vec![
					cell(
						"rest: hover, press",
						segmented("seg-long-rest", self.segments[1], long).on_change(segment_change(1)),
					),
					cell(
						"other segment selected",
						segmented("seg-long-other", 2usize, long),
					),
					cell("focused: Tab, arrows", segmented("seg-long-focus", 0usize, long)),
					cell(
						"disabled",
						segmented("seg-long-disabled", 1usize, long).disabled(true),
					),
				],
			),
			state_row(
				"Text field",
				"click to focus and type; no disabled state; pair with a danger banner for an error",
				"text_field(id, &state).placeholder(text).icon(Lucide::Search).width(px(240.))",
				vec![
					cell(
						"empty + icon",
						text_field("tf-empty", &self.search)
							.placeholder("Search operators")
							.icon(Lucide::Search)
							.width(px(240.)),
					),
					cell(
						"empty, no icon",
						text_field("tf-plain", &self.plain)
							.placeholder("Plain placeholder")
							.width(px(240.)),
					),
					cell(
						"filled",
						text_field("tf-filled", &self.filled)
							.icon(Lucide::Pencil)
							.width(px(240.)),
					),
					cell(
						"long text, scrolls",
						text_field("tf-long", &self.long)
							.icon(Lucide::FileText)
							.width(px(240.)),
					),
					cell(
						"error: field + banner",
						div()
							.w(px(300.))
							.flex()
							.flex_col()
							.gap(px(8.))
							.child(
								text_field("tf-error", &self.invalid)
									.icon(Lucide::Pencil)
									.width(px(300.)),
							)
							.child(invalid),
					),
				],
			),
		])
	}
}
