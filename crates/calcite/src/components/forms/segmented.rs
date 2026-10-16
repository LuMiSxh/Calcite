// SPDX-License-Identifier: MPL-2.0

//! The segmented control: [`segmented`] (`AdaptiveSegmentedControl`). One accent pill slides
//! under the selected cell.

use gpui::{
	App, Bounds, ElementId, Entity, Filter, FontWeight, IntoElement, ParentElement, Pixels, Role,
	SharedString, Styled, Toggled, Window, canvas, div, prelude::*, px,
};

use super::SharedChangeHandler;
use crate::{
	glass::Elevation,
	interaction::{
		ControlInteraction, ControlState, Feedback, FocusShape, KeyboardFocus, arrow_direction, focus_handle,
	},
	motion::{self, secondary, tokens::SELECT},
	theme::{ActiveTheme, Theme},
	tokens::{self, Chromatic, control, with_alpha},
};

impl<T: Clone + PartialEq + 'static> Segmented<T> {
	/// The accessible name of the group; each segment is named by its own label.
	pub fn label(mut self, label: impl Into<SharedString>) -> Self {
		self.label = Some(label.into());
		self
	}

	/// Whether the control responds. Enabled by default.
	pub fn enabled(mut self, enabled: bool) -> Self {
		self.enabled = enabled;
		self
	}

	/// The inverse of `enabled`.
	pub fn disabled(self, disabled: bool) -> Self {
		self.enabled(!disabled)
	}

	pub fn on_change(mut self, handler: impl Fn(&T, &mut Window, &mut App) + 'static) -> Self {
		self.on_change = Some(std::rc::Rc::new(handler));
		self
	}

	pub fn theme(mut self, theme: Theme) -> Self {
		self.theme = Some(theme);
		self
	}
}

