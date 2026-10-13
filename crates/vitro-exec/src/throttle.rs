// SPDX-License-Identifier: MPL-2.0

//! Bounds how often high-frequency values reach the UI.

use std::time::{Duration, Instant};

/// Admits high-frequency values (progress, transfer rates) at a bounded rate. The caller
/// passes the time, so tests use a manual clock and never sleep.
#[derive(Clone, Debug)]
pub struct Throttle {
	interval: Duration,
	last: Option<Instant>,
}

impl Throttle {
	/// About 10 Hz: what a small leaf view should redraw for per-tick progress.
	pub const PROGRESS: Duration = Duration::from_millis(100);

	pub fn new(interval: Duration) -> Self {
		Self { interval, last: None }
	}

	pub fn progress() -> Self {
		Self::new(Self::PROGRESS)
	}

	/// Whether a value produced at `now` should reach the UI.
	pub fn admit(&mut self, now: Instant) -> bool {
		match self.last {
			Some(last) if now.saturating_duration_since(last) < self.interval => false,
			_ => {
				self.last = Some(now);
				true
			}
		}
	}

	/// Admits a value unconditionally, for example the final 100 %.
	pub fn force(&mut self, now: Instant) {
		self.last = Some(now);
	}
}
