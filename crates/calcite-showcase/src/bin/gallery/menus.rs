// SPDX-License-Identifier: MPL-2.0

//! The attached menu: the options are a second glass surface that grows out of the trigger and
//! fuses with it; the trigger itself stays in place.

use calcite::prelude::*;
use gpui::{Context, IntoElement};

use super::{
	Gallery, Rendering,
	states::{cell, stack, state_row},
};

const LOCALES: [&str; 6] = ["English", "Deutsch", "日本語", "한국어", "简体中文", "Français"];

impl Gallery {
	pub fn menus_section(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
		let rendering = |id: &'static str| {
			menu_picker(
				id,
				self.rendering,
				[
					(Rendering::Retina, "Retina"),
					(Rendering::MetalFx, "MetalFX"),
					(Rendering::Standard, "Standard"),
				],
			)
			.divider_before(Rendering::Standard)
		};
		let actions = |id: &'static str| {
			action_menu(id, "Logs", Lucide::FileText)
				.item("Show in Finder", Lucide::FolderOpen, |_, _| {})
				.item("Copy Path", Lucide::Copy, |_, _| {})
				.divider()
				.item("Clear Logs", Lucide::Trash2, |_, _| {})
		};
		stack(vec![
			state_row(
				"menu_picker",
				"the select: click, the options fuse to the trigger's bottom edge",
				vec![
					cell(
						"rest · hover me · click to open",
						rendering("menu-rendering").on_change(cx.listener(
							|this, rendering: &Rendering, _, cx| {
								this.rendering = *rendering;
								cx.notify();
							},
						)),
					),
					cell(
						"six options, tall menu",
						menu_picker(
							"menu-locale",
							self.locale,
							LOCALES.iter().enumerate().map(|(index, name)| (index, *name)),
						)
						.on_change(cx.listener(|this, locale: &usize, _, cx| {
							this.locale = *locale;
							cx.notify();
						})),
					),
					cell("disabled", rendering("menu-disabled").disabled(true)),
				],
			),
			state_row(
				"action_menu",
				"commands with icons and a divider",
				vec![
					cell("rest · click to open", actions("menu-actions")),
					cell("disabled", actions("menu-actions-disabled").disabled(true)),
				],
			),
		])
	}
}
