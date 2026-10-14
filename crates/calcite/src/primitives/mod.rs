// SPDX-License-Identifier: MPL-2.0

//! Primitives: the shared building blocks the components are made of. Apps rarely call them
//! directly; reach for them to build a custom control that looks and moves like the built-in ones.
//!
//! Every public module is also reachable at the crate root (`calcite::glass`, `calcite::motion`...).

pub mod attached;
pub mod continuous;
pub mod frame_stats;
pub mod glass;
pub mod glass_group;
pub mod interaction;
pub mod keymap;
pub mod knob;
pub mod morph;
pub mod motion;
pub mod surface;
