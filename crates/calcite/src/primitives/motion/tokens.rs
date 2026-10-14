// SPDX-License-Identifier: MPL-2.0

//! Named springs and secondary-motion constants. Views pick a token by feel, never a raw number.

use super::spring::Spring;

/// Held-down response: fast and flat, so a press feels attached to the finger. No overshoot.
pub const PRESS: Spring = Spring::new(0.16, 0.);
/// Small controls snapping between states: quick, a hint of life (0.6% overshoot).
pub const SNAPPY: Spring = Spring::new(0.30, 0.15);
/// Calm travel that arrives without drama: sheets, sliders, fills. No overshoot.
pub const SMOOTH: Spring = Spring::new(0.45, 0.);
/// Playful arrival with a visible but short rebound (5.4% overshoot): toggles, toasts, pop-ins.
pub const BOUNCY: Spring = Spring::new(0.45, 0.32);
/// Slow, almost flat settle for large surfaces (0.2% overshoot).
pub const GENTLE: Spring = Spring::new(0.60, 0.10);
/// Wobbly, liquid motion for glass that stretches and recovers (12.6% overshoot): the knob lens.
pub const JELLY: Spring = Spring::new(0.42, 0.45);
/// A glass surface growing into a larger shape (4.6% overshoot).
pub const MORPH_OPEN: Spring = Spring::new(0.48, 0.30);
/// A glass surface folding back: shorter than opening but with a clearer rebound (8.4% overshoot);
/// an attached body dips a few px inside its trigger and settles back onto it.
pub const MORPH_CLOSE: Spring = Spring::new(0.34, 0.38);
/// A big attached body folding back into its trigger: calm and nearly flat (0.1% overshoot), so
/// a 200 px surface does not swing around a 28 px button on its way home.
pub const MORPH_FOLD: Spring = Spring::new(0.34, 0.04);
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

/// How strongly a surface squashes while its spring is pulled below rest (`secondary::squash`).
pub const SQUASH_GAIN: f32 = 1.5;
/// The smallest scale a squash may reach.
pub const SQUASH_MIN: f32 = 0.94;

/// Every spring token with its name, for galleries and tests.
pub const ALL: [(&str, Spring); 13] = [
	("PRESS", PRESS),
	("SNAPPY", SNAPPY),
	("SMOOTH", SMOOTH),
	("BOUNCY", BOUNCY),
	("GENTLE", GENTLE),
	("JELLY", JELLY),
	("MORPH_OPEN", MORPH_OPEN),
	("MORPH_CLOSE", MORPH_CLOSE),
	("MORPH_FOLD", MORPH_FOLD),
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
			("BOUNCY", 5.4),
			("GENTLE", 0.2),
			("JELLY", 12.6),
			("MORPH_OPEN", 4.6),
			("MORPH_CLOSE", 8.4),
			("MORPH_FOLD", 0.0),
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
