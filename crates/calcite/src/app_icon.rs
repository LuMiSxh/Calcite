// SPDX-License-Identifier: MPL-2.0

//! The app icon when the app runs outside a bundle, for example with `cargo run`.
//!
//! GPUI shows the generic executable icon until the app sets its own:
//!
//! | Platform | Bundled app                              | `cargo run`                              |
//! | -------- | ---------------------------------------- | ---------------------------------------- |
//! | macOS    | `.icns` in the `.app` (calcite-bundle)   | [`AppIcon::install`] sets the Dock icon  |
//! | Windows  | Icon resource 1 in the `.exe`            | The same: embed it in `build.rs`         |
//! | X11      | [`AppIcon::window_options`]              | [`AppIcon::window_options`]              |
//! | Wayland  | The `.desktop` file of the `app_id`      | The `.desktop` file of the `app_id`      |
//!
//! ```ignore
//! static ICON: AppIcon = calcite::app_icon!("assets/icon.png");
//!
//! application.run(|cx| {
//!     ICON.install(cx);
//!     cx.open_window(ICON.window_options(WindowOptions::default()), |_, cx| ..);
//! });
//! ```
//!
//! On Windows, `calcite_bundle::build::windows_icon("assets/icon.png")` in `build.rs` embeds
//! the icon resource that GPUI loads.

use std::sync::{Arc, OnceLock};

use gpui::{App, WindowOptions};

/// A PNG app icon compiled into the binary. Use a square image of at least 512 px.
pub struct AppIcon {
	png: &'static [u8],
	decoded: OnceLock<Option<Arc<image::RgbaImage>>>,
}

/// An [`AppIcon`] from a PNG path relative to the calling crate's `Cargo.toml`.
#[macro_export]
macro_rules! app_icon {
	($path:literal) => {
		$crate::app_icon::AppIcon::new(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/", $path)))
	};
}

impl AppIcon {
	pub const fn new(png: &'static [u8]) -> Self {
		Self {
			png,
			decoded: OnceLock::new(),
		}
	}

	/// Shows the icon in the Dock and the app switcher on macOS. Other platforms take the
	/// icon from the window or the executable, so this does nothing there. Call it once at
	/// launch, on the main thread.
	pub fn install(&self, _cx: &mut App) {
		#[cfg(target_os = "macos")]
		macos::set_application_icon(self.png);
	}

	/// `options` with the window icon set, which X11 shows in the title bar and the task bar.
	pub fn window_options(&self, mut options: WindowOptions) -> WindowOptions {
		options.icon = self.image();
		options
	}

	/// The decoded icon, or `None` when the PNG is invalid.
	pub fn image(&self) -> Option<Arc<image::RgbaImage>> {
		self.decoded
			.get_or_init(
				|| match image::load_from_memory_with_format(self.png, image::ImageFormat::Png) {
					Ok(image) => Some(Arc::new(image.to_rgba8())),
					Err(error) => {
						log_invalid(&error);
						None
					}
				},
			)
			.clone()
	}
}

fn log_invalid(error: &image::ImageError) {
	eprintln!("calcite: the app icon is not a valid PNG: {error}");
}

#[cfg(target_os = "macos")]
mod macos {
	use objc2::{AllocAnyThread, MainThreadMarker};
	use objc2_app_kit::{NSApplication, NSImage};
	use objc2_foundation::NSData;

	pub fn set_application_icon(png: &[u8]) {
		let Some(main_thread) = MainThreadMarker::new() else {
			eprintln!("calcite: AppIcon::install must run on the main thread");
			return;
		};
		let data = NSData::with_bytes(png);
		let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) else {
			eprintln!("calcite: AppKit cannot read the app icon");
			return;
		};
		let application = NSApplication::sharedApplication(main_thread);
		// SAFETY: on the main thread, with a valid image that AppKit retains.
		unsafe { application.setApplicationIconImage(Some(&image)) };
	}
}
