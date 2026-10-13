// SPDX-License-Identifier: MPL-2.0

//! Capsule menus: [`menu_picker`] (`GlassMenuPicker`) and [`action_menu`]
//! (`GlassActionMenu`). The chip uses the compact accent capsule; the open menu is a raised
//! glass panel above everything else, closed by a choice or a click outside it.
//!
//! ```ignore
//! menu_picker("rendering", self.rendering, [(Rendering::Retina, "Retina"), (Rendering::MetalFx, "MetalFX")])
//!     .on_change(cx.listener(|this, value: &Rendering, _, cx| { this.rendering = *value; cx.notify(); }))
//! action_menu("logs", "Logs", Icon::Document)
//!     .item("Show in Finder", Icon::Folder, |_, _| reveal())
//!     .item("Copy Path", Icon::Document, |_, _| copy())
//! ```

use std::rc::Rc;

use gpui::{
	Anchor, App, ElementId, Entity, Filter, FontWeight, IntoElement, ParentElement, Role, SharedString,
	Styled, Window, anchored, deferred, div, point, prelude::*, px,
};

use crate::{
	controls::theme_override,
	glass::{Elevation, GlassExt},
	icons::{Icon, IconRef},
	interaction::{FocusShape, KeyHandler, KeyRole, KeyboardFocus, focus_handle, register_key_target},
	theme::{ActiveTheme, Theme},
	tokens::{self, control, radius, scale_alpha, white, with_alpha},
};

type Action = Rc<dyn Fn(&mut Window, &mut App) + 'static>;

struct Item {
	label: SharedString,
	icon: Option<IconRef>,
	checked: bool,
	divider_before: bool,
	action: Action,
}

/// The open state of a menu, kept across frames. While the menu is open, Escape closes it
/// before anything under it.
fn open_state(id: &ElementId, window: &mut Window, cx: &mut App) -> Entity<bool> {
	let open = window.use_keyed_state((id.clone(), "calcite-menu"), cx, |_, _| false);
	if *open.read(cx) {
		let close = open.clone();
		let dismiss: KeyHandler = Rc::new(move |_, cx| {
			close.update(cx, |open, cx| {
				*open = false;
				cx.notify();
			})
		});
		register_key_target(id, KeyRole::Dismiss, Some(dismiss), None, window, cx);
	}
	open
}

/// The compact accent capsule of Settings actions and menu pickers.
fn chip(
	id: ElementId,
	theme: &Theme,
	enabled: bool,
	content: impl IntoElement,
	window: &mut Window,
	cx: &mut App,
) -> gpui::Stateful<gpui::Div> {
	let focus = focus_handle(&id, window, cx);
	let foreground = if enabled {
		tokens::contrast_tint(theme.accent)
	} else {
		with_alpha(theme.accent, control::DISABLED_FOREGROUND_OPACITY)
	};
	let glass = theme
		.surface(Some(with_alpha(
			theme.accent,
			control::ENABLED_CHROMATIC_SURFACE_OPACITY,
		)))
		.elevation(Elevation::None);
	let chip = div()
		.id(id)
		.relative()
		.flex()
		.flex_row()
		.items_center()
		.gap(px(6.))
		.px(px(control::COMPACT_HORIZONTAL_PADDING))
		.py(px(control::COMPACT_VERTICAL_PADDING))
		.rounded_full()
		.text_size(tokens::text::CAPTION)
		.font_weight(FontWeight::SEMIBOLD)
		.text_color(foreground)
		.border_1()
		.border_color(with_alpha(
			theme.accent,
			control::ENABLED_CHROMATIC_BORDER_OPACITY,
		))
		.child(content)
		.child(Icon::ChevronUpDown.render(px(9.), foreground));
	let chip = chip.when(enabled, |chip| {
		chip.keyboard_focus(&focus, FocusShape::Capsule, theme, window)
	});
	match (enabled, glass.filter()) {
		(true, Some(filter)) => chip.backdrop_filter(vec![Filter::Glass(filter)]).cursor_pointer(),
		(true, None) => chip.bg(glass.fill()).cursor_pointer(),
		(false, _) => chip.bg(white(control::DISABLED_CHROMATIC_SURFACE_OPACITY)),
	}
}

