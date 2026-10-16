// SPDX-License-Identifier: MPL-2.0

//! The option rows of an open menu and the cascade that fades them in and out.

use std::time::Instant;

use gpui::{
	AnyElement, App, ElementId, Entity, FocusHandle, IntoElement, ParentElement, Role, SharedString, Styled,
	Toggled, Window, div, prelude::*, px,
};

use crate::{
	icons::Icon,
	interaction::{ClickableRow, FocusShape, focus_handle, reduce_motion},
	morph,
	motion::{
		self, secondary,
		tokens::{EXIT, SNAPPY, STAGGER_IN},
	},
	theme::Theme,
};

use super::state::{Item, MenuFocus, close_when_focus_leaves, row_for_key, row_id, set_open};

/// How far a row slides while it fades in or out, in px.
pub(super) const ROW_SLIDE: f32 = 4.;

/// Opacity and downward offset (px) of a row at cascade progress `p` (0 hidden, 1 shown). The
/// slide follows the raw spring value, so a little overshoot settles; the opacity never exceeds 1.
pub(super) fn row_pose(p: f32) -> (f32, f32) {
	(p.clamp(0., 1.), ROW_SLIDE * (1. - p))
}

/// What a row remembers between frames to start `delay` seconds after the menu flips.
pub(super) struct Cascade {
	open: bool,
	changed: Instant,
	/// The target the row's spring follows right now; it only moves once the delay has passed.
	effective: f32,
}

/// The cascade of row `index`: it fades and slides in 4 px, each row a stagger step after the
/// previous one (closing is twice as tight and uses the quicker [`EXIT`] spring). Returns the
/// row's `(opacity, offset)`. Layout never changes: the offset is a paint-time shift.
pub(super) fn row_cascade(
	row_id: &ElementId,
	index: usize,
	open: bool,
	reduced: bool,
	window: &mut Window,
	cx: &mut App,
) -> (f32, f32) {
	let reduced = reduced || reduce_motion(cx);
	let now = Instant::now();
	let cell = window.use_keyed_state((row_id.clone(), "cascade"), cx, |_, _| Cascade {
		open,
		changed: now,
		effective: 0.,
	});
	let delay = secondary::stagger_delay(index, open, STAGGER_IN, reduced);
	let (target, waiting) = cell.update(cx, |cell, _| {
		if cell.open != open {
			cell.open = open;
			cell.changed = now;
		}
		let elapsed = now.saturating_duration_since(cell.changed).as_secs_f32();
		let waiting = elapsed < delay;
		if !waiting {
			cell.effective = if open { 1. } else { 0. };
		}
		(cell.effective, waiting)
	});
	if waiting && !reduced {
		window.request_animation_frame();
	}
	let spring = if open { SNAPPY } else { EXIT };
	let sample = motion::spring_from(
		(row_id.clone(), "cascade-spring"),
		0.,
		target,
		spring,
		reduced,
		window,
		cx,
	);
	row_pose(sample.value)
}

