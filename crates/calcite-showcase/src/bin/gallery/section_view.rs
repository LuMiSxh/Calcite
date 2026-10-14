// SPDX-License-Identifier: MPL-2.0

//! Each gallery section is its own view, so an animation inside one section re-renders and
//! re-lays-out only that section. The page re-lays-out the others only when it changes.
//!
//! A cached view is laid out from a definite style instead of measured from its content, so a
//! section measures itself: a canvas inside it records the height it came out at, and the page
//! caches the section at that height. A change in the measured height (a settings page with
//! more rows) or in the window width (text wraps differently) drops the section back to
//! uncached for a frame, until it has been measured again.
//!
//! What the cache cannot see, it is told about: a section observes the [`Gallery`] state and
//! re-renders when it changes; controls and animations inside it notify the section itself;
//! and a theme change, Reduce Motion, or a window resize refreshes the whole window, which
//! ignores every cache.

use std::hash::{DefaultHasher, Hash, Hasher};

use calcite::prelude::*;
use gpui::{
	AnyElement, AppContext as _, Context, Entity, EntityId, IntoElement, ParentElement, Pixels, Render,
	SharedString, Styled, WeakEntity, Window, canvas, div, px,
};

use super::{Gallery, sections};

/// A section is re-measured when its height moves by more than this.
const TOLERANCE: Pixels = px(0.5);

#[derive(Clone, Copy)]
pub enum Kind {
	Glass,
	Style,
	Actions,
	Glow,
	Forms,
	Menus,
	Selection,
	Settings,
	Hud,
	Morph,
	Overlay,
	Setup,
	Notices,
	Chrome,
	Icons,
	Tokens,
	Typography,
	Motion,
	MotionLab,
}

impl Kind {
	pub const ALL: [Self; 19] = [
		Self::Glass,
		Self::Style,
		Self::Actions,
		Self::Glow,
		Self::Forms,
		Self::Menus,
		Self::Selection,
		Self::Settings,
		Self::Hud,
		Self::Morph,
		Self::Overlay,
		Self::Setup,
		Self::Notices,
		Self::Chrome,
		Self::Icons,
		Self::Tokens,
		Self::Typography,
		Self::Motion,
		Self::MotionLab,
	];

