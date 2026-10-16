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

mod chip;
mod rows;
mod state;

use std::rc::Rc;

use chip::{chip, chip_foreground, chip_glass, menu_glass};
use rows::menu_rows;
use state::{Item, MenuFocus, close_when_focus_leaves, open_state, set_open, set_pending};

use gpui::{
	AnyElement, App, ElementId, IntoElement, ParentElement, Rgba, Role, SharedString, Styled, Window, div,
	prelude::*, px,
};

use crate::{
	attached::{Align, AttachedSurface, Placement, attached_surface},
	controls::theme_override,
	icons::IconRef,
	interaction::focus_handle,
	morph::{self, RowSpec},
	theme::{ActiveTheme, Theme},
	tokens::Chromatic,
};

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
