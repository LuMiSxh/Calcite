// SPDX-License-Identifier: MPL-2.0

//! Regression: a morphing HUD pill keeps its title at the same place relative to the pill at
//! every morph value. Opening and closing only grows the pill around the title; the title must
//! not shift, and the settled-closed layout must match the last morph frame.

use std::{
	cell::RefCell,
	rc::Rc,
	time::{Duration, Instant},
};

use calcite::{hud::pill, icons::Icon};
use gpui::{
	AppContext as _, Bounds, Context, IntoElement, ParentElement, Pixels, Render, Styled, TestAppContext,
	Window, WindowHandle, canvas, div, px,
};

const DETAIL_HEIGHT: f32 = 200.;

#[derive(Clone, Default)]
struct Probes {
	pill: Rc<RefCell<Option<Bounds<Pixels>>>>,
	title: Rc<RefCell<Option<Bounds<Pixels>>>>,
}

struct Host {
	expanded: bool,
	probes: Probes,
}

impl Render for Host {
	fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
		let title_probe = self.probes.title.clone();
		let pill_probe = self.probes.pill.clone();
		let hud = pill("server-time", Icon::Globe, move || {
			let title_probe = title_probe.clone();
			div().relative().child("Global 04:12:09").child(
				canvas(
					move |bounds, _, _| *title_probe.borrow_mut() = Some(bounds),
					|_, _, _, _| {},
				)
				.absolute()
				.size_full(),
			)
		})
		.noun("server time")
		.detail(div().size(px(120.)), px(360.), px(DETAIL_HEIGHT))
		.expanded(self.expanded);
		// The wrapper is sized by its content, so its bounds are the pill's bounds.
		div().flex().flex_col().items_start().p(px(20.)).child(
			div().relative().flex().flex_row().child(hud).child(
				canvas(
					move |bounds, _, _| *pill_probe.borrow_mut() = Some(bounds),
					|_, _, _, _| {},
				)
				.absolute()
				.size_full(),
			),
		)
	}
}

#[derive(Clone, Copy, Debug)]
struct Sample {
	pill: Bounds<Pixels>,
	title: Bounds<Pixels>,
}

fn host(cx: &mut TestAppContext) -> (WindowHandle<Host>, Probes) {
	cx.update(calcite::init);
	let probes = Probes::default();
	let window = cx.add_window({
		let probes = probes.clone();
		move |_, _| Host {
			expanded: false,
			probes,
		}
	});
	cx.update_window(window.into(), |_, window, _| window.activate())
		.unwrap();
	cx.run_until_parked();
	(window, probes)
}

fn set_expanded(window: WindowHandle<Host>, cx: &mut TestAppContext, expanded: bool) {
	window
		.update(cx, |host, _, cx| {
			host.expanded = expanded;
			cx.notify();
		})
		.unwrap();
}

/// Draws one frame. The root view is notified so the pill re-renders and its springs advance.
fn frame(window: WindowHandle<Host>, cx: &mut TestAppContext, probes: &Probes) -> Sample {
	window.update(cx, |_, _, cx| cx.notify()).unwrap();
	cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
		.unwrap();
	let pill = probes.pill.borrow().expect("the pill was not laid out");
	let title = probes.title.borrow().expect("the title was not laid out");
	Sample { pill, title }
}

/// Samples every frame for `duration` while the pill morphs.
fn sweep(
	window: WindowHandle<Host>,
	cx: &mut TestAppContext,
	probes: &Probes,
	duration: Duration,
) -> Vec<Sample> {
	let start = Instant::now();
	let mut samples = Vec::new();
	while start.elapsed() < duration {
		samples.push(frame(window, cx, probes));
		std::thread::sleep(Duration::from_millis(4));
	}
	samples
}

/// The title origin relative to the pill's top-left.
fn offset(sample: &Sample) -> (f32, f32) {
	let dx = f32::from(sample.title.origin.x) - f32::from(sample.pill.origin.x);
	let dy = f32::from(sample.title.origin.y) - f32::from(sample.pill.origin.y);
	(dx, dy)
}

#[gpui::test]
fn title_stays_put_relative_to_the_pill_across_the_morph(cx: &mut TestAppContext) {
	let (window, probes) = host(cx);
	let settled = frame(window, cx, &probes);
	let (x0, y0) = offset(&settled);

	let mut samples = Vec::new();
	set_expanded(window, cx, true);
	samples.extend(sweep(window, cx, &probes, Duration::from_millis(1200)));
	set_expanded(window, cx, false);
	samples.extend(sweep(window, cx, &probes, Duration::from_millis(1200)));
	samples.push(frame(window, cx, &probes));

	// The sweep must pass through intermediate morph values, or it proves nothing.
	let morphing = samples
		.iter()
		.filter(|sample| (40.0..190.0).contains(&f32::from(sample.pill.size.height)))
		.count();
	assert!(morphing > 0, "the sweep did not sample the morph");

	for sample in &samples {
		let (dx, dy) = offset(sample);
		assert!(
			(dx - x0).abs() <= 0.5 && (dy - y0).abs() <= 0.5,
			"the title sat at ({dx}, {dy}) relative to the pill, not ({x0}, {y0}), at pill height {}",
			f32::from(sample.pill.size.height),
		);
	}
}
