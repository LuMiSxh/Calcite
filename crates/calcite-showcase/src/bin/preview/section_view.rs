// SPDX-License-Identifier: MPL-2.0

//! Each preview section is its own view, so an animation inside one section re-renders and
//! re-lays-out only that section. The page re-lays-out the others only when it changes.
//!
//! A cached view is laid out from a definite style instead of measured from its content, so a
//! section measures itself: a canvas inside it records the height it came out at, and the page
//! caches the section at that height. A change in the measured height (a settings page with
//! more rows) or in the window width (text wraps differently) drops the section back to
//! uncached for a frame, until it has been measured again.
//!
//! What the cache cannot see, it is told about: a section observes the [`Preview`] state and
//! re-renders when it changes; controls and animations inside it notify the section itself;
//! and a theme change, Reduce Motion, or a window resize refreshes the whole window, which
//! ignores every cache.

use std::hash::{DefaultHasher, Hash, Hasher};

use calcite::prelude::*;
use gpui::{
	AnyElement, Context, Entity, EntityId, IntoElement, ParentElement, Pixels, Render, SharedString, Styled,
	WeakEntity, Window, canvas, div, px,
};

use super::{Preview, sections};

/// A section is re-measured when its height moves by more than this.
const TOLERANCE: Pixels = px(0.5);

/// The sidebar groups, in the order the sidebar lists them.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Group {
	Foundations,
	Controls,
	Forms,
	MenusOverlays,
	Feedback,
	Layout,
	Hud,
	Playground,
}

impl Group {
	pub const ALL: [Self; 8] = [
		Self::Foundations,
		Self::Controls,
		Self::Forms,
		Self::MenusOverlays,
		Self::Feedback,
		Self::Layout,
		Self::Hud,
		Self::Playground,
	];

