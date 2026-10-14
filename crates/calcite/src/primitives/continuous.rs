// SPDX-License-Identifier: MPL-2.0

//! Looping decorative motion on shared clocks (`ContinuousMotion` in the launcher).
//!
//! Each loop type (the progress sweep, the spinner, the skeleton pulse) has one clock, so
//! every view of that type moves in step. A clock advances only while decorative motion
//! may run: not under Reduce Motion, not in a background window, not while the app pauses
//! it, and not under serious thermal pressure. A paused clock keeps its phase and resumes
//! there, instead of restarting the loop. Running clocks redraw at most
//! [`DECORATIVE_FPS`] times a second.
//!
//! ```ignore
//! let phase = ContinuousMotion::phase("spinner", 1.1, &theme, window, cx);
//! glyph.with_transformation(Transformation::rotate(percentage(phase)))
//! ```

use std::{
	collections::HashMap,
	time::{Duration, Instant},
};

use gpui::{AnyWindowHandle, App, AppContext as _, EntityId, Global, ThermalState, Window};

use crate::{motion::DECORATIVE_FPS, theme::Theme};

/// The longest step one frame may add: a stall or a breakpoint must not jump the loop.
const MAX_STEP: Duration = Duration::from_millis(100);

#[derive(Default)]
struct Clocks {
	elapsed: HashMap<&'static str, Clock>,
	/// Windows with a redraw already scheduled, so many views share one timer.
	scheduled: Vec<AnyWindowHandle>,
	/// The next redraw on each window's frame grid.
	due: HashMap<AnyWindowHandle, Instant>,
	/// The views that read a clock since each window's last tick. A tick redraws these, not the
	/// whole window, so cached views that show no loop keep their layout.
	views: HashMap<AnyWindowHandle, Vec<EntityId>>,
}

#[derive(Default)]
struct Clock {
	seconds: f64,
	last: Option<Instant>,
}

impl Global for Clocks {}

/// Shared clocks for looping decorative motion.
pub struct ContinuousMotion;

impl ContinuousMotion {
	/// Whether looping motion may run in `window` now.
	pub fn allowed(theme: &Theme, window: &Window, cx: &App) -> bool {
		theme.decorative_motion(window)
			&& !matches!(cx.thermal_state(), ThermalState::Serious | ThermalState::Critical)
	}

	/// The phase from 0 to 1 of the loop `key` with a `period` in seconds. Call it from a
	/// render; it schedules the next frame while the loop runs.
	pub fn phase(key: &'static str, period: f32, theme: &Theme, window: &mut Window, cx: &mut App) -> f32 {
		let running = Self::allowed(theme, window, cx);
		let now = Instant::now();
		let clocks = cx.default_global::<Clocks>();
		let clock = clocks.elapsed.entry(key).or_default();
		if running {
			if let Some(last) = clock.last {
				clock.seconds += now.saturating_duration_since(last).min(MAX_STEP).as_secs_f64();
			}
			clock.last = Some(now);
		} else {
			// Frozen: the next running frame starts from here without a jump.
			clock.last = None;
		}
		let phase = (clock.seconds / f64::from(period.max(0.001))).fract() as f32;
		if running {
			schedule_frame(window, cx);
		}
		phase
	}
}

fn schedule_frame(window: &mut Window, cx: &mut App) {
	let handle = window.window_handle();
	let now = Instant::now();
	let view = window.current_view();
	let clocks = cx.default_global::<Clocks>();
	let views = clocks.views.entry(handle).or_default();
	if !views.contains(&view) {
		views.push(view);
	}
	if clocks.scheduled.contains(&handle) {
		return;
	}
	clocks.scheduled.push(handle);
	let interval = Duration::from_secs_f32(1. / DECORATIVE_FPS);
	// Stay on a fixed grid: a timer started after each draw would add the draw and the wait
	// for the next display refresh to every step (33 ms became 50 ms, 20 fps instead of 30).
	let previous = clocks.due.get(&handle).copied().unwrap_or(now);
	let mut due = previous + interval;
	if due + interval < now {
		due = now + interval;
	}
	clocks.due.insert(handle, due);
	wait_for(window, handle, due);
}

/// How early a display refresh may redraw: half a refresh at 120 Hz, so a refresh just
/// before the grid line is not skipped for the one after it.
const REFRESH_SLACK: Duration = Duration::from_millis(4);

/// Redraws `window` on the first display refresh at `due`. The frame callback runs on every
/// refresh but does not draw, so the wait costs no GPU work. Only the views that read a clock
/// are notified (`Window::refresh` would also re-render cached views).
fn wait_for(window: &mut Window, handle: AnyWindowHandle, due: Instant) {
	window.on_next_frame(move |window, cx| {
		if Instant::now() + REFRESH_SLACK < due {
			return wait_for(window, handle, due);
		}
		let clocks = cx.default_global::<Clocks>();
		clocks.scheduled.retain(|scheduled| *scheduled != handle);
		let views = clocks.views.remove(&handle).unwrap_or_default();

		for view in views {
			cx.notify(view);
		}
	});
}
