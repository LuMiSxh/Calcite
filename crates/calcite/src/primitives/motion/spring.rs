// SPDX-License-Identifier: MPL-2.0

//! The spring description every Calcite motion is built from.

use gpui::SpringConfig;

/// A spring described the way SwiftUI does: how fast it responds and how much it bounces.
///
/// `response` is the perceptual duration in seconds (the period of the undamped oscillation),
/// `bounce` runs from 0 (critically damped, no overshoot) towards 1 (never settles). The physical
/// spring has mass 1, stiffness `(2π / response)²` and damping `4π (1 − bounce) / response`, so
/// the damping ratio is `1 − bounce`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
	pub response: f32,
	pub bounce: f32,
}

impl Spring {
	pub const fn new(response: f32, bounce: f32) -> Self {
		Self { response, bounce }
	}

	/// A spring that eases into place without overshooting.
	pub const fn smooth(response: f32) -> Self {
		Self::new(response, 0.)
	}

	/// A spring that overshoots its target by a little.
	pub const fn bouncy(response: f32, bounce: f32) -> Self {
		Self::new(response, bounce)
	}

	/// The physical spring: mass 1, stiffness `(2π / response)²`, damping `4π (1 − bounce) / response`.
	pub fn to_config(self) -> SpringConfig {
		let tau = std::f32::consts::TAU;
		let stiffness = (tau / self.response).powi(2);
		let damping = 2. * tau * (1. - self.bounce) / self.response;
		SpringConfig::new(stiffness, damping, 1.)
	}

	/// How long the spring takes to settle, in seconds: 1.6 response times. One-shot timings and
	/// [`landing`](super::value::landing) use it.
	pub fn settle_time(self) -> f32 {
		self.response * 1.6
	}

	/// The damping ratio, `1 − bounce`.
	pub fn damping_ratio(self) -> f32 {
		1. - self.bounce
	}

	/// The first overshoot of a step response as a fraction of the step, in closed form:
	/// `exp(−π ζ / √(1 − ζ²))` for an underdamped spring, 0 at or above critical damping.
	/// A spring with no damping (`bounce` 1) never settles and reports 1.
	pub fn overshoot(self) -> f32 {
		let zeta = self.damping_ratio();
		if zeta >= 1. {
			0.
		} else if zeta <= 0. {
			1.
		} else {
			(-std::f32::consts::PI * zeta / (1. - zeta * zeta).sqrt()).exp()
		}
	}
}

#[cfg(test)]
mod tests {
	use gpui::SpringState;

	use super::*;

	#[test]
	fn config_uses_the_swiftui_mapping() {
		let config = Spring::new(0.5, 0.25).to_config();
		let tau = std::f32::consts::TAU;
		assert_eq!(config.mass, 1.);
		assert!((config.stiffness - (tau / 0.5).powi(2)).abs() < 1e-3);
		assert!((config.damping - 2. * tau * 0.75 / 0.5).abs() < 1e-3);
		let (_, ratio) = config.canonical();
		assert!((ratio - 0.75).abs() < 1e-5);
	}

	#[test]
	fn critically_damped_springs_do_not_overshoot() {
		assert_eq!(Spring::smooth(0.3).overshoot(), 0.);
		assert_eq!(Spring::new(0.3, -0.2).overshoot(), 0.);
	}

	#[test]
	fn undamped_springs_report_full_overshoot() {
		assert_eq!(Spring::new(0.3, 1.).overshoot(), 1.);
	}

	#[test]
	fn overshoot_matches_the_simulated_step_response() {
		for spring in [
			Spring::new(0.3, 0.15),
			Spring::new(0.45, 0.32),
			Spring::new(0.42, 0.45),
		] {
			let config = spring.to_config();
			let mut state = SpringState {
				position: 0.,
				velocity: 0.,
			};
			let mut peak = 0.0_f32;
			for _ in 0..4000 {
				state = config.step(state, 1., 1. / 2000.);
				peak = peak.max(state.position);
			}
			assert!(
				(peak - 1. - spring.overshoot()).abs() < 2e-3,
				"{spring:?}: simulated {} closed form {}",
				peak - 1.,
				spring.overshoot()
			);
		}
	}
}
