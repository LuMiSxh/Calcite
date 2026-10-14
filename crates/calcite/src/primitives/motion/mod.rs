// SPDX-License-Identifier: MPL-2.0

//! Calcite's motion system. Views pick a semantic [`Curve`] or a named spring token, never a
//! raw duration.
//!
//! # Springs
//!
//! A [`Spring`] is a `response` (perceptual duration in seconds) and a `bounce` (0 = no
//! overshoot). [`spring()`] drives a value with one and keeps its velocity when the target changes
//! mid-motion, so interrupting a hover with a press bends the path instead of restarting it.
//!
//! | Token         | Response | Bounce | Overshoot | Use                                        |
//! |---------------|----------|--------|-----------|--------------------------------------------|
//! | `PRESS`       | 0.16     | 0.00   | 0.0%      | held-down response                         |
//! | `SNAPPY`      | 0.30     | 0.15   | 0.6%      | small controls snapping between states     |
//! | `SMOOTH`      | 0.45     | 0.00   | 0.0%      | calm travel: sheets, sliders, fills        |
//! | `BOUNCY`      | 0.45     | 0.32   | 5.4%      | toggles, toasts, pop-ins                   |
//! | `GENTLE`      | 0.60     | 0.10   | 0.2%      | large surfaces settling                    |
//! | `JELLY`       | 0.42     | 0.45   | 12.6%     | glass that stretches and recovers          |
//! | `MORPH_OPEN`  | 0.48     | 0.30   | 4.6%      | glass growing into a larger shape          |
//! | `MORPH_CLOSE` | 0.34     | 0.38   | 8.4%      | glass folding back, attached closing       |
//! | `MORPH_FOLD`  | 0.34     | 0.04   | 0.0%      | an attached body folding into its trigger  |
//! | `SELECT`      | 0.40     | 0.28   | 3.8%      | selection moving between options           |
//! | `TOOLTIP`     | 0.22     | 0.10   | 0.2%      | tooltips and tiny hints                    |
//! | `EXIT`        | 0.20     | 0.00   | 0.0%      | things leaving                             |
//! | `TICKER`      | 0.34     | 0.30   | 4.6%      | rolling digits                             |
//!
//! Where they are used: `PRESS` and `JELLY` are the press and release of every control
//! (`interaction`); `SNAPPY` and `EXIT` drive menu rows opening and closing; `SELECT` the segmented
//! indicator; `TOOLTIP` the tooltip enter; `TICKER` the ticker digits; `MORPH_*` the pill and the
//! lab demo. Toggles use `Curve::Release`, and attached bodies `Curve::Present` / `MORPH_CLOSE`.
//!
//! Stagger steps are [`STAGGER_IN`] 24 ms and [`STAGGER_OUT`] 12 ms; menu rows cascade on them
//! (at most 8 rows) and `Curve::stagger` uses `STAGGER_IN`. The other semantic [`Curve`]s keep their
//! own numbers ([`Curve::spring`]).
//!
//! # Secondary motion
//!
//! [`secondary`] holds pure helpers for effects that follow the main spring: squash, stretch,
//! glass reacting to speed, a landing pulse, and staggered delays. All return the identity under
//! Reduce Motion.

pub mod secondary;
pub mod spring;
pub mod tokens;
pub mod value;

use std::time::Duration;

use gpui::{
	Animation, AnimationElement, AnimationExt, AnyMotion, ElementId, Filter, Motion, MotionDurationExt,
	SpringConfig, Styled, ease_in_out, linear, px,
};

pub use self::spring::Spring;
pub use self::tokens::{STAGGER_IN, STAGGER_OUT};
pub use self::value::{SpringSample, SpringState, spring, spring_from};

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
	/// Popups, overlays, and HUD pill expansion.
	Present,
	/// Popups and overlays leaving; faster than presentation.
	Dismiss,
	/// Artwork, wordmark, and theme crossfades.
	Crossfade,
	/// Selection changes in lists and segmented controls.
	Select,
	/// Determinate progress fill changes.
	Progress,
	/// Inline detail content opening or closing.
	Disclosure,
}

impl Curve {
	pub const ALL: [Self; 10] = [
		Self::Press,
		Self::Release,
		Self::Hover,
		Self::State,
		Self::Present,
		Self::Dismiss,
		Self::Crossfade,
		Self::Select,
		Self::Progress,
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
	Spring::new(duration, bounce).to_config()
}

enum Kind {
	Spring(Spring),
	Eased { duration: f32, easing: fn(f32) -> f32 },
}

impl Curve {
	fn kind(self) -> Kind {
		use Kind::*;
		match self {
			Self::Press => Spring(self::spring::Spring::new(0.20, 0.)),
			Self::Release => Spring(self::spring::Spring::new(0.46, 0.42)),
			Self::Hover => Spring(self::spring::Spring::new(0.32, 0.22)),
			Self::State => Spring(self::spring::Spring::new(0.38, 0.16)),
			Self::Present => Spring(self::spring::Spring::new(0.50, 0.24)),
			Self::Dismiss => Spring(self::spring::Spring::new(0.24, 0.)),
			Self::Crossfade => Eased {
				duration: 0.48,
				easing: ease_in_out,
			},
			Self::Select => Eased {
				duration: 0.16,
				easing: ease_in_out,
			},
			Self::Progress => Eased {
				duration: 0.20,
				easing: linear,
			},
			Self::Disclosure => Spring(tokens::SNAPPY),
		}
	}