fn menu_panel(id: ElementId, theme: &Theme, open: Entity<bool>, items: Vec<Item>) -> impl IntoElement {
	let accent = tokens::contrast_tint(theme.accent);
	let close = open.clone();
	deferred(
		anchored()
			.anchor(Anchor::TopLeft)
			.offset(point(px(0.), px(30.)))
			.snap_to_window_with_margin(px(8.))
			.child(
				div()
					.id((id.clone(), "panel"))
					.min_w(px(180.))
					.p(px(5.))
					.flex()
					.flex_col()
					.gap(px(1.))
					.rounded(radius::PANEL)
					.rounded_smoothing_ios()
					.glass(theme.hud_surface().elevation(Elevation::Raised))
					.border_1()
					.border_color(scale_alpha(theme.palette.hairline, control::HUD_BORDER_OPACITY))
					.text_size(px(12.))
					.text_color(theme.palette.primary_text)
					.role(Role::Menu)
					.on_mouse_down_out(move |_, _, cx| {
						close.update(cx, |open, cx| {
							*open = false;
							cx.notify();
						})
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
						let row = div()
							.id((id.clone(), SharedString::from(index.to_string())))
							.h(px(26.))
							.px(px(8.))
							.flex()
							.flex_row()
							.items_center()
							.gap(px(7.))
							.rounded(radius::CONTROL)
							.cursor_pointer()
							.role(Role::MenuItem)
							.aria_label(item.label.clone())
							.aria_selected(item.checked)
							.hover(|style| style.bg(white(0.08)))
							.child(div().w(px(12.)).when(item.checked, |this| {
								this.child(Icon::Checkmark.render(px(10.), accent))
							}))
							.when_some(item.icon, |row, icon| {
								row.child(icon.render(px(12.), theme.palette.secondary_text))
							})
							.child(div().flex_1().child(item.label))
							.on_click(move |_, window, cx| {
								open.update(cx, |open, cx| {
									*open = false;
									cx.notify();
								});
								action(window, cx);
							})
							.into_any_element();
						divider.into_iter().chain(std::iter::once(row))
					})),
			),
	)
	.priority(1)
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

	pub fn disabled(mut self, disabled: bool) -> Self {
		self.enabled = !disabled;
		self
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
		let open = open_state(&self.id, window, cx);
		let is_open = *open.read(cx);
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
					divider_before: self.divider_before.contains(&value),
					action: Rc::new(move |window, cx| {
						if let Some(on_change) = &on_change {
							on_change(&value, window, cx);
						}
					}),
				}
			})
			.collect();
		let toggle = open.clone();
		div()
			.relative()
			.child(
				chip(
					self.id.clone(),
					&theme,
					self.enabled,
					div().child(title.clone()),
					window,
					cx,
				)
				.role(Role::ComboBox)
				.aria_expanded(is_open)
				.aria_label(title)
				.when(self.enabled, |chip| {
					chip.on_click(move |_, _, cx| {
						toggle.update(cx, |open, cx| {
							*open = !*open;
							cx.notify();
						})
					})
				}),
			)
			.when(is_open, |this| {
				this.child(menu_panel(self.id, &theme, open, items))
			})
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
			divider_before: true,
			action: Rc::new(|_, _| {}),
		});
		self
	}

	pub fn disabled(mut self, disabled: bool) -> Self {
		self.enabled = !disabled;
		self
	}
}

impl RenderOnce for ActionMenu {
	fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
		let theme = self.theme.unwrap_or_else(|| cx.theme());
		let open = open_state(&self.id, window, cx);
		let is_open = *open.read(cx);
		let foreground = tokens::contrast_tint(theme.accent);
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
		let toggle = open.clone();
		div()
			.relative()
			.child(
				chip(
					self.id.clone(),
					&theme,
					self.enabled,
					div()
						.flex()
						.flex_row()
						.items_center()
						.gap(px(5.))
						.child(self.icon.render(px(10.), foreground))
						.child(self.title.clone()),
					window,
					cx,
				)
				.role(Role::Button)
				.aria_expanded(is_open)
				.aria_label(self.title)
				.when(self.enabled, |chip| {
					chip.on_click(move |_, _, cx| {
						toggle.update(cx, |open, cx| {
							*open = !*open;
							cx.notify();
						})
					})
				}),
			)
			.when(is_open, |this| {
				this.child(menu_panel(self.id, &theme, open, items))
			})
	}
}

theme_override!(ActionMenu);
