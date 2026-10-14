// SPDX-License-Identifier: MPL-2.0

//! # Calcite
//!
//! A glass UI library for [gpui-ce](https://github.com/gpui-ce/gpui-ce): the Liquid Glass
//! looks of macOS 26 and 27, spring motion, and a complete set of controls, forms, menus,
//! panels, modals, and HUD surfaces.
//!
//! ```ignore
//! use calcite::prelude::*;
//!
//! gpui_platform::application().with_assets(Assets::default()).run(|cx| {
//!     calcite::init(cx);
//!     // In a view's render:
//!     // control_bar(status("Ready to play")).action(button("play", "Play").accent().large().icon(Icon::Play))
//! });
//! ```
//!
//! - Every component reads the app [`Theme`] with `cx.theme()`; `.theme(theme)` overrides
//!   it per component.
//! - Builders are free functions named after the component (`button`, `toggle`, `modal`...),
//!   configured with chained methods, like GPUI's own `div()`.
//! - Glass needs the patched gpui-ce with `Filter::Glass`; see the crate README.
//! - Icons come from [Lucide](https://lucide.dev): [`Icon`](icons::Icon) for the components, and
//!   `Lucide` for the full set (default `lucide` feature).

pub mod components;
pub mod fonts;
pub mod icons;
pub mod palette;
pub mod primitives;
pub mod style;
pub mod theme;
pub mod tokens;

// The groups are for navigation only: every module stays at its original path (`calcite::controls`).
pub use components::{
	app_icon, choice, chrome, controls, feedback, forms, hud, layout, menu, notice, overlay, progress, ticker,
};
pub use primitives::{
	attached, continuous, frame_stats, glass, glass_group, interaction, keymap, morph, motion,
};
// Crate-private helpers extracted from components.
pub(crate) use primitives::{knob, surface};

#[cfg(feature = "tint")]
pub use calcite_tint as tint;

pub use glass::{GlassStyle, Transparency};
pub use gpui_ce_elements::editable_text::EditableTextState;
pub use style::{Radii, Style};
pub use theme::{ActiveTheme, Theme, ThemedView};

/// Installs the default theme with the system's Reduce Motion setting, unless the app
/// installed one already, and binds the text editing keys of [`forms::text_field`].
pub fn init(cx: &mut gpui::App) {
	let theme = Theme::from_system(cx);
	init_with(cx, theme);
}

/// Like [`init`], with your own theme, for example one built from a `const` [`Palette`].
/// An existing app theme stays.
///
/// [`Palette`]: palette::Palette
pub fn init_with(cx: &mut gpui::App, theme: Theme) {
	fonts::register(cx);
	if !theme::is_installed(cx) {
		Theme::set_immediately(cx, theme);
	}
	interaction::init_keyboard(cx);
	frame_stats::install(cx);
	use gpui_ce_elements::editable_text::actions::{DEFAULT_INPUT_CONTEXT, default_bindings};
	cx.bind_keys(default_bindings().as_keybindings(Some(DEFAULT_INPUT_CONTEXT)));
}

/// Everything most views need: `use calcite::prelude::*;`.
pub mod prelude {
	#[cfg(feature = "lucide")]
	pub use crate::icons::Lucide;
	pub use crate::{
		ActiveTheme, GlassStyle, Radii, Style, Theme, ThemedView, Transparency,
		attached::{Align, Placement, attached_surface},
		choice::{ChoiceKind, StepState, choice_card, choice_group, step, step_rail, tag},
		chrome::{ChromeStyle, title_bar, window_frame},
		continuous::ContinuousMotion,
		controls::{Size, ThemeOverride, Tone, button, halo, icon_button},
		feedback::{TooltipExt, badge, skeleton},
		forms::{segmented, slider, text_field, text_state, toggle},
		glass::{Elevation, Glass, GlassExt},
		glass_group::glass_group,
		hud::{control_bar, pill, status},
		icons,
		icons::{Assets, Icon, IconSet, icon},
		keymap,
		layout::{
			action_row, danger_zone, document_link, glass_panel, hairline, link, list_row, nav_item,
			nav_rail, page_header, settings_page, settings_panel,
		},
		menu::{action_menu, menu_picker},
		motion::{Curve, EnterExt},
		notice::{ConfirmRole, banner, chip, confirm_dialog, empty_state, progress_bar, spinner},
		overlay::{done_button, floating_action_bar, modal, modal_layer},
		palette::Palette,
		progress::{OutlineMode, progress_outline},
		ticker::ticker,
		tokens,
	};
}

/// The default sans family, the same on every platform: `Style::default().fonts.sans`. Components
/// draw with [`Theme::sans`](theme::Theme::sans), so this reaches only text an app draws outside
/// them, such as the root of a window.
pub const UI_FONT: &str = "Plus Jakarta Sans";

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn ui_font_is_the_default_sans_family() {
		assert_eq!(UI_FONT, style::Fonts::default().sans);
	}
}
