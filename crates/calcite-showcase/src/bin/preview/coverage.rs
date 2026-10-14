// SPDX-License-Identifier: MPL-2.0

//! The coverage table: every public Calcite component, the preview section that shows it, and
//! the Rust call that builds it.
//!
//! The list is kept by hand. When you add a component to `calcite::prelude`, add a row here and
//! show the component in that section; the tests fail until you do:
//!
//! - every prelude builder must have a row, and
//! - every row's builder must be called in the source files of its section.
//!
//! The section pages print their rows as a `Source` block, so the table is also the usage list.

use super::section_view::Kind;

/// `(component, section, source)`: the builder, the section that shows it, and a call to copy.
/// An empty source means the section shows the builder with `demo!`, which prints its own source.
pub type Row = (&'static str, Kind, &'static str);

pub const COVERAGE: &[Row] = &[
	// Controls
	(
		"button",
		Kind::Actions,
		"button(id, label).tone(Tone::Accent).large().icon(Lucide::Download).disabled(false)",
	),
	(
		"icon_button",
		Kind::Actions,
		"icon_button(id, Icon::Gear, \"Settings\").diameter(px(40.))",
	),
	(
		"halo",
		Kind::Actions,
		"halo(button(id, \"Play\").accent().large())",
	),
	(
		"glass_tooltip",
		Kind::Actions,
		"div().id(id).glass_tooltip(\"A glass tooltip\")",
	),
	(
		"choice_card",
		Kind::Choices,
		"choice_card(id, title).kind(ChoiceKind::Single).selected(true).detail(text)",
	),
	(
		"chip",
		Kind::Choices,
		"chip(id, text).selected(true).on_remove(handler)",
	),
	("badge", Kind::Choices, "badge(text).tone(Tone::Warning)"),
	("tag", Kind::Choices, "tag(\"Recommended\")"),
	// Forms
	(
		"toggle",
		Kind::Forms,
		"toggle(id, on).label(text).large().on_change(handler)",
	),
	(
		"slider",
		Kind::Forms,
		"slider(id, value).step(0.05).ticks(11).leading_icon(Lucide::Volume1)",
	),
	(
		"segmented",
		Kind::Forms,
		"segmented(id, value, [(value, label)]).on_change(handler)",
	),
	(
		"text_field",
		Kind::Forms,
		"text_field(id, &state).placeholder(text).icon(Lucide::Search).width(px(240.))",
	),
	(
		"choice_group",
		Kind::Setup,
		"choice_group(title, Icon::Folder).footnote(text).child(choice_card(..))",
	),
	(
		"step_rail",
		Kind::Setup,
		"step_rail(header).child(step(title, icon, StepState::Current)).footer(el)",
	),
	// Menus and overlays
	(
		"menu_picker",
		Kind::Menus,
		"menu_picker(id, value, [(value, label)]).divider_before(value)",
	),
	(
		"action_menu",
		Kind::Menus,
		"action_menu(id, label, icon).item(label, icon, handler).divider()",
	),
	(
		"modal",
		Kind::Overlay,
		"modal(id, title).dimensions(px(520.), px(330.)).child(body).action(button(..))",
	),
	(
		"modal_layer",
		Kind::Overlay,
		"modal_layer(id, modal(..)).shown(open).on_dismiss(handler)",
	),
	(
		"floating_action_bar",
		Kind::Overlay,
		"floating_action_bar().child(button(..)).child(done_button(id))",
	),
	(
		"done_button",
		Kind::Overlay,
		"done_button(id).icon(Icon::ArrowDown)",
	),
	(
		"skeleton",
		Kind::Overlay,
		"skeleton(id).dimensions(px(320.), px(12.)).corner(px(6.)).pulses(true)",
	),
	// Feedback
	("banner", Kind::Notices, ""),
	(
		"spinner",
		Kind::Notices,
		"spinner().id(id).diameter(px(28.)).color(rgba)",
	),
	(
		"progress_bar",
		Kind::Notices,
		"progress_bar(id, Some(0.42)).height(px(8.)).label(text)",
	),
	(
		"progress_outline",
		Kind::Notices,
		"progress_outline(OutlineMode::Determinate(0.42)).glint(true).animated(true)",
	),
	(
		"empty_state",
		Kind::Notices,
		"empty_state(Icon::Magnifier, title).detail(text).action(button(..))",
	),
	(
		"confirm_dialog",
		Kind::Notices,
		"confirm_dialog(id, title, text).confirm(\"Uninstall\", ConfirmRole::Destructive).shown(open)",
	),
	// Layout and navigation
	(
		"nav_rail",
		Kind::Selection,
		"nav_rail(id).child(nav_item(id, title, icon).selected(true).disabled(false))",
	),
	(
		"nav_item",
		Kind::Selection,
		"nav_item(id, title, Lucide::Palette).selected(true)",
	),
	(
		"list_row",
		Kind::Selection,
		"list_row(id, label).value(text).selected(true)",
	),
	("link", Kind::Selection, "link(id, title).on_click(handler)"),
	(
		"document_link",
		Kind::Selection,
		"document_link(id, title, Lucide::FileText)",
	),
	(
		"step",
		Kind::Selection,
		"step(title, Lucide::Check, StepState::Done)",
	),
	(
		"settings_page",
		Kind::Settings,
		"settings_page(id, title, subtitle).child(settings_panel(..))",
	),
	(
		"settings_panel",
		Kind::Settings,
		"settings_panel(id, title, icon).tone(Tone::Warning).collapsible(open, handler)",
	),
	(
		"danger_zone",
		Kind::Settings,
		"danger_zone(id).child(action_row(..))",
	),
	(
		"action_row",
		Kind::Settings,
		"action_row(title).detail(text).action(toggle(..))",
	),
	("page_header", Kind::Settings, "page_header(title, subtitle)"),
	("hairline", Kind::Settings, "hairline()"),
	("glass_panel", Kind::Settings, "glass_panel().child(content)"),
	// HUD
	(
		"pill",
		Kind::Hud,
		"pill(id, Icon::Globe, || title).detail(el, w, h).expanded(open).receded(false)",
	),
	(
		"control_bar",
		Kind::Hud,
		"control_bar(status(text).detail(text)).progress(Some(OutlineMode::Indeterminate), true).action(el)",
	),
	("status", Kind::Hud, "status(text).emphasized(true).detail(text)"),
	(
		"glass_group",
		Kind::Morph,
		"glass_group(id, px(20.)).member(radius, element)",
	),
	// Window chrome
	(
		"title_bar",
		Kind::Chrome,
		"title_bar(id).preview(ChromeStyle::Windows).child(el)",
	),
	("window_frame", Kind::Chrome, "window_frame(root)"),
	(
		"app_icon",
		Kind::Chrome,
		"static ICON: AppIcon = calcite::app_icon!(\"icon.png\")",
	),
	// Foundations
	(
		"icon",
		Kind::Icons,
		"icon(Icon::Gear).size(px(24.)).text_color(color)",
	),
	(
		"glass",
		Kind::Glass,
		"div().glass(theme.hud_surface().elevation(Elevation::Raised))",
	),
	(
		"ticker",
		Kind::Typography,
		"ticker(id, \"1 280\").font_size(px(28.))",
	),
	(
		"attached_surface",
		Kind::Menus,
		"attached_surface(id).open(open).placement(Placement::Down).trigger(f).body(size, radius, f)",
	),
];

