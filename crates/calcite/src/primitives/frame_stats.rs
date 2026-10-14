// SPDX-License-Identifier: MPL-2.0

//! Opt-in frame statistics: set `CALCITE_FRAME_STATS=1` and, once a second, a summary of
//! the frames drawn goes to stderr: frames, the interval between frames, the time spent in
//! `Window::draw` (render, layout, and paint of the whole window), and the time spent
//! presenting the frame to the platform.
//!
//! The numbers come from gpui's frame profiler, so the `frame-stats` cargo feature has to be
//! on (`cargo run --release --features calcite-showcase/frame-stats --bin preview`). Without
//! it, [`install`] only says so.

use gpui::App;

/// Starts the once-a-second report when `CALCITE_FRAME_STATS` is set to anything but `0`.
pub fn install(cx: &mut App) {
	if !std::env::var_os("CALCITE_FRAME_STATS").is_some_and(|value| value != "0") {
		return;
	}
	imp::install(cx);
}

#[cfg(not(feature = "frame-stats"))]
mod imp {
	pub(super) fn install(_: &mut gpui::App) {
		eprintln!("calcite: CALCITE_FRAME_STATS needs the `frame-stats` cargo feature");
	}
}

#[cfg(feature = "frame-stats")]
mod imp {
	use gpui::{App, FrameEvent, FrameTimingCollector, set_trace_enabled};
	use std::time::{Duration, Instant};

	const REPORT: Duration = Duration::from_secs(1);

	pub(super) fn install(cx: &mut App) {
		set_trace_enabled(true);
		let mut collector = FrameTimingCollector::new();
		let mut previous_draw: Option<Instant> = None;
		cx.spawn(async move |cx| {
			loop {
				cx.background_executor().timer(REPORT).await;
				let mut intervals = Vec::new();
				let mut draws = Vec::new();
				let mut presents = Vec::new();
				for event in collector.collect_unseen() {
					match event {
						FrameEvent::Draw(timing) => {
							if let Some(previous) = previous_draw {
								intervals.push(timing.draw_start.duration_since(previous));
							}
							previous_draw = Some(timing.draw_start);
							draws.push(timing.draw_duration());
						}
						FrameEvent::Present(timing) => presents.push(timing.present_duration()),
					}
				}
				eprintln!(
					"calcite frames: {:>3} | interval {} | draw {} | present {}",
					draws.len(),
					Summary::of(&mut intervals),
					Summary::of(&mut draws),
					Summary::of(&mut presents),
				);
			}
		})
		.detach();
	}

	struct Summary {
		average: f64,
		p95: f64,
		max: f64,
	}

	impl Summary {
		fn of(durations: &mut [Duration]) -> Self {
			durations.sort_unstable();
			let millis = |duration: Duration| duration.as_secs_f64() * 1000.;
			let total: f64 = durations.iter().copied().map(millis).sum();
			let last = durations.len().saturating_sub(1);
			Self {
				average: total / durations.len().max(1) as f64,
				p95: durations
					.get((durations.len() * 95 / 100).min(last))
					.map_or(0., |duration| millis(*duration)),
				max: durations.last().map_or(0., |duration| millis(*duration)),
			}
		}
	}

	impl std::fmt::Display for Summary {
		fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
			write!(
				f,
				"avg {:>5.2} p95 {:>5.2} max {:>5.2} ms",
				self.average, self.p95, self.max
			)
		}
	}
}
