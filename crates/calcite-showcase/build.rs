// SPDX-License-Identifier: MPL-2.0

fn main() {
	// Windows takes the window and Explorer icon from the executable's resources.
	calcite_bundle::build::windows_icon("assets/icon.png");
}
