// SPDX-License-Identifier: MPL-2.0

//! The compact accent capsule that is the menu's trigger, and the glass of the chip and the menu.

use gpui::{App, ElementId, Filter, IntoElement, ParentElement, Rgba, Styled, Window, prelude::*};

use crate::{
	glass::Elevation,
	icons::Icon,
	interaction::{FocusShape, KeyboardFocus, focus_handle},
	surface::chip_capsule,
	theme::Theme,
	tokens::{self, Chromatic, control, with_alpha},
};

/// The label color of the chip.
pub(super) fn chip_foreground(theme: &Theme, enabled: bool) -> Rgba {
	if enabled {
		theme.legible(theme.accent)
	} else {
		with_alpha(theme.accent, control::DISABLED_FOREGROUND_OPACITY)
	}
}

/// The glass of the chip: a flat accent capsule.
pub(super) fn chip_glass(theme: &Theme) -> crate::glass::Glass {
	theme
		.surface(Some(Chromatic::enabled(theme.accent).fill))
		.elevation(Elevation::None)
}

/// The glass of the open menu: raised HUD glass.
pub(super) fn menu_glass(theme: &Theme) -> crate::glass::Glass {
	theme.hud_surface().elevation(Elevation::Raised)
}

/// The compact accent capsule of Settings actions and menu pickers. It is the real trigger and
/// stays in place at every `rim`: only its own glass and border fade with `rim`, which is 0 while
/// the open menu's fused surface supplies them. The label, chevron, size, and focus never change.
#[allow(clippy::too_many_arguments)]
pub(super) fn chip(
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
