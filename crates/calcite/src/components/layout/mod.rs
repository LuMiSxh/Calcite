// SPDX-License-Identifier: MPL-2.0

//! Page and panel layout: [`page_header`], [`settings_page`], [`settings_panel`],
//! [`danger_zone`], [`action_row`], [`hairline`], [`nav_rail`] with [`nav_item`],
//! [`list_row`], [`link`], [`document_link`], and [`glass_panel`]. Every builder reads the
//! theme from the app (`cx.theme()`); `.theme(theme)` overrides it per component.
//!
//! ```ignore
//! settings_page("updates", "Updates", "Keep the launcher and the game current.")
//!     .child(settings_panel("Launcher", Icon::Sparkles)
//!         .child(action_row("Automatic updates").detail("Checks once a day")
//!             .action(toggle("auto", self.auto).label("Automatic updates"))))
//! ```

mod links;
mod nav;
mod page;
mod panel;
mod rows;

pub use links::{DocumentLink, Link, document_link, link};
pub use nav::{NavItem, NavRail, nav_item, nav_rail};
pub use page::{Hairline, PageHeader, SettingsPage, hairline, page_header, settings_page};
pub use panel::{ActionRow, GlassPanel, SettingsPanel, action_row, danger_zone, glass_panel, settings_panel};
pub use rows::{ListRow, list_row};