impl<T: Clone + PartialEq + 'static> RenderOnce for Segmented<T> {
	fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let focus = focus_handle(&self.id, window, cx);
		let reduced = theme.reduce_transparency();
		let container_fill = if reduced {
			theme.palette.opaque_surface
		} else {
			theme.ink(control::ENABLED_NEUTRAL_SURFACE_OPACITY)
		};
		let chromatic = Chromatic::enabled(theme.accent);
		let selected_glass = theme.surface(Some(chromatic.fill)).elevation(Elevation::None);
		let enabled = self.enabled;
		let id = self.id.clone();
		let count = self.options.len();
		let selected_index = self.options.iter().position(|(value, _)| *value == self.selected);
		let values: Vec<T> = self.options.iter().map(|(value, _)| value.clone()).collect();
		let keys = self.on_change.clone().filter(|_| enabled);
		// One glass indicator slides between the segments, like a macOS segmented control.
		// It needs the laid-out width, so the first frame styles the selected segment itself.
		let bounds = window.use_keyed_state((id.clone(), "calcite-segmented-bounds"), cx, |_, _| {
			None::<Bounds<Pixels>>
		});
		let probe = bounds.clone();
		// Each cell reports its own laid-out bounds, so the indicator sits exactly behind a label
		// whatever its width: cells size to their content, never to an assumed equal share.
		let cell_bounds = window.use_keyed_state((id.clone(), "calcite-segmented-cells"), cx, |_, _| {
			Vec::<Option<Bounds<Pixels>>>::new()
		});
		let cells: Option<Vec<(f32, f32)>> = bounds.read(cx).and_then(|container| {
			let reported = cell_bounds.read(cx);
			(reported.len() == count)
				.then(|| {
					reported
						.iter()
						.map(|cell| {
							cell.map(|cell| {
								(
									f32::from(cell.origin.x - container.origin.x) - SEGMENTED_BORDER,
									f32::from(cell.size.width),
								)
							})
						})
						.collect::<Option<Vec<_>>>()
				})
				.flatten()
		});
		let segment_states: Vec<Entity<ControlState>> = (0..count)
			.map(|index| {
				let segment_id: ElementId = (id.clone(), SharedString::from(index.to_string())).into();
				ControlState::of(&segment_id, window, cx)
			})
			.collect();
		let slide = motion::spring(
			(id.clone(), "calcite-segmented-position"),
			selected_index.unwrap_or(0) as f32,
			SELECT,
			theme.reduce_motion,
			window,
			cx,
		);
		let position = slide.value;
		// The indicator rims briefly as it lands on its new segment.
		let rim = motion::value::landing(
			(id.clone(), "calcite-segmented-landing"),
			selected_index.unwrap_or(0) as f32,
			SELECT,
			theme.reduce_motion,
			window,
			cx,
		);
		let indicator = cells
			.as_deref()
			.filter(|_| selected_index.is_some() && count > 0)
			.and_then(|cells| pill_rect(cells, position))
			.map(|(left, width)| {
				let (left, indicator_width) = lead_edge(left, width, slide.velocity, theme.reduce_motion);
				let indicator = div()
					.absolute()
					.top(px(SEGMENTED_PADDING))
					.bottom(px(SEGMENTED_PADDING))
					.left(px(left))
					.w(px(indicator_width))
					.rounded_full()
					.border_1()
					.border_color(tokens::mix(chromatic.border, theme.ink(1.), rim));
				match selected_glass.filter() {
					Some(filter) => indicator.backdrop_filter(vec![Filter::Glass(filter)]),
					None => indicator.bg(selected_glass.fill()),
				}
			});
		let slides = indicator.is_some();
		div()
			.font_family(theme.sans())
			.id(self.id)
			.relative()
			.child(
				canvas(
					move |bounds, _, cx| probe.update(cx, |stored, _| *stored = Some(bounds)),
					|_, _, _, _| {},
				)
				.absolute()
				.size_full(),
			)
			.children(indicator)
			// Equal-width cells: every column is as wide as the widest label, so the capsule never
			// favours one option and the selection pill is the same size wherever it rests.
			.grid()
			.grid_cols_min_content(count.max(1) as u16)
			.gap(px(SEGMENTED_GAP))
			.p(px(SEGMENTED_PADDING))
			.rounded_full()
			.bg(container_fill)
			.border_1()
			.border_color(with_alpha(
				theme.palette.control,
				control::ENABLED_NEUTRAL_BORDER_OPACITY,
			))
			.role(Role::RadioGroup)
			.when_some(self.label.clone(), |group, label| group.aria_label(label))
			.aria_disabled(!enabled)
			// A fast exit can skip a cell's own hover event; leaving the whole control clears all.
			.clear_hover_on_leave(segment_states)
			.when(enabled, |this| {
				this.keyboard_focus(&focus, FocusShape::Capsule, &theme, window)
			})
			// Like a macOS segmented control, the arrow keys move the selection itself.
			.when_some(keys, move |this, on_change| {
				this.on_key_down(move |event, window, cx| {
					let Some(direction) = arrow_direction(event) else {
						return;
					};
					cx.stop_propagation();
					let next = match selected_index {
						Some(index) => index.saturating_add_signed(direction as isize).min(count - 1),
						None => 0,
					};
					if Some(next) != selected_index
						&& let Some(value) = values.get(next)
					{
						on_change(value, window, cx);
					}
				})
			})
			.children(
				self.options
					.into_iter()
					.enumerate()
					.map(|(index, (value, label))| {
						let selected = value == self.selected;
						let segment_id: ElementId =
							(id.clone(), SharedString::from(index.to_string())).into();
						let segment_state = ControlState::of(&segment_id, window, cx);
						let segment_frame = ControlState::frame(
							&segment_state,
							Feedback::PressOnly,
							enabled,
							theme.reduce_motion,
							window,
							cx,
						);
						let on_change = self.on_change.clone();
						let foreground = if !enabled {
							with_alpha(theme.accent, control::DISABLED_FOREGROUND_OPACITY)
						} else if selected {
							theme.palette.primary_text
						} else {
							theme.palette.secondary_text
						};
						let segment = div()
							.id(segment_id)
							.flex()
							.items_center()
							.justify_center()
							.text_center()
							.whitespace_nowrap()
							.px(theme.pt(10.))
							.py(theme.pt(4.))
							.rounded_full()
							.text_size(theme.text_body())
							.font_weight(FontWeight::SEMIBOLD)
							.text_color(foreground)
							.role(Role::RadioButton)
							.aria_label(label.clone())
							.aria_selected(selected)
							.aria_toggled(if selected { Toggled::True } else { Toggled::False })
							.aria_position_in_set(index + 1)
							.aria_size_of_set(count)
							.child(
								canvas(
									{
										let cell_bounds = cell_bounds.clone();
										move |bounds, _, cx| {
											cell_bounds.update(cx, |cells, _| {
												if cells.len() <= index {
													cells.resize(index + 1, None);
												}
												cells[index] = Some(bounds);
											})
										}
									},
									|_, _, _, _| {},
								)
								.absolute()
								.size_full(),
							)
							.child(label);
						let segment = if selected && slides {
							segment
						} else if selected {
							let segment = segment.border_1().border_color(chromatic.border);
							match selected_glass.filter() {
								Some(filter) => segment.backdrop_filter(vec![Filter::Glass(filter)]),
								None => segment.bg(selected_glass.fill()),
							}
						} else {
							segment
								.bg(segment_frame.hover_fill(theme.ink(1.)))
								.control_state(&segment_state, false)
						};
						segment.when_some(
							on_change.filter(|_| enabled && !selected),
							move |segment, on_change| {
								segment.on_click(move |_, window, cx| on_change(&value, window, cx))
							},
						)
					}),
			)
	}
}

