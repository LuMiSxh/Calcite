// SPDX-License-Identifier: MPL-2.0

//! Icon sets and the asset source that serves them.
//!
//! Declare an app's own icons with [`icons!`](crate::icons!); they render like the built-in
//! [`Icon`] set and register with [`Assets::icons`]. SF Symbols may not ship outside Apple
//! platforms, so the kit never depends on them.
//!
//! All glyphs come from [Lucide](https://lucide.dev) (ISC, see `assets/lucide/LICENSE`).
//! [`Icon`] names the glyphs the components use; with the default `lucide` feature,
//! [`Lucide`] names every icon of the set, for example `Lucide::Rocket`.
//! `scripts/update-lucide.sh` vendors a new Lucide version.

use std::borrow::Cow;

use gpui::{AssetSource, IntoElement, Pixels, Rgba, SharedString, Styled, Svg, svg};

/// A set of embedded SVG icons. Implement it with [`icons!`](crate::icons!).
pub trait IconSet: Copy + 'static {
	/// Every icon in the set.
	fn all() -> &'static [Self];
	/// The asset path the icon is served at.
	fn path(self) -> &'static str;
	/// The SVG source.
	fn bytes(self) -> &'static [u8];
	/// The variant name, for galleries and debugging.
	fn name(self) -> &'static str;

	/// The icon at `size` in `color`. SVGs render as masks, so any fill color works.
	fn render(self, size: Pixels, color: Rgba) -> Svg {
		svg().path(self.path()).size(size).flex_none().text_color(color)
	}
}

/// A reference to an icon of any set, as components store it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IconRef(&'static str);

impl IconRef {
	pub fn render(self, size: Pixels, color: Rgba) -> Svg {
		svg().path(self.0).size(size).flex_none().text_color(color)
	}
}

impl<I: IconSet> From<I> for IconRef {
	fn from(icon: I) -> Self {
		Self(icon.path())
	}
}

/// An icon as an element; style it like any SVG: `icon(Icon::Play).size_3()`.
pub fn icon(icon: impl Into<IconRef>) -> impl IntoElement + Styled {
	svg().path(icon.into().0).flex_none()
}

/// Declares an icon set embedded from SVG files.
///
/// ```ignore
/// calcite::icons! {
///     /// Icons of my app.
///     pub enum AppIcon in "assets/icons" {
///         Rocket => "rocket.svg",
///         Shield => "shield.svg",
///     }
/// }
/// // Register them once: Assets::default().icons::<AppIcon>()
/// // Render: AppIcon::Rocket.render(px(16.), color), or button(..).icon(AppIcon::Rocket)
/// ```
///
/// Paths are relative to the calling crate's `Cargo.toml`.
#[macro_export]
macro_rules! icons {
	(
		$(#[$meta:meta])*
		$vis:vis enum $name:ident in $directory:literal {
			$($(#[$variant_meta:meta])* $variant:ident => $file:literal),* $(,)?
		}
	) => {
		$(#[$meta])*
		#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
		$vis enum $name {
			$($(#[$variant_meta])* $variant),*
		}

		impl $name {
			/// Every icon in the set.
			pub const ALL: &'static [Self] = &[$(Self::$variant),*];
		}

		impl $crate::icons::IconSet for $name {
			fn all() -> &'static [Self] {
				Self::ALL
			}

			fn path(self) -> &'static str {
				match self {
					$(Self::$variant => concat!(env!("CARGO_PKG_NAME"), "/", $directory, "/", $file)),*
				}
			}

			fn bytes(self) -> &'static [u8] {
				match self {
					$(Self::$variant => include_bytes!(concat!(
						env!("CARGO_MANIFEST_DIR"), "/", $directory, "/", $file
					))),*
				}
			}

			fn name(self) -> &'static str {
				match self {
					$(Self::$variant => stringify!($variant)),*
				}
			}
		}
	};
}

crate::icons! {
	/// The glyphs the components use, with semantic names. Transport glyphs (play, pause,
	/// stop, forward) are the Lucide glyphs filled, like their SF Symbols counterparts.
	pub enum Icon in "assets/icons" {
		ArrowClockwise => "arrow-clockwise.svg",
		ArrowDown => "arrow-down.svg",
		Checkmark => "checkmark.svg",
		/// An empty radio button.
		Circle => "circle.svg",
		CircleAlert => "circle-alert.svg",
		/// A selected radio button: a filled disc with the check cut out.
		CircleCheckFill => "circle-check-fill.svg",
		CircleX => "circle-x.svg",
		ChevronDown => "chevron-down.svg",
		ChevronRight => "chevron-right.svg",
		ChevronUp => "chevron-up.svg",
		ChevronUpDown => "chevron-up-down.svg",
		Copy => "copy.svg",
		Document => "document.svg",
		Folder => "folder.svg",
		Forward => "forward.svg",
		Gear => "gear.svg",
		Globe => "globe.svg",
		Info => "info.svg",
		/// The open arc of an indeterminate spinner.
		LoaderCircle => "loader-circle.svg",
		Magnifier => "magnifier.svg",
		Minus => "minus.svg",
		Music => "music.svg",
		Pause => "pause.svg",
		Play => "play.svg",
		Sparkles => "sparkles.svg",
		/// An empty checkbox.
		Square => "square.svg",
		/// A checked checkbox: a filled square with the check cut out.
		SquareCheckFill => "square-check-fill.svg",
		Stop => "stop.svg",
		Tag => "tag.svg",
		Warning => "warning.svg",
		XMark => "xmark.svg",
	}
}

#[cfg(feature = "lucide")]
include!(concat!(env!("OUT_DIR"), "/lucide.rs"));

impl Icon {
	/// Renders the icon; kept as an inherent method so callers need no trait import.
	pub fn render(self, size: Pixels, color: Rgba) -> Svg {
		IconSet::render(self, size, color)
	}
}

/// Serves the built-in icons, registered icon sets, and other app assets to GPUI's `svg()`
/// and `img()` elements. The default registers [`Icon`] and, with the `lucide` feature,
/// [`Lucide`].
pub struct Assets {
	entries: Vec<(&'static str, &'static [u8])>,
}

impl Default for Assets {
	fn default() -> Self {
		let assets = Self { entries: Vec::new() }.icons::<Icon>();
		#[cfg(feature = "lucide")]
		let assets = assets.icons::<Lucide>();
		assets
	}
}

impl Assets {
	/// Registers every icon of an icon set.
	pub fn icons<I: IconSet>(mut self) -> Self {
		self.entries
			.extend(I::all().iter().map(|icon| (icon.path(), icon.bytes())));
		self
	}

	/// Registers one app-owned asset served at `path`.
	pub fn with(mut self, path: &'static str, bytes: &'static [u8]) -> Self {
		self.entries.push((path, bytes));
		self
	}
}

impl AssetSource for Assets {
	fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
		Ok(self
			.entries
			.iter()
			.find(|(name, _)| *name == path)
			.map(|(_, bytes)| Cow::Borrowed(*bytes)))
	}

	fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
		Ok(self
			.entries
			.iter()
			.map(|(name, _)| *name)
			.filter(|name| name.starts_with(path))
			.map(SharedString::from)
			.collect())
	}
}