	fn title(self) -> &'static str {
		match self {
			Self::Glass => "Glass styles",
			Self::Style => "Style",
			Self::Actions => "Actions",
			Self::Glow => "Cursor glow",
			Self::Forms => "Forms",
			Self::Menus => "Menu morph",
			Self::Selection => "Selection and navigation",
			Self::Settings => "Settings",
			Self::Hud => "HUD",
			Self::Morph => "Pill morph",
			Self::Overlay => "Overlay and feedback",
			Self::Setup => "Setup assistant",
			Self::Notices => "Notices",
			Self::Chrome => "Window chrome",
			Self::Icons => "Icons",
			Self::Tokens => "Tokens",
			Self::Typography => "Typography",
			Self::Motion => "Motion",
			Self::MotionLab => "Motion lab",
		}
	}

	fn note(self, theme: &Theme) -> SharedString {
		match self {
			Self::Glass => format!(
				"G cycles the glass · A the accent · P the palette · current: {}",
				theme.style.glass.label()
			)
			.into(),
			Self::Style => "Theme::style live: radii · glass strength · glass style · pointer glow".into(),
			Self::Actions => {
				"button · icon_button · halo · done_button · glass_tooltip, by tone, size and state".into()
			}
			Self::Glow => "move the cursor over it: glow(true) next to glow(false)".into(),
			Self::Forms => "toggle · slider · segmented · text_field, by state".into(),
			Self::Menus => "menu_picker · action_menu: the menu morphs out of its trigger".into(),
			Self::Selection => {
				"choice_card · chip · nav_item · list_row · link · document_link · step · badge · tag".into()
			}
			Self::Settings => "nav_rail · page_header · settings_panel · action_row · danger_zone".into(),
			Self::Hud => "pill · control_bar · status · list_row".into(),
			Self::Morph => "pill · glass_group · O toggles the pill".into(),
			Self::Overlay => "modal · floating_action_bar · link · badge · skeleton".into(),
			Self::Setup => "step_rail · step · choice_group · choice_card · tag".into(),
			Self::Notices => "banner · spinner · progress_bar · chip · empty_state · confirm_dialog".into(),
			Self::Chrome => "window_frame · title_bar · ChromeStyle".into(),
			Self::Icons => "icons! macro · built-in Icon set · every Lucide icon, searchable".into(),
			Self::Tokens => "colors · radii · typography".into(),
			Self::Typography => "Fonts · sans · mono · ticker".into(),
			Self::Motion => "LauncherMotion curves as springs".into(),
			Self::MotionLab => {
				"tune the open and close springs · interrupt a motion · see the overshoot".into()
			}
		}
	}

	/// A fingerprint of the [`Gallery`] fields this section shows. A field the section reads but
	/// this leaves out would leave the section stale, so it lists every field the section
	/// builders (`*_section`, `*_page`) read.
	fn state(self, gallery: &Gallery) -> u64 {
		let mut hasher = DefaultHasher::new();
		match self {
			Self::Forms => {
				(gallery.toggles, gallery.volume.to_bits(), gallery.display as u8).hash(&mut hasher)
			}
			Self::Menus => (gallery.rendering as u8, gallery.locale).hash(&mut hasher),
			Self::Icons => (&gallery.icon_text, gallery.icon_page).hash(&mut hasher),
			Self::Settings => (
				gallery.page as u8,
				gallery.display as u8,
				gallery.rendering as u8,
				gallery.game_mode,
				gallery.auto_update,
				gallery.panel_open,
				gallery.accent_index,
				gallery.volume.to_bits(),
			)
				.hash(&mut hasher),
			Self::Hud => gallery.region.hash(&mut hasher),
			Self::Typography => (gallery.ticker_percent.to_bits(), gallery.ticker_count).hash(&mut hasher),
			Self::Morph => (
				gallery.morph_open,
				gallery.fuse_slid,
				gallery.fuse_spacing.to_bits(),
			)
				.hash(&mut hasher),
			Self::Setup => (gallery.colors_choice, gallery.extras).hash(&mut hasher),
			Self::Notices => {
				(gallery.volume.to_bits(), gallery.chip_selected, &gallery.tags).hash(&mut hasher)
			}
			Self::Glass
			| Self::Style
			| Self::Actions
			| Self::Glow
			| Self::Selection
			| Self::Overlay
			| Self::Chrome
			| Self::Tokens
			| Self::Motion
			| Self::MotionLab => {}
		}
		hasher.finish()
	}

	fn body(
		self,
		gallery: &Gallery,
		theme: Theme,
		window: &mut Window,
		cx: &mut Context<Gallery>,
	) -> AnyElement {
		match self {
			Self::Glass => sections::glass_styles(theme).into_any_element(),
			Self::Style => gallery.style_section(cx).into_any_element(),
			Self::Actions => gallery.actions_section(&theme).into_any_element(),
			Self::Glow => gallery.glow_section().into_any_element(),
			Self::Forms => gallery.forms_section(cx).into_any_element(),
			Self::Menus => gallery.menus_section(cx).into_any_element(),
			Self::Selection => gallery.selection_section(&theme).into_any_element(),
			Self::Settings => gallery.settings_section(cx).into_any_element(),
			Self::Hud => gallery.hud_section(cx).into_any_element(),
			Self::Morph => gallery.morph_section(window, cx).into_any_element(),
			Self::Overlay => gallery.overlay_section(cx).into_any_element(),
			Self::Setup => gallery.setup_section(cx).into_any_element(),
			Self::Notices => gallery.notices_section(cx).into_any_element(),
			Self::Chrome => sections::chrome_section().into_any_element(),
			Self::Icons => gallery.icons_section(&theme, cx).into_any_element(),
			Self::Tokens => sections::tokens_section(theme).into_any_element(),
			Self::Typography => gallery.typography_section(cx).into_any_element(),
			Self::Motion => sections::motion_section(theme.accent).into_any_element(),
			Self::MotionLab => gallery.motion_lab.clone().into_any_element(),
		}
	}
}

