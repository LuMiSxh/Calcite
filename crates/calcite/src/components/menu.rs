// SPDX-License-Identifier: MPL-2.0

//! Capsule menus: [`menu_picker`] (`GlassMenuPicker`) and [`action_menu`]
//! (`GlassActionMenu`). The chip uses the compact accent capsule and always stays a real element
//! in its place. The open menu is a second glass surface attached to the chip's bottom edge that
//! morphs out of it and fuses with it (see [`crate::attached`]); one spring drives its rect, corner
//! radius, and the colour from the chip's to the menu's. It grows downward, or upward when the
//! window has no room below, and closes by a choice, Escape, or a click outside it.
//!
//! From the keyboard, Space, Return, Up, and Down on the chip open the menu and move focus to the
//! checked row (or the first). Up, Down, Home, and End move between rows, a letter jumps to the
//! next row that starts with it, Return and Space choose, and Escape closes and returns focus to
//! the chip.
//!
//! ```ignore
//! menu_picker("rendering", self.rendering, [(Rendering::Retina, "Retina"), (Rendering::MetalFx, "MetalFX")])
//!     .on_change(cx.listener(|this, value: &Rendering, _, cx| { this.rendering = *value; cx.notify(); }))
//! action_menu("logs", "Logs", Icon::Document)
//!     .item("Show in Finder", Icon::Folder, |_, _| reveal())
//!     .item("Copy Path", Icon::Document, |_, _| copy())
//! ```

use std::{rc::Rc, time::Instant};

use gpui::{
	AnyElement, App, ElementId, Entity, Filter, FocusHandle, IntoElement, ParentElement, Rgba, Role,
	SharedString, Styled, Toggled, Window, div, prelude::*, px,
};

use crate::{
	attached::{Align, AttachedSurface, Placement, attached_surface},
	controls::theme_override,
	glass::Elevation,
	icons::{Icon, IconRef},
	interaction::{
		ClickableRow, FocusShape, KeyHandler, KeyRole, KeyboardFocus, focus_handle, reduce_motion,
		register_key_target,
	},
	morph::{self, RowSpec},
	motion::{
		self, secondary,
		tokens::{EXIT, SNAPPY, STAGGER_IN},
	},
	surface::chip_capsule,
	theme::{ActiveTheme, Theme},
	tokens::{self, Chromatic, control, with_alpha},
};

type Action = Rc<dyn Fn(&mut Window, &mut App) + 'static>;

struct Item {
	label: SharedString,
	icon: Option<IconRef>,
	checked: bool,
	/// Whether the row is one choice of a set (a picker option) rather than an action.
	choice: bool,
	divider_before: bool,
	action: Action,
}

/// The focus handles of a menu's chip and rows, written while they render (each inside its own
/// element scope, where their keyed state lives) and read by the key handlers.
#[derive(Default)]
struct MenuFocus {
	trigger: Option<FocusHandle>,
	rows: Vec<FocusHandle>,
}

/// The open state of a menu, kept across frames. While the menu is open, Escape closes it
/// before anything under it, and returns focus to the chip if a row had it.
fn open_state(
	id: &ElementId,
	handles: &Entity<MenuFocus>,
	window: &mut Window,
	cx: &mut App,
) -> Entity<bool> {
	let open = window.use_keyed_state((id.clone(), "calcite-menu"), cx, |_, _| false);
	if *open.read(cx) {
		let close = open.clone();
		let handles = handles.clone();
		let dismiss: KeyHandler = Rc::new(move |window, cx| {
			let (trigger, in_rows) = {
				let handles = handles.read(cx);
				(
					handles.trigger.clone(),
					handles.rows.iter().any(|row| row.is_focused(window)),
				)
			};
			if let Some(trigger) = trigger.filter(|_| in_rows) {
				window.focus(&trigger, cx);
			}
			close.update(cx, |open, cx| {
				*open = false;
				cx.notify();
			})
		});
		register_key_target(id, KeyRole::Dismiss, Some(dismiss), None, window, cx);
	}
	open
}

