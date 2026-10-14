// SPDX-License-Identifier: MPL-2.0

//! Secondary motion: small effects that follow the main spring, such as squash, stretch, glass
//! reacting to speed, and a rim pulse on landing. Pure functions of the spring's value, velocity,
//! or elapsed time. Each takes `reduced` (Reduce Motion) and returns the identity when it is set.

use super::tokens::{SQUASH_GAIN, SQUASH_MIN, STAGGER_CAP, STAGGER_IN, STAGGER_OUT};

/// A spring value below rest (negative) is pulled past its start, so the surface squashes.
const SQUASH_RANGE: f32 = 0.06;
/// The speed (units per second) at which glass effects reach full strength.
const FULL_SPEED: f32 = 6.;
/// The speed at which a knob or segment reaches full stretch.
const FULL_STRETCH_SPEED: f32 = 8.;
/// How long the rim stays lit after landing, in seconds.
pub const LANDING_DURATION: f32 = 0.18;

/// Uniform scale for a surface whose spring value `t` dipped below rest (`t < 0`): `1 − 1.5u` with
/// `u = min(−t, 0.06)`, never below 0.94. At or above rest the scale is 1.
pub fn squash(t: f32, reduced: bool) -> f32 {
	if reduced || t >= 0. || t.is_nan() {
		return 1.;
	}
	let u = (-t).min(SQUASH_RANGE);
	(1. - SQUASH_GAIN * u).max(SQUASH_MIN)
}

/// How fast something moves, from 0 (still) to 1 (at or above 6 units per second).
pub fn speed_factor(velocity: f32) -> f32 {
	if velocity.is_nan() {
		return 0.;
	}
	(velocity.abs() / FULL_SPEED).clamp(0., 1.)
}

/// What glass adds while its surface moves quickly.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlassBoost {
	/// Added to the glass brightness.
	pub brightness: f32,
	/// Added to the specular highlight strength.
	pub specular: f32,
	/// Multiplies the refraction strength.
	pub refraction: f32,
}

impl GlassBoost {
	pub const NONE: Self = Self {
		brightness: 0.,
		specular: 0.,
		refraction: 1.,
	};
}

/// Glass reacting to `speed` (a [`speed_factor`]): brightness +0.06 s, specular +0.35 s,
/// refraction ×(1 + 0.25 s).
pub fn glass_boost(speed: f32, reduced: bool) -> GlassBoost {
	if reduced {
		return GlassBoost::NONE;
	}
	let s = speed.clamp(0., 1.);
	GlassBoost {
		brightness: 0.06 * s,
		specular: 0.35 * s,
		refraction: 1. + 0.25 * s,
	}
}

/// Extra rim light just after something lands: +0.4 at `elapsed` 0, fading linearly to 0 over
/// [`LANDING_DURATION`] seconds.
pub fn landing_pulse(elapsed: f32, reduced: bool) -> f32 {
	if reduced || elapsed.is_nan() {
		return 0.;
	}
	0.4 * (1. - elapsed / LANDING_DURATION).clamp(0., 1.)
}

/// Delay in seconds before the row at `index` starts. Rows past the eighth share its delay.
/// Opening uses `step` per row; closing is twice as tight, in the ratio of [`STAGGER_OUT`] to
/// [`STAGGER_IN`].
pub fn stagger_delay(index: usize, open: bool, step: f32, reduced: bool) -> f32 {
	if reduced {
		return 0.;
	}
	let step = if open {
		step
	} else {
		step * (STAGGER_OUT / STAGGER_IN)
	};
	index.min(STAGGER_CAP) as f32 * step
}

/// `(width, height)` scale of a knob or segment moving at `velocity` units per second: wider by up
/// to 22% and shorter by up to 6% at full speed (8 units per second).
pub fn stretch(velocity: f32, reduced: bool) -> (f32, f32) {
	if reduced || velocity.is_nan() {
		return (1., 1.);
	}
	let s = (velocity.abs() / FULL_STRETCH_SPEED).clamp(0., 1.);
	(1. + 0.22 * s, 1. - 0.06 * s)
}

#[cfg(test)]
mod tests {
	use super::*;

	fn close(a: f32, b: f32) -> bool {
		(a - b).abs() < 1e-5
	}

	#[test]
	fn squash_only_acts_below_rest_and_is_clamped() {
		assert_eq!(squash(0., false), 1.);
		assert_eq!(squash(0.5, false), 1.);
		assert!(close(squash(-0.02, false), 0.97));
		assert_eq!(squash(-0.06, false), SQUASH_MIN);
		assert_eq!(squash(-5., false), SQUASH_MIN);
		assert_eq!(squash(-0.03, true), 1.);
	}

	#[test]
	fn speed_factor_saturates_at_six() {
		assert_eq!(speed_factor(0.), 0.);
		assert!(close(speed_factor(-3.), 0.5));
		assert_eq!(speed_factor(60.), 1.);
	}

	#[test]
	fn glass_boost_scales_with_speed() {
		assert_eq!(glass_boost(0., false), GlassBoost::NONE);
		let full = glass_boost(1., false);
		assert!(close(full.brightness, 0.06) && close(full.specular, 0.35) && close(full.refraction, 1.25));
		let half = glass_boost(0.5, false);
		assert!(close(half.brightness, 0.03) && close(half.refraction, 1.125));
		assert_eq!(glass_boost(1., true), GlassBoost::NONE);
		assert_eq!(glass_boost(9., false), full);
	}

	#[test]
	fn landing_pulse_fades_over_180_ms() {
		assert!(close(landing_pulse(0., false), 0.4));
		assert!(close(landing_pulse(0.09, false), 0.2));
		assert_eq!(landing_pulse(0.18, false), 0.);
		assert_eq!(landing_pulse(2., false), 0.);
		assert_eq!(landing_pulse(0., true), 0.);
	}

	#[test]
	fn stagger_caps_at_eight_rows_and_closes_faster() {
		assert_eq!(stagger_delay(0, true, 0.024, false), 0.);
		assert!(close(stagger_delay(3, true, 0.024, false), 0.072));
		assert_eq!(
			stagger_delay(8, true, 0.024, false),
			stagger_delay(40, true, 0.024, false)
		);
		assert!(close(stagger_delay(4, false, 0.024, false), 0.048));
		assert_eq!(stagger_delay(5, true, 0.024, true), 0.);
	}

	#[test]
	fn stretch_widens_and_flattens_with_speed() {
		assert_eq!(stretch(0., false), (1., 1.));
		let (w, h) = stretch(-8., false);
		assert!(close(w, 1.22) && close(h, 0.94));
		assert_eq!(stretch(80., false), stretch(8., false));
		let (w, h) = stretch(4., false);
		assert!(close(w, 1.11) && close(h, 0.97));
		assert_eq!(stretch(8., true), (1., 1.));
	}
}