/// Where a section came out in its last layout.
#[derive(Clone, Copy)]
struct Measured {
	height: Pixels,
	/// The window width the section was measured at.
	window_width: Pixels,
}

/// The page: the sections in a column. It re-renders when a section is measured anew, which
/// is the only time it has to pick a new cached height.
pub struct PageView {
	sections: Vec<Entity<SectionView>>,
}

impl PageView {
	pub fn new(gallery: &Entity<Gallery>, cx: &mut Context<Gallery>) -> Entity<Self> {
		cx.new(|cx| {
			let page = cx.entity_id();
			Self {
				sections: Kind::ALL
					.into_iter()
					.map(|kind| SectionView::new(gallery, page, kind, cx))
					.collect(),
			}
		})
	}
}

impl Render for PageView {
	fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
		div()
			.relative()
			.w_full()
			.pt(px(44.))
			.pb(px(80.))
			.px(px(36.))
			.flex()
			.flex_col()
			.gap(px(30.))
			.children(
				self.sections
					.iter()
					.map(|section| SectionView::element(section, window, cx)),
			)
	}
}

pub struct SectionView {
	gallery: WeakEntity<Gallery>,
	page: EntityId,
	kind: Kind,
	/// The part of the gallery state this section showed at its last render.
	state: u64,
	measured: Option<Measured>,
}

impl SectionView {
	/// A section that re-renders when `gallery` changes.
	fn new(
		gallery: &Entity<Gallery>,
		page: EntityId,
		kind: Kind,
		cx: &mut Context<PageView>,
	) -> Entity<Self> {
		let weak = gallery.downgrade();
		cx.new(|cx: &mut Context<Self>| {
			// Gallery state changes on every slider tick; a section follows only its own part.
			cx.observe(gallery, |this, gallery, cx| {
				if this.kind.state(gallery.read(cx)) != this.state {
					cx.notify();
				}
			})
			.detach();
			Self {
				gallery: weak,
				page,
				kind,
				state: 0,
				measured: None,
			}
		})
	}

	/// The section as a child of the page: cached at its measured height, or uncached until it
	/// has one for the current window width.
	pub fn element(this: &Entity<Self>, window: &Window, cx: &gpui::App) -> AnyElement {
		let window_width = window.viewport_size().width;
		match this.read(cx).measured {
			Some(measured) if measured.window_width == window_width => this
				.clone()
				.cached(gpui::StyleRefinement::default().w_full().h(measured.height))
				.into_any_element(),
			_ => this.clone().into_any_element(),
		}
	}
}

impl Render for SectionView {
	fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
		let Some(gallery) = self.gallery.upgrade() else {
			return div().into_any_element();
		};
		let theme = cx.theme();
		let kind = self.kind;
		let body = gallery.update(cx, |gallery, cx| {
			self.state = kind.state(gallery);
			kind.body(gallery, theme, window, cx)
		});
		let page = self.page;
		let this = cx.entity();
		let measure = canvas(
			move |bounds, window, cx| {
				let measured = Measured {
					height: bounds.size.height,
					window_width: window.viewport_size().width,
				};
				let changed = this.update(cx, |this, _| {
					let changed = this.measured.is_none_or(|old| {
						old.window_width != measured.window_width
							|| (old.height - measured.height).abs() > TOLERANCE
					});
					if changed {
						this.measured = Some(measured);
					}
					changed
				});
				if changed {
					// The page picks the new height up on its next render.
					window.on_next_frame(move |_, cx| gpui::AppContext::notify(cx, page));
				}
			},
			|_, _, _, _| {},
		)
		.absolute()
		.size_full();
		div()
			.relative()
			.w_full()
			.child(sections::section(kind.title(), kind.note(&theme), body))
			.child(measure)
			.into_any_element()
	}
}