fn set_open(open: &Entity<bool>, value: Option<bool>, cx: &mut App) {
	open.update(cx, |open, cx| {
		*open = value.unwrap_or(!*open);
		cx.notify();
	});
}

/// Closes the open menu when focus leaves the chip and all of its rows (Tab past the last row, or
/// a click on another control). Registered once per handle, kept in keyed state beside it.
fn close_when_focus_leaves(
	key: ElementId,
	handle: &FocusHandle,
	open: &Entity<bool>,
	menu_focus: &Entity<MenuFocus>,
	window: &mut Window,
	cx: &mut App,
) {
	window.use_keyed_state((key, "calcite-menu-leave"), cx, |window, cx| {
		let (open, menu_focus) = (open.clone(), menu_focus.clone());
		window.on_focus_out(handle, cx, move |_, window, cx| {
			if !*open.read(cx) {
				return;
			}
			// No focus at all (window deactivated, field blurred): leave the menu as it is.
			if window.focused(cx).is_none() {
				return;
			}
			let inside = {
				let focus = menu_focus.read(cx);
				focus
					.trigger
					.iter()
					.chain(focus.rows.iter())
					.any(|h| h.is_focused(window))
			};
			if !inside {
				set_open(&open, Some(false), cx);
			}
		})
	});
}

/// Asks the open menu to move focus onto a row once its rows exist (opened from the keyboard).
fn set_pending(pending: &Entity<bool>, cx: &mut App) {
	pending.update(cx, |pending, cx| {
		*pending = true;
		cx.notify();
	});
}

/// The element id of row `index` of the menu `id`.
fn row_id(id: &ElementId, index: usize) -> ElementId {
	(id.clone(), SharedString::from(index.to_string())).into()
}

/// The row a menu key moves focus to. `current` is the focused row, if any; `key_char` is the
/// text the key types. Arrow keys wrap, Home and End jump, and a letter selects the next row
/// (after `current`, wrapping) whose label starts with it, ignoring case.
fn row_for_key(
	key: &str,
	key_char: Option<&str>,
	labels: &[SharedString],
	current: Option<usize>,
) -> Option<usize> {
	let count = labels.len();
	if count == 0 {
		return None;
	}
	match key {
		"down" => Some(current.map_or(0, |index| (index + 1) % count)),
		"up" => Some(current.map_or(count - 1, |index| (index + count - 1) % count)),
		"home" => Some(0),
		"end" => Some(count - 1),
		_ => {
			let typed = key_char?
				.chars()
				.next()
				.filter(|c| !c.is_whitespace() && !c.is_control())?;
			let starts = |index: &usize| {
				labels[*index]
					.chars()
					.next()
					.is_some_and(|first| first.to_lowercase().eq(typed.to_lowercase()))
			};
			let from = current.map_or(0, |index| index + 1);
			(0..count).map(|offset| (from + offset) % count).find(starts)
		}
	}
}

/// The label color of the chip.
fn chip_foreground(theme: &Theme, enabled: bool) -> Rgba {
	if enabled {
		theme.legible(theme.accent)
	} else {
		with_alpha(theme.accent, control::DISABLED_FOREGROUND_OPACITY)
	}
}

/// The glass of the chip: a flat accent capsule.
fn chip_glass(theme: &Theme) -> crate::glass::Glass {
	theme
		.surface(Some(Chromatic::enabled(theme.accent).fill))
		.elevation(Elevation::None)
}

/// The glass of the open menu: raised HUD glass.
fn menu_glass(theme: &Theme) -> crate::glass::Glass {
	theme.hud_surface().elevation(Elevation::Raised)
}

