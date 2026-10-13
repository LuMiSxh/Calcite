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
//! - Icons come from [Lucide](https://lucide.dev): [`Icon`] for the components, and
//!   `Lucide` for the full set (default `lucide` feature).

pub mod app_icon;
pub mod choice;
pub mod controls;
pub mod feedback;
pub mod forms;
pub mod glass;
pub mod hud;
pub mod icons;
pub mod interaction;
mod keymap;
pub mod layout;
pub mod menu;
pub mod motion;
pub mod notice;
pub mod overlay;
pub mod palette;
pub mod progress;
pub mod theme;
pub mod tokens;

#[cfg(feature = "tint")]
pub use calcite_tint as tint;

pub use glass::{GlassStyle, Transparency};
pub use gpui_ce_elements::editable_text::EditableTextState;
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
	if !theme::is_installed(cx) {
		Theme::set_immediately(cx, theme);
	}
	interaction::init_keyboard(cx);
	use gpui_ce_elements::editable_text::actions::{DEFAULT_INPUT_CONTEXT, default_bindings};
	cx.bind_keys(default_bindings().as_keybindings(Some(DEFAULT_INPUT_CONTEXT)));
}

/// Everything most views need: `use calcite::prelude::*;`.
pub mod prelude {
	#[cfg(feature = "lucide")]
	pub use crate::icons::Lucide;
	pub use crate::{
		ActiveTheme, GlassStyle, Theme, ThemedView, Transparency,
		choice::{ChoiceKind, StepState, choice_card, choice_group, step, step_rail, tag},
		controls::{Size, ThemeOverride, Tone, button, halo, icon_button},
		feedback::{TooltipExt, badge, skeleton},
		forms::{segmented, slider, text_field, text_state, toggle},
		glass::{Elevation, Glass, GlassExt},
		hud::{control_bar, pill, status},
		icons,
		icons::{Assets, Icon, IconSet, icon},
		keymap,
		layout::{
			PanelTone, action_row, danger_zone, document_link, glass_panel, hairline, link, list_row,
			nav_item, nav_rail, page_header, settings_page, settings_panel,
		},
		menu::{action_menu, menu_picker},
		motion::Curve,
		notice::{BannerTone, ConfirmRole, banner, chip, confirm_dialog, empty_state, progress_bar, spinner},
		overlay::{done_button, floating_action_bar, modal, modal_layer},
		palette::Palette,
		progress::OutlineMode,
		tokens,
	};
}

/// The platform UI font. Inter replaces it outside Apple platforms.
#[cfg(target_os = "macos")]
pub const UI_FONT: &str = ".SystemUIFont";
#[cfg(not(target_os = "macos"))]
pub const UI_FONT: &str = "Inter";
