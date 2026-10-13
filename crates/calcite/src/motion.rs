// SPDX-License-Identifier: MPL-2.0

//! Port of `LauncherMotion.swift`. Views pick a semantic curve, never a raw duration.

use std::time::Duration;

use gpui::{AnyMotion, Motion, MotionDurationExt, SpringConfig, ease_in_out, linear, spring};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Curve {
	/// Immediate, critically damped response while a control is held down.
	Press,
	/// Springy settle after a press ends.
	Release,
	/// Hover lift and pointer highlights.
	Hover,
	/// Structural layout changes inside a visible surface.
	State,
	/// Primary action shape and label morphing between operations.
	Morph,
	/// Popups, overlays, and HUD pill expansion.
	Present,
	/// Popups and overlays leaving; faster than presentation.
	Dismiss,
	/// Artwork, wordmark, and theme crossfades.
	Crossfade,
	/// Selection changes in lists and segmented controls.
	Select,
	/// Page and step transitions.
	Page,
	/// Determinate progress fill changes.
	Progress,
	/// The dimming backdrop behind popups.
	Backdrop,
	/// The expanding ripple on completion.
	Ripple,
	/// A quick ease-in as an element settles out.
	Settle,
	/// Inline detail content opening or closing.
	Disclosure,
}

impl Curve {
	pub const ALL: [Self; 15] = [
		Self::Press,
		Self::Release,
		Self::Hover,
		Self::State,
		Self::Morph,
		Self::Present,
		Self::Dismiss,
		Self::Crossfade,
		Self::Select,
		Self::Page,
		Self::Progress,
		Self::Backdrop,
		Self::Ripple,
		Self::Settle,
		Self::Disclosure,
	];
}

/// SwiftUI `easeOut`: a cubic that starts fast and slows into place.
pub fn ease_out(t: f32) -> f32 {
	1. - (1. - t).powi(3)
}

/// SwiftUI `easeIn`: a cubic that starts slow and leaves fast.
pub fn ease_in(t: f32) -> f32 {
	t.powi(3)
}

/// SwiftUI `spring(duration:bounce:)` with mass 1:
/// stiffness = (2π / duration)², damping = 4π (1 − bounce) / duration.
pub fn swiftui_spring(duration: f32, bounce: f32) -> SpringConfig {
	let tau = std::f32::consts::TAU;
	let stiffness = (tau / duration).powi(2);
	let damping = 2. * tau * (1. - bounce) / duration;
	spring(stiffness, damping, 1.)
}

enum Kind {
	Spring { duration: f32, bounce: f32 },
	Eased { duration: f32, easing: fn(f32) -> f32 },
}

impl Curve {
	fn kind(self) -> Kind {
		use Kind::*;
		match self {
			Self::Press => Spring {
				duration: 0.20,
				bounce: 0.,
			},
			Self::Release => Spring {
				duration: 0.46,
				bounce: 0.42,
			},
			Self::Hover => Spring {
				duration: 0.32,
				bounce: 0.22,
			},
			Self::State => Spring {
				duration: 0.38,
				bounce: 0.16,
			},
			Self::Morph => Spring {
				duration: 0.52,
				bounce: 0.30,
			},
			Self::Present => Spring {
				duration: 0.50,
				bounce: 0.24,
			},
			Self::Dismiss => Spring {
				duration: 0.24,
				bounce: 0.,
			},
			Self::Crossfade => Eased {
				duration: 0.48,
				easing: ease_in_out,
			},
			Self::Select => Eased {
				duration: 0.16,
				easing: ease_in_out,
			},
			Self::Page => Eased {
				duration: 0.22,
				easing: ease_in_out,
			},
			Self::Progress => Eased {
				duration: 0.20,
				easing: linear,
			},
			Self::Backdrop => Eased {
				duration: 0.24,
				easing: ease_out,
			},
			Self::Ripple => Eased {
				duration: 0.9,
				easing: ease_out,
			},
			Self::Settle => Eased {
				duration: 0.18,
				easing: ease_in,
			},
			// SwiftUI `snappy` is a spring with a small bounce.
			Self::Disclosure => Spring {
				duration: 0.3,
				bounce: 0.15,
			},
		}
	}

	/// Motion for declarative style transitions (`.transitions(...)`). These keep their
	/// velocity when a hover or press retargets them, like SwiftUI springs.
	pub fn transition(self, reduce_motion: bool) -> AnyMotion {
		if reduce_motion {
			return Duration::ZERO.into();
		}
		match self.kind() {
			Kind::Spring { duration, bounce } => swiftui_spring(duration, bounce).into(),
			Kind::Eased { duration, easing } => Duration::from_secs_f32(duration).with_easing(easing).into(),
		}
	}

