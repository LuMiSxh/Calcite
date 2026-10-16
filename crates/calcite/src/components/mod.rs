// SPDX-License-Identifier: MPL-2.0

//! Components: the user-facing building blocks an app calls directly, such as buttons, forms,
//! menus, HUD surfaces, layout parts, notices, overlays, and the window chrome.
//!
//! Every module is also reachable at the crate root (`calcite::controls`, `calcite::forms`...).

pub mod app_icon;
pub mod artwork;
pub mod choice;
pub mod chrome;
pub mod controls;
pub mod feedback;
pub mod forms;
pub mod hud;
pub mod layout;
pub mod menu;
pub mod notice;
pub mod overlay;
pub mod progress;
pub mod ticker;