	/// The spring behind this curve, or `None` for the timed (eased) curves.
	pub fn spring(self) -> Option<Spring> {
		match self.kind() {
			Kind::Spring(spring) => Some(spring),
			Kind::Eased { .. } => None,
		}
	}

	/// Motion for declarative style transitions (`.transitions(...)`). gpui's `Animated::set`
	/// keeps the position when a hover or press retargets one, but restarts the spring from
	/// rest, so velocity is lost; use [`spring`](fn@spring) for values that must keep it.
	pub fn transition(self, reduce_motion: bool) -> AnyMotion {
		if reduce_motion {
			return Duration::ZERO.into();
		}
		match self.kind() {
			Kind::Spring(spring) => spring.to_config().into(),
			Kind::Eased { duration, easing } => Duration::from_secs_f32(duration).with_easing(easing).into(),
		}
	}

	/// How long the curve runs; springs report their settle time.
	pub fn duration(self) -> Duration {
		match self.kind() {
			Kind::Spring(spring) => Duration::from_secs_f32(spring.response * 1.6),
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

	/// The delay of the sibling at `index` when siblings enter together, such as HUD pills:
	/// [`STAGGER_IN`] (24 ms) per sibling.
	pub fn stagger(index: usize, reduce_motion: bool) -> Duration {
		if reduce_motion {
			return Duration::ZERO;
		}
		Duration::from_secs_f32(index as f32 * STAGGER_IN)
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
			Kind::Spring(spring) => timed_spring(spring),
			Kind::Eased { duration, easing } => Duration::from_secs_f32(duration).with_easing(easing),
		}
	}
}

/// Motion for a one-shot `with_animation` driven by `spring`, sampled over its settle time.
/// SwiftUI's `duration` is the perceptual duration; the tail settles later.
fn timed_spring(spring: Spring) -> Motion {
	let settle = Duration::from_secs_f32(spring.response * 1.6);
	settle.with_spring(spring.to_config())
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

/// How far entering content rises into place.
const ENTER_RISE: f32 = 8.;
/// The blur radius entering content starts with; it clears as the content arrives.
const ENTER_BLUR: f32 = 4.;

/// The blur (px) of entering content at progress `t`: `ENTER_BLUR` at 0, gone once `t` reaches
/// 1, and none under Reduce Motion. Callers add the filter only while this is above 0, so settled
/// content never pays for an offscreen blur pass.
pub fn enter_blur(t: f32, reduce_motion: bool) -> f32 {
	if reduce_motion || t.is_nan() {
		return 0.;
	}
	ENTER_BLUR * (1. - t).clamp(0., 1.)
}

/// Content that materializes when it first renders (`AnimationExt.materialize` and
/// `LauncherMotion.staggered` in the launcher).
pub trait EnterExt: AnimationExt + Styled + Sized + 'static {
	/// Fades and rises into place on [`Curve::Present`]. Siblings that enter together pass
	/// their `index`, so each one starts [`Curve::stagger`] later. Reduce Motion keeps a
	/// short fade only.
	fn enter(self, id: impl Into<ElementId>, index: usize, reduce_motion: bool) -> AnimationElement<Self> {
		let spring = Curve::Present.spring().expect("Present is a spring");
		self.enter_with(id, index, spring, reduce_motion)
	}

	/// [`enter`](Self::enter) on a chosen `spring`, such as [`tokens::TOOLTIP`] for a hint.
	fn enter_with(
		self,
		id: impl Into<ElementId>,
		index: usize,
		spring: Spring,
		reduce_motion: bool,
	) -> AnimationElement<Self> {
		let delay = Curve::stagger(index, reduce_motion);
		let mut animations = Vec::with_capacity(2);
		if !delay.is_zero() {
			animations.push(Animation::new(delay.with_easing(linear)));
		}
		let main = if reduce_motion {
			Duration::from_secs_f32(0.15).with_easing(ease_in_out)
		} else {
			timed_spring(spring)
		};
		animations.push(Animation::new(main));
		let waits = animations.len() == 2;
		self.with_animations(id, animations, move |element, phase, t| {
			let t = if waits && phase == 0 { 0. } else { t };
			let shown = Materialize::at(t, reduce_motion);
			let rise = if reduce_motion { 0. } else { ENTER_RISE * (1. - t) };
			let element = element.opacity(shown.opacity.clamp(0., 1.)).mt(px(rise));
			match enter_blur(t, reduce_motion) {
				blur if blur > 0.01 => element.filter(vec![Filter::Blur(px(blur))]),
				_ => element,
			}
		})
	}
}

impl<E: AnimationExt + Styled + Sized + 'static> EnterExt for E {}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn siblings_enter_one_stagger_step_apart() {
		assert_eq!(Curve::stagger(0, false), Duration::ZERO);
		assert_eq!(Curve::stagger(1, false), Duration::from_secs_f32(STAGGER_IN));
		assert_eq!(Curve::stagger(3, false), Duration::from_secs_f32(3. * STAGGER_IN));
		assert_eq!(Curve::stagger(3, true), Duration::ZERO);
	}

	#[test]
	fn entering_blur_clears_by_the_end_and_never_returns() {
		assert_eq!(enter_blur(0., false), 4.);
		assert!((enter_blur(0.5, false) - 2.).abs() < 1e-6);
		assert_eq!(enter_blur(1., false), 0.);
		// A spring that overshoots past 1 stays sharp.
		assert_eq!(enter_blur(1.04, false), 0.);
		assert_eq!(enter_blur(0., true), 0.);
	}
}