	/// How long the curve runs; springs report their settle time.
	pub fn duration(self) -> Duration {
		match self.kind() {
			Kind::Spring { duration, .. } => Duration::from_secs_f32(duration * 1.6),
			Kind::Eased { duration, .. } => Duration::from_secs_f32(duration),
		}
	}

	/// Progress from 0 to 1 after `elapsed`, for values a view samples itself (colors,
	/// theme transitions). Springs ease in and out over their settle time.
	pub fn progress(self, elapsed: Duration) -> f32 {
		let t = (elapsed.as_secs_f32() / self.duration().as_secs_f32()).clamp(0., 1.);
		match self.kind() {
			Kind::Eased { easing, .. } => easing(t),
			Kind::Spring { .. } => ease_in_out(t),
		}
	}

	/// The delay of the sibling at `index` when siblings enter together, such as HUD pills.
	pub fn stagger(index: usize, reduce_motion: bool) -> Duration {
		if reduce_motion {
			return Duration::ZERO;
		}
		Duration::from_secs_f32(index as f32 * STAGGER_STEP)
	}

	/// A quiet fade that keeps a state change legible under Reduce Motion
	/// (`LauncherMotion.fade`).
	pub fn fade(reduce_motion: bool) -> AnyMotion {
		if reduce_motion {
			Duration::from_secs_f32(0.15).with_easing(ease_in_out).into()
		} else {
			Self::State.transition(false)
		}
	}

	/// Motion for a one-shot `with_animation`, sampled over the spring's settle time.
	pub fn timed(self, reduce_motion: bool) -> Motion {
		if reduce_motion {
			return Duration::from_millis(1).with_easing(linear);
		}
		match self.kind() {
			Kind::Spring { duration, bounce } => {
				// SwiftUI's `duration` is the perceptual duration; the tail settles later.
				let settle = Duration::from_secs_f32(duration * 1.6);
				settle.with_spring(swiftui_spring(duration, bounce))
			}
			Kind::Eased { duration, easing } => Duration::from_secs_f32(duration).with_easing(easing),
		}
	}
}

/// Looping and one-shot durations shared by decorative motion (`LauncherMotion.Cycle`).
pub mod cycle {
	/// Period of the halo pulse and the indeterminate pulse.
	pub const BREATHING: f32 = 3.2;
	/// Length of the completion feedback.
	pub const COMPLETION_FEEDBACK: f32 = 0.4;
	pub const PROGRESS_SWEEP_FADE_IN: f32 = 0.12;
	pub const PROGRESS_SWEEP_TRAVEL: f32 = 1.40;
	pub const PROGRESS_SWEEP_FADE_OUT: f32 = 0.25;
	pub const PROGRESS_SWEEP_PAUSE: f32 = 0.55;
	pub const INDETERMINATE_ORBIT_LAPS: f32 = 2.;
	pub const INDETERMINATE_GLINT_LENGTH: f32 = 0.14;

	pub const fn progress_sweep_total() -> f32 {
		PROGRESS_SWEEP_FADE_IN + PROGRESS_SWEEP_TRAVEL + PROGRESS_SWEEP_FADE_OUT + PROGRESS_SWEEP_PAUSE
	}
}

/// Delay in seconds between siblings that enter together.
const STAGGER_STEP: f32 = 0.045;

/// How content materializes instead of popping in (`AnyTransition.materialize`): blur,
/// scale, and opacity at a progress from 0 (hidden) to 1 (shown). Reduce Motion keeps only
/// the opacity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Materialize {
	pub opacity: f32,
	pub scale: f32,
	pub blur: f32,
}

impl Materialize {
	pub fn at(progress: f32, reduce_motion: bool) -> Self {
		let t = progress.clamp(0., 1.);
		if reduce_motion {
			return Self {
				opacity: t,
				scale: 1.,
				blur: 0.,
			};
		}
		Self {
			opacity: t,
			scale: lerp(0.94, 1., t),
			blur: lerp(6., 0., t),
		}
	}
}

/// `ContinuousMotion` updates at most this often.
pub const DECORATIVE_FPS: f32 = 30.;

pub fn lerp(from: f32, to: f32, t: f32) -> f32 {
	from + (to - from) * t
}
