// SPDX-License-Identifier: MPL-2.0

//! The bundled families reach the platform text system through `calcite::fonts::register`.
//! The Mac platform must be created on the main thread, which libtest's worker threads are
//! not, so this test has no harness (`harness = false` in Cargo.toml) and runs `main` itself.

use calcite::fonts::{Family, register};
use calcite::gpui::Application;

fn main() {
	// The platform may end the process when the app quits, so the check runs before `quit`
	// and reports through the exit code.
	Application::with_platform(calcite::platform::current_platform(true)).run(|cx| {
		register(cx);
		let names = cx.text_system().all_font_names();
		let missing: Vec<&str> = Family::ALL
			.iter()
			.map(|family| family.name())
			.filter(|name| !names.iter().any(|listed| listed == name))
			.collect();
		if missing.is_empty() {
			println!(
				"fonts_platform: all {} bundled families are listed",
				Family::ALL.len()
			);
		} else {
			eprintln!("fonts_platform: not listed after registration: {missing:?} (listed: {names:?})");
			std::process::exit(1);
		}
		cx.quit();
	});
}