/// A row of mutually exclusive options; the selection carries the accent glass.
#[derive(IntoElement)]
pub struct Segmented<T: Clone + PartialEq + 'static> {
	id: ElementId,
	selected: T,
	options: Vec<(T, SharedString)>,
	enabled: bool,
	label: Option<SharedString>,
	theme: Option<Theme>,
	on_change: Option<SharedChangeHandler<T>>,
}

/// A segmented control over `options`, showing `selected`.
pub fn segmented<T: Clone + PartialEq + 'static, L: Into<SharedString>>(
	id: impl Into<ElementId>,
	selected: T,
	options: impl IntoIterator<Item = (T, L)>,
) -> Segmented<T> {
	Segmented {
		id: id.into(),
		selected,
		options: options
			.into_iter()
			.map(|(value, label)| (value, label.into()))
			.collect(),
		enabled: true,
		label: None,
		theme: None,
		on_change: None,
	}
}

/// The indicator's `(left, width)` at a fractional `position` between the laid-out `cells`
/// (`(left, width)` each, relative to the control's inner edge): exactly a cell's rect at a whole
/// position, a blend of the two neighbours between them. `None` without cells.
fn pill_rect(cells: &[(f32, f32)], position: f32) -> Option<(f32, f32)> {
	let last = cells.len().checked_sub(1)?;
	let position = position.clamp(0., last as f32);
	let from = (position.floor() as usize).min(last);
	let to = (from + 1).min(last);
	let t = position - from as f32;
	let lerp = |a: f32, b: f32| a + (b - a) * t;
	Some((lerp(cells[from].0, cells[to].0), lerp(cells[from].1, cells[to].1)))
}

/// The indicator of a segmented control while it moves at `velocity` segments per second: the edge
/// it moves toward stretches by half of the knob stretch ([`secondary::stretch`]) and the trailing
/// edge stays, so it reads as a drop pulled along. At rest it is `(left, width)` unchanged.
fn lead_edge(left: f32, width: f32, velocity: f32, reduced: bool) -> (f32, f32) {
	let extra = width * (secondary::stretch(velocity, reduced).0 - 1.) * 0.5;
	if velocity < 0. {
		(left - extra, width + extra)
	} else {
		(left, width + extra)
	}
}

/// Inner padding and spacing of a segmented control. The border sits outside the padding.
const SEGMENTED_PADDING: f32 = 3.;

const SEGMENTED_GAP: f32 = 0.;

const SEGMENTED_BORDER: f32 = 1.;

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn the_indicator_is_exactly_the_selected_cell_for_any_cell_widths() {
		let layouts: [&[(f32, f32)]; 3] = [
			&[(4., 60.), (68., 60.)],
			&[(4., 70.), (78., 80.), (162., 70.)],
			&[(4., 44.), (52., 180.), (236., 36.)],
		];
		for cells in layouts {
			for (index, cell) in cells.iter().enumerate() {
				let (left, width) = pill_rect(cells, index as f32).unwrap();
				assert_eq!((left, width), *cell);
				// The label is centered in its cell, so its centre is the indicator's centre.
				assert!((left + width / 2. - (cell.0 + cell.1 / 2.)).abs() < 1e-4);
			}
		}
	}

	#[test]
	fn the_indicator_blends_between_neighbouring_cells_and_clamps() {
		let cells = [(4., 40.), (48., 100.)];
		assert_eq!(pill_rect(&cells, 0.5), Some((26., 70.)));
		assert_eq!(pill_rect(&cells, -1.), Some((4., 40.)));
		assert_eq!(pill_rect(&cells, 5.), Some((48., 100.)));
		assert_eq!(pill_rect(&[], 0.), None);
	}

	#[test]
	fn indicator_is_unchanged_at_rest_and_under_reduce_motion() {
		assert_eq!(lead_edge(10., 80., 0., false), (10., 80.));
		assert_eq!(lead_edge(10., 80., 8., true), (10., 80.));
	}

	#[test]
	fn indicator_stretches_toward_the_edge_it_moves_to() {
		// Full speed: the knob stretch is 22%, half of it on the lead edge.
		let (left, width) = lead_edge(10., 100., 8., false);
		assert_eq!(left, 10.);
		assert!((width - 111.).abs() < 1e-3);
		let (left, width) = lead_edge(10., 100., -8., false);
		assert!((left - -1.).abs() < 1e-3 && (width - 111.).abs() < 1e-3);
		// The trailing edge stays put either way.
		assert!((left + width - 110.).abs() < 1e-3);
	}
}