	pub fn title(self) -> &'static str {
		match self {
			Self::Foundations => "Foundations",
			Self::Controls => "Controls",
			Self::Forms => "Forms",
			Self::MenusOverlays => "Menus & overlays",
			Self::Feedback => "Feedback & status",
			Self::Layout => "Layout & navigation",
			Self::Hud => "HUD",
			Self::Playground => "Playground",
		}
	}
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
	Glass,
	Style,
	Actions,
	Choices,
	Glow,
	Effects,
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
	pub const ALL: [Self; 21] = [
		Self::Glass,
		Self::Style,
		Self::Actions,
		Self::Choices,
		Self::Glow,
		Self::Effects,
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

	/// The sidebar group this section is listed under.
	pub fn group(self) -> Group {
		match self {
			Self::Glass
			| Self::Glow
			| Self::Effects
			| Self::Tokens
			| Self::Typography
			| Self::Icons
			| Self::Motion
			| Self::MotionLab => Group::Foundations,
			Self::Actions | Self::Choices => Group::Controls,
			Self::Forms | Self::Setup => Group::Forms,
			Self::Menus | Self::Overlay => Group::MenusOverlays,
			Self::Notices => Group::Feedback,
			Self::Settings | Self::Selection | Self::Chrome => Group::Layout,
			Self::Hud | Self::Morph => Group::Hud,
			Self::Style => Group::Playground,
		}
	}

	/// The sidebar icon.
	pub fn icon(self) -> Lucide {
		match self {
			Self::Glass => Lucide::Layers,
			Self::Style => Lucide::SlidersHorizontal,
			Self::Actions => Lucide::MousePointer2,
			Self::Choices => Lucide::Check,
			Self::Glow => Lucide::Sparkles,
			Self::Effects => Lucide::Droplet,
			Self::Forms => Lucide::ToggleRight,
			Self::Menus => Lucide::ChevronDown,
			Self::Selection => Lucide::ListFilter,
			Self::Settings => Lucide::Settings,
			Self::Hud => Lucide::Gauge,
			Self::Morph => Lucide::Layers,
			Self::Overlay => Lucide::PanelTop,
			Self::Setup => Lucide::Sparkles,
			Self::Notices => Lucide::Bell,
			Self::Chrome => Lucide::AppWindow,
			Self::Icons => Lucide::Shapes,
			Self::Tokens => Lucide::Droplet,
			Self::Typography => Lucide::Type,
			Self::Motion => Lucide::Activity,
			Self::MotionLab => Lucide::FlaskConical,
		}
	}

	pub fn title(self) -> &'static str {
		match self {
			Self::Glass => "Glass styles",
			Self::Style => "Style",
			Self::Actions => "Buttons and actions",
			Self::Choices => "Choices and chips",
			Self::Glow => "Cursor glow",
			Self::Effects => "Effects",
			Self::Forms => "Forms",
			Self::Menus => "Menu morph",
			Self::Selection => "Selection and nav",
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
			Self::Choices => "choice_card · chip · badge · tag, by kind, selection and enabled state".into(),
			Self::Glow => "move the cursor over it: glow(true) next to glow(false)".into(),
			Self::Effects => "radial_gradient · drop_shadow · text_shadow".into(),
			Self::Forms => "toggle · slider · segmented · text_field, by state".into(),
			Self::Menus => "menu_picker · action_menu: the menu morphs out of its trigger".into(),
			Self::Selection => {
				"nav_item · nav_rail · list_row · link · document_link · step_rail · step".into()
			}
			Self::Settings => "nav_rail · page_header · settings_panel · action_row · danger_zone".into(),
			Self::Hud => "pill · control_bar · status · list_row".into(),
			Self::Morph => "pill · glass_group · O toggles the pill".into(),
			Self::Overlay => "modal · floating_action_bar · link · badge · skeleton".into(),
			Self::Setup => "step_rail · step · choice_group · choice_card · tag".into(),
			Self::Notices => {
				"banner · spinner · progress_bar · progress_outline · chip · empty_state · confirm_dialog"
					.into()
			}
			Self::Chrome => "window_frame · title_bar · app_icon · ChromeStyle".into(),
			Self::Icons => "icons! macro · built-in Icon set · every Lucide icon, searchable".into(),
			Self::Tokens => "colors · radii · typography".into(),
			Self::Typography => "Fonts · sans · mono · ticker".into(),
			Self::Motion => "LauncherMotion curves as springs".into(),
			Self::MotionLab => {
				"tune the open and close springs · interrupt a motion · see the overshoot".into()
			}
		}
	}

	/// A fingerprint of the [`Preview`] fields this section shows. A field the section reads but
	/// this leaves out would leave the section stale, so it lists every field the section
	/// builders (`*_section`, `*_page`) read.
	fn state(self, preview: &Preview) -> u64 {
		let mut hasher = DefaultHasher::new();
		match self {
			Self::Forms => (
				preview.toggles,
				preview.segments,
				preview.volume.to_bits(),
				preview.display as u8,
			)
				.hash(&mut hasher),
			Self::Menus => (preview.rendering as u8, preview.locale).hash(&mut hasher),
			Self::Icons => (&preview.icon_text, preview.icon_page).hash(&mut hasher),
			Self::Settings => (
				preview.page as u8,
				preview.display as u8,
				preview.rendering as u8,
				preview.game_mode,
				preview.auto_update,
				preview.panel_open,
				preview.accent_index,
				preview.volume.to_bits(),
			)
				.hash(&mut hasher),
			Self::Hud => preview.region.hash(&mut hasher),
			Self::Typography => (preview.ticker_percent.to_bits(), preview.ticker_count).hash(&mut hasher),
			Self::Morph => (
				preview.morph_open,
				preview.fuse_slid,
				preview.fuse_spacing.to_bits(),
			)
				.hash(&mut hasher),
			Self::Setup => (preview.colors_choice, preview.extras).hash(&mut hasher),
			Self::Notices => {
				(preview.volume.to_bits(), preview.chip_selected, &preview.tags).hash(&mut hasher)
			}
			Self::Glass
			| Self::Style
			| Self::Actions
			| Self::Glow
			| Self::Effects
			| Self::Choices
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
		preview: &Preview,
		theme: Theme,
		window: &mut Window,
		cx: &mut Context<Preview>,
	) -> AnyElement {
		match self {
			Self::Glass => sections::glass_styles(theme).into_any_element(),
			Self::Style => preview.style_section(cx).into_any_element(),
			Self::Actions => preview.actions_section(&theme).into_any_element(),
			Self::Choices => preview.choices_section(&theme).into_any_element(),
			Self::Glow => preview.glow_section().into_any_element(),
			Self::Effects => super::effects::effects_section(&theme).into_any_element(),
			Self::Forms => preview.forms_section(cx).into_any_element(),
			Self::Menus => preview.menus_section(cx).into_any_element(),
			Self::Selection => preview.selection_section().into_any_element(),
			Self::Settings => preview.settings_section(cx).into_any_element(),
			Self::Hud => preview.hud_section(cx).into_any_element(),
			Self::Morph => preview.morph_section(window, cx).into_any_element(),
			Self::Overlay => preview.overlay_section(cx).into_any_element(),
			Self::Setup => preview.setup_section(cx).into_any_element(),
			Self::Notices => preview.notices_section(cx).into_any_element(),
			Self::Chrome => sections::chrome_section().into_any_element(),
			Self::Icons => preview.icons_section(&theme, cx).into_any_element(),
			Self::Tokens => sections::tokens_section(theme).into_any_element(),
			Self::Typography => preview.typography_section(cx).into_any_element(),
			Self::Motion => sections::motion_section(theme.accent).into_any_element(),
			Self::MotionLab => preview.motion_lab.clone().into_any_element(),
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

/// The page: the section the sidebar shows, in a column. It re-renders when the selection
/// changes or a section is measured anew; those are the only times it has to pick a new height.
pub struct PageView {
	sections: Vec<Entity<SectionView>>,
	/// The section on show, as of the last change.
	shown: Kind,
}

impl PageView {
	/// The page showing `shown`. `preview` is still being built, so the caller passes its section.
	pub fn new(preview: &Entity<Preview>, shown: Kind, cx: &mut Context<Preview>) -> Entity<Self> {
		cx.new(|cx| {
			let page = cx.entity_id();
			// The sidebar changes the section; the other sections stay as they were.
			cx.observe(preview, |this: &mut PageView, observed: Entity<Preview>, cx| {
				let shown = observed.read(cx).section;
				if shown != this.shown {
					this.shown = shown;
					cx.notify();
				}
			})
			.detach();
			Self {
				sections: Kind::ALL
					.into_iter()
					.map(|kind| SectionView::new(preview, page, kind, cx))
					.collect(),
				shown,
			}
		})
	}
}

impl Render for PageView {
	fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
		let mut column = div()
			.relative()
			.w_full()
			.pt(px(44.))
			.pb(px(80.))
			.px(px(36.))
			.flex()
			.flex_col()
			.gap(px(30.));
		for section in self
			.sections
			.iter()
			.filter(|section| section.read(cx).kind == self.shown)
		{
			column = column.child(SectionView::element(section, window, cx));
		}
		column
	}
}

pub struct SectionView {
	preview: WeakEntity<Preview>,
	page: EntityId,
	kind: Kind,
	/// The part of the preview state this section showed at its last render.
	state: u64,
	measured: Option<Measured>,
}

impl SectionView {
	/// A section that re-renders when `preview` changes.
	fn new(
		preview: &Entity<Preview>,
		page: EntityId,
		kind: Kind,
		cx: &mut Context<PageView>,
	) -> Entity<Self> {
		let weak = preview.downgrade();
		cx.new(|cx: &mut Context<Self>| {
			// Preview state changes on every slider tick; a section follows only its own part.
			cx.observe(preview, |this, preview, cx| {
				if this.kind.state(preview.read(cx)) != this.state {
					cx.notify();
				}
			})
			.detach();
			Self {
				preview: weak,
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
		let Some(preview) = self.preview.upgrade() else {
			return div().into_any_element();
		};
		let theme = cx.theme();
		let kind = self.kind;
		let body = preview.update(cx, |preview, cx| {
			self.state = kind.state(preview);
			kind.body(preview, theme, window, cx)
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
			.child(sections::section(
				kind.title(),
				kind.note(&theme),
				sections::source_block(kind),
				body,
			))
			.child(measure)
			.into_any_element()
	}
}
