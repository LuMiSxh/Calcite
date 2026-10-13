// SPDX-License-Identifier: MPL-2.0

//! Icon set and asset source. The icons are simple project-owned SVGs. SF Symbols may
//! not ship outside Apple platforms, so the kit never depends on them.

use std::borrow::Cow;

use gpui::{AssetSource, Pixels, Rgba, SharedString, Styled, Svg, svg};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
	ArrowClockwise,
	ArrowDown,
	Checkmark,
	ChevronDown,
	ChevronUp,
	Forward,
	Gear,
	Globe,
	Info,
	Music,
	Pause,
	Play,
	Stop,
	Tag,
}

impl Icon {
	pub const ALL: [Self; 14] = [
		Self::ArrowClockwise,
		Self::ArrowDown,
		Self::Checkmark,
		Self::ChevronDown,
		Self::ChevronUp,
		Self::Forward,
		Self::Gear,
		Self::Globe,
		Self::Info,
		Self::Music,
		Self::Pause,
		Self::Play,
		Self::Stop,
		Self::Tag,
	];

	fn file(self) -> (&'static str, &'static [u8]) {
		macro_rules! icon {
			($name:literal) => {
				(
					concat!("icons/", $name, ".svg"),
					include_bytes!(concat!("../assets/icons/", $name, ".svg")),
				)
			};
		}
		match self {
			Self::ArrowClockwise => icon!("arrow-clockwise"),
			Self::ArrowDown => icon!("arrow-down"),
			Self::Checkmark => icon!("checkmark"),
			Self::ChevronDown => icon!("chevron-down"),
			Self::ChevronUp => icon!("chevron-up"),
			Self::Forward => icon!("forward"),
			Self::Gear => icon!("gear"),
			Self::Globe => icon!("globe"),
			Self::Info => icon!("info"),
			Self::Music => icon!("music"),
			Self::Pause => icon!("pause"),
			Self::Play => icon!("play"),
			Self::Stop => icon!("stop"),
			Self::Tag => icon!("tag"),
		}
	}

	pub fn path(self) -> &'static str {
		self.file().0
	}

	pub fn render(self, size: Pixels, color: Rgba) -> Svg {
		svg().path(self.path()).size(size).flex_none().text_color(color)
	}
}

/// Serves the embedded icons, plus any app assets, to GPUI's `svg()` and `img()` elements.
#[derive(Default)]
pub struct Assets {
	extra: Vec<(&'static str, &'static [u8])>,
}

impl Assets {
	/// Adds an app-owned asset served at `path`.
	pub fn with(mut self, path: &'static str, bytes: &'static [u8]) -> Self {
		self.extra.push((path, bytes));
		self
	}

	fn entries(&self) -> impl Iterator<Item = (&'static str, &'static [u8])> + '_ {
		Icon::ALL
			.iter()
			.map(|icon| icon.file())
			.chain(self.extra.iter().copied())
	}
}

impl AssetSource for Assets {
	fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
		Ok(self
			.entries()
			.find(|(name, _)| *name == path)
			.map(|(_, bytes)| Cow::Borrowed(bytes)))
	}

	fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
		Ok(self
			.entries()
			.map(|(name, _)| name)
			.filter(|name| name.starts_with(path))
			.map(SharedString::from)
			.collect())
	}
}