/// Prelude names that are not components: helpers and modules.
#[cfg(test)]
const NOT_COMPONENTS: &[&str] = &["icons", "keymap", "tokens", "text_state"];

/// The source files a section's builders live in.
#[cfg(test)]
fn files(kind: Kind) -> &'static [&'static str] {
	const ACTIONS: &str = include_str!("states.rs");
	const SELECTION: &str = include_str!("selection.rs");
	const INTERACTIVE: &str = include_str!("interactive.rs");
	const SETUP: &str = include_str!("setup.rs");
	const SECTIONS: &str = include_str!("sections.rs");
	match kind {
		Kind::Actions | Kind::Forms => &[ACTIONS],
		Kind::Choices | Kind::Selection => &[SELECTION],
		Kind::Settings | Kind::Hud | Kind::Morph => &[INTERACTIVE],
		// The modal and the dialog live in the window root; their sections hold the buttons.
		Kind::Overlay => &[INTERACTIVE, include_str!("main.rs")],
		Kind::Notices => &[SETUP, include_str!("main.rs")],
		Kind::Setup => &[SETUP],
		Kind::Glass | Kind::Tokens | Kind::Motion => &[SECTIONS],
		Kind::Chrome => &[SECTIONS, include_str!("main.rs")],
		// The shortcut legend at the top right of the window is an attached surface.
		Kind::Menus => &[include_str!("menus.rs"), include_str!("main.rs")],
		Kind::Icons => &[include_str!("icon_search.rs")],
		Kind::Typography => &[include_str!("typography.rs")],
		Kind::Style | Kind::Glow => &[include_str!("style_demo.rs")],
		Kind::MotionLab => &[include_str!("motion_lab.rs")],
	}
}