/// The compact accent capsule of Settings actions and menu pickers. It is the real trigger and
/// stays in place at every `rim`: only its own glass and border fade with `rim`, which is 0 while
/// the open menu's fused surface supplies them. The label, chevron, size, and focus never change.
#[allow(clippy::too_many_arguments)]
fn chip(
	id: ElementId,
	theme: &Theme,
	enabled: bool,
	rim: f32,
	foreground: Rgba,
	content: impl IntoElement,
	window: &mut Window,
	cx: &mut App,
) -> gpui::Stateful<gpui::Div> {
	let focus = focus_handle(&id, window, cx);
	let glass = chip_glass(theme);
	let chip = chip_capsule(id)
		.font_family(theme.sans())
		.text_color(foreground)
		.border_1()
		.border_color(tokens::scale_alpha(Chromatic::enabled(theme.accent).border, rim))
		.child(content)
		.child(Icon::ChevronUpDown.render(theme.pt(9.), foreground));
	let chip = chip.when(enabled, |chip| {
		chip.keyboard_focus(&focus, FocusShape::Capsule, theme, window)
	});
	match (enabled, glass.filter()) {
		(true, Some(_)) if rim <= 0. => chip,
		(true, Some(filter)) => chip.backdrop_filter(vec![Filter::Glass(filter)]),
		(true, None) => chip.bg(tokens::scale_alpha(glass.fill(), rim)),
		(false, _) => chip.bg(Chromatic::disabled_with(theme.accent, theme.palette.ink).fill),
	}
}

/// The shared body of both menus: the real chip, with the options attached to its bottom edge.
struct MenuView {
	id: ElementId,
	theme: Theme,
	enabled: bool,
	role: Role,
	label: SharedString,
	items: Vec<Item>,
	/// Builds the chip's label for a foreground color.
	content: Rc<dyn Fn(Rgba) -> AnyElement>,
}

/// How far a row slides while it fades in or out, in px.
const ROW_SLIDE: f32 = 4.;

/// Opacity and downward offset (px) of a row at cascade progress `p` (0 hidden, 1 shown). The
/// slide follows the raw spring value, so a little overshoot settles; the opacity never exceeds 1.
fn row_pose(p: f32) -> (f32, f32) {
	(p.clamp(0., 1.), ROW_SLIDE * (1. - p))
}

/// What a row remembers between frames to start `delay` seconds after the menu flips.
struct Cascade {
	open: bool,
	changed: Instant,
	/// The target the row's spring follows right now; it only moves once the delay has passed.
	effective: f32,
}

