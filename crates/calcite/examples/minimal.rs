// SPDX-License-Identifier: MPL-2.0

//! The smallest Calcite app: one glass control bar with a toggle and a primary action.
//!
//! Run it with `cargo run -p calcite --example minimal`.

use calcite::{UI_FONT, gpui, prelude::*};

struct Minimal {
	notifications: bool,
	launches: usize,
}

impl Render for Minimal {
	fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
		let theme = cx.theme();
		div()
			.size_full()
			.flex()
			.flex_col()
			.justify_end()
			.gap(px(16.))
			.p(px(24.))
			.bg(gpui::linear_gradient(
				160.,
				gpui::linear_color_stop(gpui::rgb(0x1E3A5F), 0.),
				gpui::linear_color_stop(gpui::rgb(0x5F1E3A), 1.),
			))
			.font_family(UI_FONT)
			.text_color(theme.palette.primary_text)
			.child(
				div()
					.flex()
					.flex_row()
					.items_center()
					.gap(px(10.))
					.child("Notifications")
					.child(
						toggle("notifications", self.notifications)
							.label("Notifications")
							.on_change(cx.listener(|this, on: &bool, _, cx| {
								this.notifications = *on;
								cx.notify();
							})),
					),
			)
			.child(
				control_bar(status("Ready").detail(format!("Launched {} times", self.launches))).action(
					button("launch", "Launch")
						.accent()
						.large()
						.icon(Icon::Play)
						.on_click(cx.listener(|this, _, _, cx| {
							this.launches += 1;
							cx.notify();
						})),
				),
			)
	}
}

fn main() {
	calcite::platform::application()
		.with_assets(Assets::default())
		.run(|cx: &mut App| {
			calcite::init(cx);
			let bounds = Bounds::centered(None, size(px(640.), px(360.)), cx);
			cx.open_window(
				WindowOptions {
					window_bounds: Some(WindowBounds::Windowed(bounds)),
					..Default::default()
				},
				|_, cx| {
					cx.new(|_| Minimal {
						notifications: true,
						launches: 0,
					})
				},
			)
			.expect("failed to open the window");
			cx.activate(true);
		});
}
