// SPDX-License-Identifier: MPL-2.0

//! The icons section: the built-in set, and a searchable, paged grid of every Lucide icon.
//! Only one page of tiles is built, so the frame cost does not grow with the set.

use std::ops::Range;

use calcite::prelude::*;
use gpui::{Context, IntoElement, ParentElement, Styled, div, px};

use super::{Preview, sections::label};

/// Tiles per page.
pub const PAGE_SIZE: usize = 96;

/// Lowercase letters and digits only, so `arrow-down`, `ArrowDown` and `arrow down` agree.
fn normalize(text: &str) -> String {
	text.chars()
		.filter(char::is_ascii_alphanumeric)
		.map(|c| c.to_ascii_lowercase())
		.collect()
}

/// The items whose name contains `query`, names that start with it first, each group in the
/// order of `items`. An empty query keeps everything.
pub fn matching<T: Copy>(items: &[T], name: impl Fn(T) -> &'static str, query: &str) -> Vec<T> {
	let query = normalize(query);
	if query.is_empty() {
		return items.to_vec();
	}
	let (mut prefix, mut inside) = (Vec::new(), Vec::new());
	for item in items {
		let name = normalize(name(*item));
		if name.starts_with(&query) {
			prefix.push(*item);
		} else if name.contains(&query) {
			inside.push(*item);
		}
	}
	prefix.append(&mut inside);
	prefix
}

/// How many pages `total` results take; at least one.
pub fn page_count(total: usize) -> usize {
	total.div_ceil(PAGE_SIZE).max(1)
}

/// The result indices on `page`, which clamps to the last page.
pub fn page_range(total: usize, page: usize) -> Range<usize> {
	let page = page.min(page_count(total) - 1);
	let start = page * PAGE_SIZE;
	start..(start + PAGE_SIZE).min(total)
}

fn tile(glyph: gpui::Svg, name: &'static str, corner: gpui::Pixels) -> impl IntoElement {
	div()
		.w(px(96.))
		.h(px(64.))
		.flex_none()
		.flex()
		.flex_col()
		.items_center()
		.justify_center()
		.gap(px(6.))
		.px(px(4.))
		.rounded(corner)
		.bg(tokens::white(0.05))
		.border_1()
		.border_color(tokens::white(0.08))
		.child(glyph)
		.child(
			div()
				.max_w_full()
				.overflow_hidden()
				.whitespace_nowrap()
				.text_ellipsis()
				.child(label(name)),
		)
}

impl Preview {
	pub fn icons_section(&self, theme: &Theme, cx: &mut Context<Self>) -> impl IntoElement + use<> {
		let results = matching(Lucide::ALL, IconSet::name, &self.icon_text);
		let pages = page_count(results.len());
		let page = self.icon_page.min(pages - 1);
		let shown = page_range(results.len(), page);
		let color = theme.palette.primary_text;
		let corner = theme.style.radii.control;

		div()
			.flex()
			.flex_col()
			.gap(px(12.))
			.child(label("Icon: the semantic set the components use"))
			.child(
				div()
					.flex()
					.flex_row()
					.items_center()
					.gap(px(10.))
					.child(icon(Icon::Gear).size(px(24.)).text_color(color))
					.child(label(
						"icon(Icon::Gear).size(px(24.)).text_color(color): any size and color",
					)),
			)
			.child(
				div().flex().flex_row().flex_wrap().gap(px(10.)).children(
					Icon::ALL
						.iter()
						.map(|icon| tile(icon.render(px(20.), color), icon.name(), corner)),
				),
			)
			.child(label(format!(
				"Lucide: {} of {} icons match · page {} of {}",
				results.len(),
				Lucide::ALL.len(),
				page + 1,
				pages
			)))
			.child(
				div()
					.flex()
					.flex_row()
					.items_center()
					.gap(px(10.))
					.child(
						text_field("icon-search", &self.icon_query)
							.placeholder("Search Lucide icons")
							.icon(Lucide::Search)
							.width(px(280.)),
					)
					.child(
						button("icon-prev", "Previous")
							.compact()
							.icon(Lucide::ChevronLeft)
							.disabled(page == 0)
							.on_click(cx.listener(move |this, _, _, cx| {
								this.icon_page = page.saturating_sub(1);
								cx.notify();
							})),
					)
					.child(
						button("icon-next", "Next")
							.compact()
							.icon(Lucide::ChevronRight)
							.disabled(page + 1 >= pages)
							.on_click(cx.listener(move |this, _, _, cx| {
								this.icon_page = page + 1;
								cx.notify();
							})),
					),
			)
			.child(
				div().flex().flex_row().flex_wrap().gap(px(10.)).children(
					results[shown]
						.iter()
						.map(|icon| tile(icon.render(px(20.), color), icon.name(), corner)),
				),
			)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	const NAMES: [&str; 5] = ["ArrowDown", "ArrowUp", "Download", "Gamepad2", "Cloud-Download"];

	fn find(query: &str) -> Vec<&'static str> {
		matching(&NAMES, |name| name, query)
	}

	#[test]
	fn an_empty_query_keeps_every_icon() {
		assert_eq!(find(""), NAMES);
		assert_eq!(find("  "), NAMES);
	}

	#[test]
	fn prefix_matches_come_before_inner_matches() {
		assert_eq!(find("down"), ["Download", "ArrowDown", "Cloud-Download"]);
	}

	#[test]
	fn case_spaces_and_dashes_do_not_matter() {
		assert_eq!(find("ARROW dOwn"), ["ArrowDown"]);
		assert_eq!(find("cloud-download"), ["Cloud-Download"]);
		assert_eq!(find("gamepad 2"), ["Gamepad2"]);
	}

	#[test]
	fn no_match_is_empty() {
		assert!(find("zzz").is_empty());
	}

	#[test]
	fn pages_cover_the_results_and_clamp() {
		assert_eq!(page_count(0), 1);
		assert_eq!(page_count(PAGE_SIZE), 1);
		assert_eq!(page_count(PAGE_SIZE + 1), 2);
		assert_eq!(page_range(0, 0), 0..0);
		assert_eq!(page_range(PAGE_SIZE + 5, 1), PAGE_SIZE..PAGE_SIZE + 5);
		assert_eq!(page_range(PAGE_SIZE + 5, 9), PAGE_SIZE..PAGE_SIZE + 5);
	}
}