/// The cascade of row `index`: it fades and slides in 4 px, each row a stagger step after the
/// previous one (closing is twice as tight and uses the quicker [`EXIT`] spring). Returns the
/// row's `(opacity, offset)`. Layout never changes: the offset is a paint-time shift.
fn row_cascade(
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
fn menu_rows(
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

impl MenuView {
	fn render(self, window: &mut Window, cx: &mut App) -> AttachedSurface {
		let Self {
			id,
			theme,
			enabled,
			role,
			label,
			items,
			content,
		} = self;
		let menu_focus = window.use_keyed_state((id.clone(), "calcite-menu-handles"), cx, |_, _| {
			MenuFocus::default()
		});
		let open = open_state(&id, &menu_focus, window, cx);
		let focus_pending = window.use_keyed_state((id.clone(), "calcite-menu-focus"), cx, |_, _| false);
		let is_open = *open.read(cx);
		let specs: Vec<RowSpec> = items
			.iter()
			.map(|item| RowSpec {
				label: morph::text_width(&item.label),
				icon: item.icon.is_some(),
				divider_before: item.divider_before,
			})
			.collect();
		// The surface widens the body to the chip's width if the chip is wider.
		let menu = morph::menu_size(0., &specs);
		let radius: f32 = theme.style.radii.panel.into();
		let toggle = open.clone();
		let leave_open = open.clone();
		let close = open.clone();
		let rows_id = id.clone();
		let rows_theme = theme;
		let rows_open = open.clone();
		let rows_pending = focus_pending.clone();
		let rows_focus = menu_focus.clone();
		let pending = focus_pending.clone();

		attached_surface(id.clone())
			.theme(theme)
			.open(is_open && enabled)
			.placement(Placement::Auto)
			.align(Align::Start)
			.trigger_look(chip_glass(&theme), Chromatic::enabled(theme.accent).border)
			.body_look(menu_glass(&theme), theme.hud_border())
			.body_role(Role::Menu)
			.on_dismiss(move |_, cx| set_open(&close, Some(false), cx))
			.trigger(move |state, window, cx| {
				// The label keeps the chip's colour: the trigger stays itself while the body opens.
				let foreground = chip_foreground(&theme, enabled);
				let handle = focus_handle(&id, window, cx);
				close_when_focus_leaves(id.clone(), &handle, &leave_open, &menu_focus, window, cx);
				menu_focus.update(cx, |focus, _| focus.trigger = Some(handle));
				chip(
					id.clone(),
					&theme,
					enabled,
					state.rim,
					foreground,
					content(foreground),
					window,
					cx,
				)
				.role(role)
				.aria_expanded(is_open)
				.aria_label(label)
				.when(enabled, |chip| {
					let key_toggle = toggle.clone();
					let key_pending = pending.clone();
					chip.on_click(move |event, _, cx| {
						let opening = !*toggle.read(cx);
						set_open(&toggle, None, cx);
						if opening && event.is_keyboard() {
							set_pending(&pending, cx);
						}
					})
					// Down and Up open the menu, or step into it when a click opened it.
					.on_key_down(move |event, _, cx| {
						let stroke = &event.keystroke;
						if !stroke.modifiers.modified() && matches!(stroke.key.as_str(), "down" | "up") {
							set_open(&key_toggle, Some(true), cx);
							set_pending(&key_pending, cx);
							cx.stop_propagation();
						}
					})
				})
				.into_any_element()
			})
			.body(menu, radius, move |window, cx| {
				menu_rows(
					&rows_id,
					&rows_theme,
					&rows_open,
					&rows_pending,
					&rows_focus,
					enabled,
					items,
					window,
					cx,
				)
			})
	}
}

/// A capsule that picks one of `options`.
#[derive(IntoElement)]
pub struct MenuPicker<T: Clone + PartialEq + 'static> {
	id: ElementId,
	selected: T,
	options: Vec<(T, SharedString)>,
	divider_before: Vec<T>,
	enabled: bool,
	theme: Option<Theme>,
	on_change: Option<crate::forms::SharedChangeHandler<T>>,
}

/// A menu picker over `options`, showing the title of `selected`.
pub fn menu_picker<T: Clone + PartialEq + 'static, L: Into<SharedString>>(
	id: impl Into<ElementId>,
	selected: T,
	options: impl IntoIterator<Item = (T, L)>,
) -> MenuPicker<T> {
	MenuPicker {
		id: id.into(),
		selected,
		options: options
			.into_iter()
			.map(|(value, label)| (value, label.into()))
			.collect(),
		divider_before: Vec::new(),
		enabled: true,
		theme: None,
		on_change: None,
	}
}

impl<T: Clone + PartialEq + 'static> MenuPicker<T> {
	/// Draws a divider above `value`, to set a group of choices apart.
	pub fn divider_before(mut self, value: T) -> Self {
		self.divider_before.push(value);
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
		self.on_change = Some(Rc::new(handler));
		self
	}

	pub fn theme(mut self, theme: Theme) -> Self {
		self.theme = Some(theme);
		self
	}
}

impl<T: Clone + PartialEq + 'static> RenderOnce for MenuPicker<T> {
	fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let title = self
			.options
			.iter()
			.find(|(value, _)| *value == self.selected)
			.map(|(_, label)| label.clone())
			.unwrap_or_default();
		let items = self
			.options
			.iter()
			.map(|(value, label)| {
				let on_change = self.on_change.clone();
				let value = value.clone();
				Item {
					label: label.clone(),
					icon: None,
					checked: value == self.selected,
					choice: true,
					divider_before: self.divider_before.contains(&value),
					action: Rc::new(move |window, cx| {
						if let Some(on_change) = &on_change {
							on_change(&value, window, cx);
						}
					}),
				}
			})
			.collect();
		let shown = title.clone();
		MenuView {
			id: self.id,
			theme,
			enabled: self.enabled,
			role: Role::ComboBox,
			label: title,
			items,
			content: Rc::new(move |_| div().child(shown.clone()).into_any_element()),
		}
		.render(window, cx)
	}
}

