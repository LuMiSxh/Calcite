// SPDX-License-Identifier: MPL-2.0

//! Named springs and secondary-motion constants. Views pick a token by feel, never a raw number.

use super::spring::Spring;

/// Held-down response: fast and flat, so a press feels attached to the finger. No overshoot.
pub const PRESS: Spring = Spring::new(0.16, 0.);
/// Small controls snapping between states: quick, a hint of life (0.6% overshoot).
pub const SNAPPY: Spring = Spring::new(0.30, 0.15);
/// Calm travel that arrives without drama: sheets, sliders, fills. No overshoot.
pub const SMOOTH: Spring = Spring::new(0.45, 0.);
/// Wobbly, liquid motion for glass that stretches and recovers (12.6% overshoot): the knob lens.
pub const JELLY: Spring = Spring::new(0.42, 0.45);
/// A glass surface growing into a larger shape (4.6% overshoot).
pub const MORPH_OPEN: Spring = Spring::new(0.48, 0.30);
/// A glass surface folding back: shorter than opening but with a clearer rebound (8.4% overshoot);
/// an attached body swells a few px past its trigger and settles back onto it.
pub const MORPH_CLOSE: Spring = Spring::new(0.34, 0.38);
/// A selection moving between options in a list or segmented control (3.8% overshoot).
pub const SELECT: Spring = Spring::new(0.40, 0.28);
/// Tooltips and other tiny hints: brisk, nearly flat (0.2% overshoot).
pub const TOOLTIP: Spring = Spring::new(0.22, 0.10);
/// Things leaving: fast and without rebound, so they get out of the way.
pub const EXIT: Spring = Spring::new(0.20, 0.);
/// A digit rolling into place: lively, with a hint of overshoot (4.6%) that settles back.
pub const TICKER: Spring = Spring::new(0.34, 0.30);

/// Delay in seconds between rows that enter together.
pub const STAGGER_IN: f32 = 0.024;
/// Delay in seconds between rows that leave together; leaving is quicker than entering.
pub const STAGGER_OUT: f32 = 0.012;
/// Rows beyond this index share the last row's delay, so long lists do not drag.
pub const STAGGER_CAP: usize = 8;

/// Length of the cross-fade between two looks, in seconds. Eased in and out, not a spring.
pub const CROSSFADE_SECONDS: f32 = 0.48;
/// Length of a determinate progress fill change, in seconds. Linear, so the fill keeps pace.
pub const PROGRESS_SECONDS: f32 = 0.20;

/// Every spring token with its name, for galleries and tests.
pub const ALL: [(&str, Spring); 10] = [
	("PRESS", PRESS),
	("SNAPPY", SNAPPY),
	("SMOOTH", SMOOTH),
	("JELLY", JELLY),
	("MORPH_OPEN", MORPH_OPEN),
	("MORPH_CLOSE", MORPH_CLOSE),
	("SELECT", SELECT),
	("TOOLTIP", TOOLTIP),
	("EXIT", EXIT),
	("TICKER", TICKER),
];

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn overshoot_percentages_match_the_documented_table() {
		let expected = [
			("PRESS", 0.),
			("SNAPPY", 0.6),
			("SMOOTH", 0.),
			("JELLY", 12.6),
			("MORPH_OPEN", 4.6),
			("MORPH_CLOSE", 8.4),
			("SELECT", 3.8),
			("TOOLTIP", 0.2),
			("EXIT", 0.),
			("TICKER", 4.6),
		];
		for ((name, spring), (doc_name, percent)) in ALL.into_iter().zip(expected) {
			assert_eq!(name, doc_name);
			let actual = spring.overshoot() * 100.;
			assert!(
				(actual - percent).abs() < 0.06,
				"{name}: {actual:.2}% vs documented {percent}%"
			);
		}
	}

	#[test]
	fn leaving_is_quicker_than_entering() {
		const {
			assert!(STAGGER_OUT < STAGGER_IN);
			assert!(EXIT.response < SMOOTH.response);
			assert!(MORPH_CLOSE.response < MORPH_OPEN.response);
		}
	}
}