/// The rows of one section.
pub fn rows(kind: Kind) -> impl Iterator<Item = &'static Row> {
	COVERAGE.iter().filter(move |row| row.1 == kind)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn every_listed_component_is_built_in_its_section() {
		for (component, kind, _) in COVERAGE {
			let call = format!("{component}(");
			let call_bang = format!("{component}!(");
			assert!(
				files(*kind)
					.iter()
					.any(|source| source.contains(&call) || source.contains(&call_bang)),
				"`{component}` is listed under the {} section, but no `{call}` call is in its files",
				kind.title()
			);
		}
	}

	#[test]
	fn every_prelude_builder_has_a_section() {
		let lib = include_str!("../../../../calcite/src/lib.rs");
		let prelude = lib
			.split("pub mod prelude {")
			.nth(1)
			.and_then(|rest| rest.split("\n}\n").next())
			.expect("lib.rs declares the prelude");
		let mut names = Vec::new();
		let mut word = String::new();
		let mut chars = prelude.chars().peekable();
		while let Some(c) = chars.next() {
			if c.is_ascii_alphanumeric() || c == '_' {
				word.push(c);
				continue;
			}
			// A word followed by `::` is a module path, not an item.
			let path = c == ':' && chars.peek() == Some(&':');
			let builder = word.starts_with(|first: char| first.is_ascii_lowercase())
				&& word
					.chars()
					.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
			if builder && !path {
				names.push(std::mem::take(&mut word));
			}
			word.clear();
		}
		for name in names {
			let ignored = [
				"pub", "use", "mod", "prelude", "cfg", "feature", "lucide", "crate",
			];
			if ignored.contains(&name.as_str()) || NOT_COMPONENTS.contains(&name.as_str()) {
				continue;
			}
			assert!(
				COVERAGE.iter().any(|row| row.0 == name),
				"`{name}` is in calcite::prelude but has no row in COVERAGE: add it and show it in a section"
			);
		}
	}

	#[test]
	fn every_section_lists_its_components() {
		for kind in Kind::ALL {
			// A section that only shows the system (colors, motion, lab) may have no builder rows.
			let foundation = matches!(
				kind,
				Kind::Style | Kind::Glow | Kind::Tokens | Kind::Motion | Kind::MotionLab | Kind::Typography
			);
			assert!(
				foundation || rows(kind).next().is_some(),
				"the {} section has no rows in COVERAGE",
				kind.title()
			);
		}
	}
}