/// A capsule that opens a list of actions.
#[derive(IntoElement)]
pub struct ActionMenu {
	id: ElementId,
	title: SharedString,
	icon: IconRef,
	items: Vec<Item>,
	enabled: bool,
	theme: Option<Theme>,
}

/// An action menu with a title and an icon on its chip.
pub fn action_menu(
	id: impl Into<ElementId>,
	title: impl Into<SharedString>,
	icon: impl Into<IconRef>,
) -> ActionMenu {
	ActionMenu {
		id: id.into(),
		title: title.into(),
		icon: icon.into(),
		items: Vec::new(),
		enabled: true,
		theme: None,
	}
}

impl ActionMenu {
	/// Adds an action.
	pub fn item(
		mut self,
		label: impl Into<SharedString>,
		icon: impl Into<IconRef>,
		action: impl Fn(&mut Window, &mut App) + 'static,
	) -> Self {
		self.items.push(Item {
			label: label.into(),
			icon: Some(icon.into()),
			checked: false,
			choice: false,
			divider_before: false,
			action: Rc::new(action),
		});
		self
	}

	/// Adds a divider before the next item.
	pub fn divider(mut self) -> Self {
		self.items.push(Item {
			label: SharedString::default(),
			icon: None,
			checked: false,
			choice: false,
			divider_before: true,
			action: Rc::new(|_, _| {}),
		});
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
}

impl RenderOnce for ActionMenu {
	fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		// A divider entry carries no label: merge it into the next item.
		let mut items: Vec<Item> = Vec::new();
		let mut pending_divider = false;
		for item in self.items {
			if item.label.is_empty() && item.divider_before {
				pending_divider = true;
				continue;
			}
			items.push(Item {
				divider_before: item.divider_before || pending_divider,
				..item
			});
			pending_divider = false;
		}
		let title = self.title.clone();
		let icon = self.icon;
		MenuView {
			id: self.id,
			theme,
			enabled: self.enabled,
			role: Role::Button,
			label: self.title,
			items,
			content: Rc::new(move |foreground| {
				div()
					.flex()
					.flex_row()
					.items_center()
					.gap(px(5.))
					.child(icon.render(theme.pt(10.), foreground))
					.child(title.clone())
					.into_any_element()
			}),
		}
		.render(window, cx)
	}
}

theme_override!(ActionMenu);

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

	fn labels() -> Vec<SharedString> {
		["Alpha", "beta", "Bravo", "Gamma"]
			.map(SharedString::from)
			.to_vec()
	}

	#[test]
	fn arrows_wrap_and_start_from_the_ends() {
		let labels = labels();
		assert_eq!(row_for_key("down", None, &labels, None), Some(0));
		assert_eq!(row_for_key("up", None, &labels, None), Some(3));
		assert_eq!(row_for_key("down", None, &labels, Some(3)), Some(0));
		assert_eq!(row_for_key("up", None, &labels, Some(0)), Some(3));
		assert_eq!(row_for_key("home", None, &labels, Some(2)), Some(0));
		assert_eq!(row_for_key("end", None, &labels, Some(0)), Some(3));
		assert_eq!(row_for_key("down", None, &[], None), None);
	}

	#[test]
	fn a_letter_jumps_to_the_next_row_that_starts_with_it() {
		let labels = labels();
		assert_eq!(row_for_key("g", Some("g"), &labels, Some(0)), Some(3));
		// Case does not matter, and repeated presses cycle through the matches.
		assert_eq!(row_for_key("b", Some("b"), &labels, Some(0)), Some(1));
		assert_eq!(row_for_key("b", Some("b"), &labels, Some(1)), Some(2));
		assert_eq!(row_for_key("b", Some("b"), &labels, Some(2)), Some(1));
		assert_eq!(row_for_key("b", Some("B"), &labels, None), Some(1));
		assert_eq!(row_for_key("z", Some("z"), &labels, Some(0)), None);
		assert_eq!(row_for_key("space", Some(" "), &labels, Some(0)), None);
		assert_eq!(row_for_key("enter", None, &labels, Some(0)), None);
	}
}