/// The option rows, as wide as the menu body they sit in.
#[allow(clippy::too_many_arguments)]
pub(super) fn menu_rows(
	id: &ElementId,
	theme: &Theme,
	open: &Entity<bool>,
	focus_pending: &Entity<bool>,
	menu_focus: &Entity<MenuFocus>,
	enabled: bool,
	items: Vec<Item>,
	window: &mut Window,
	cx: &mut App,
) -> AnyElement {
	let accent = theme.legible(theme.accent);
	let is_open = *open.read(cx) && enabled;
	let handles: Vec<FocusHandle> = (0..items.len())
		.map(|index| focus_handle(&row_id(id, index), window, cx))
		.collect();
	menu_focus.update(cx, |focus, _| focus.rows = handles.clone());
	for (index, handle) in handles.iter().enumerate() {
		close_when_focus_leaves(row_id(id, index), handle, open, menu_focus, window, cx);
	}
	let labels: Vec<SharedString> = items.iter().map(|item| item.label.clone()).collect();
	// Opened from the keyboard: focus lands once the rows are in the dispatch tree, after this frame.
	if is_open && *focus_pending.read(cx) {
		focus_pending.update(cx, |pending, _| *pending = false);
		let target = items.iter().position(|item| item.checked).unwrap_or(0);
		if let Some(handle) = handles.get(target).cloned() {
			window.defer(cx, move |window, cx| window.focus(&handle, cx));
		}
	}
	let key_handles = handles.clone();
	div()
		.w_full()
		.p(px(morph::PANEL_PADDING + morph::BORDER))
		.flex()
		.flex_col()
		.gap(px(morph::ROW_GAP))
		.font_family(theme.sans())
		.text_size(theme.text_supporting())
		.text_color(theme.palette.primary_text)
		.on_key_down(move |event, window, cx| {
			let stroke = &event.keystroke;
			if stroke.modifiers.modified() {
				return;
			}
			let current = key_handles.iter().position(|handle| handle.is_focused(window));
			if let Some(target) = row_for_key(&stroke.key, stroke.key_char.as_deref(), &labels, current) {
				window.focus(&key_handles[target], cx);
				cx.stop_propagation();
			}
		})
		.children(items.into_iter().enumerate().flat_map(|(index, item)| {
			let open = open.clone();
			let divider = item.divider_before.then(|| {
				div()
					.h(px(1.))
					.mx(px(6.))
					.my(px(3.))
					.bg(theme.palette.hairline)
					.into_any_element()
			});
			let action = item.action.clone();
			let row_id = row_id(id, index);
			let menu_focus = menu_focus.clone();
			let (opacity, slide) = row_cascade(&row_id, index, is_open, theme.reduce_motion, window, cx);
			let clickable = ClickableRow::new(&row_id, theme, window, cx);
			let row = div()
				.id(row_id)
				.h(px(morph::ROW_HEIGHT))
				.px(px(8.))
				.flex()
				.flex_row()
				.items_center()
				.gap(px(7.))
				.rounded(theme.style.radii.row)
				.bg(clickable.fill(theme, 0.))
				.role(if item.choice {
					Role::MenuItemRadio
				} else {
					Role::MenuItem
				})
				.aria_label(item.label.clone())
				.when(item.choice, |row| {
					row.aria_toggled(if item.checked {
						Toggled::True
					} else {
						Toggled::False
					})
				});
			let row = clickable
				.wire(row, FocusShape::Rounded(theme.style.radii.row), theme, window)
				.child(div().w(px(12.)).when(item.checked, |this| {
					this.child(Icon::Checkmark.render(theme.pt(10.), accent))
				}))
				.when_some(item.icon, |row, icon| {
					row.child(icon.render(theme.pt(12.), theme.palette.secondary_text))
				})
				.child(div().flex_1().whitespace_nowrap().child(item.label))
				.on_click(move |event, window, cx| {
					set_open(&open, Some(false), cx);
					if event.is_keyboard()
						&& let Some(trigger) = menu_focus.read(cx).trigger.clone()
					{
						window.focus(&trigger, cx);
					}
					action(window, cx);
				})
				.relative()
				.top(px(slide))
				.opacity(opacity)
				.into_any_element();
			divider.into_iter().chain(std::iter::once(row))
		}))
		.into_any_element()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn rows_slide_in_four_px_while_fading() {
		assert_eq!(row_pose(0.), (0., 4.));
		assert_eq!(row_pose(1.), (1., 0.));
		let (opacity, slide) = row_pose(0.5);
		assert!((opacity - 0.5).abs() < 1e-6 && (slide - 2.).abs() < 1e-6);
	}

	#[test]
	fn overshoot_never_raises_opacity_past_one() {
		let (opacity, slide) = row_pose(1.02);
		assert_eq!(opacity, 1.);
		assert!(slide < 0. && slide > -0.2);
		assert_eq!(row_pose(-0.1).0, 0.);
	}

	#[test]
	fn rows_cascade_by_the_stagger_token_and_close_tighter() {
		let open = |i| secondary::stagger_delay(i, true, STAGGER_IN, false);
		let close = |i| secondary::stagger_delay(i, false, STAGGER_IN, false);
		assert_eq!(open(0), 0.);
		assert!((open(3) - 0.072).abs() < 1e-6);
		assert!(close(3) < open(3));
		assert_eq!(open(8), open(30));
		assert_eq!(secondary::stagger_delay(5, true, STAGGER_IN, true), 0.);
	}
}
