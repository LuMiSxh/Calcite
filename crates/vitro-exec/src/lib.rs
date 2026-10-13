// SPDX-License-Identifier: MPL-2.0

//! The smallest execution layer that keeps asynchronous work honest in a gpui-ce view.
//!
//! It adds no runtime of its own: work runs as ordinary GPUI tasks owned by a view, and
//! results reach state only through `Entity::update`. Each type maps one-to-one to a
//! Swift counterpart in Arknights Client, so a feature reads the same on both sides:
//!
//! | Rust                                   | Swift                                        |
//! | -------------------------------------- | -------------------------------------------- |
//! | [`TaskSlot`], [`Job`]                  | `TaskSlot` (`replace`, `owns`, `invalidate`) |
//! | [`ActivityStore`], [`ActivityLease`]   | `LauncherLifecycleStore`, `ActivityLease`    |
//! | [`Throttle`]                           | progress throttling to about 10 Hz           |
//! | [`bounded`]                            | windowed task group with an `AppConstants` limit |
//!
//! Rules, as in the Swift code: capture a token or lease before every `await` and check it
//! after; cancellation alone is not ownership. [`Job`] does the check on every publish, and
//! `?` on [`Stale`] ends replaced work quietly. Call [`TaskSlot::invalidate`] before you
//! clear or replace the state that work would publish into.
//!
//! ```ignore
//! use vitro_exec::prelude::*;
//!
//! fn start(&mut self, cx: &mut Context<Self>) {
//!     let Some(lease) = self.activity.begin(Activity::Downloading) else { return };
//!     self.download.run(cx, |view| &mut view.download, async move |job| {
//!         job.guard(move |view: &Self| view.activity.holds(lease));
//!         let mut throttle = Throttle::progress();
//!         for step in 1..=100 {
//!             job.sleep(Duration::from_millis(20)).await;
//!             job.update_throttled(&mut throttle, |view, cx| {
//!                 view.progress = step as f32 / 100.;
//!                 cx.notify();
//!             })?;
//!         }
//!         job.update(|view, cx| {
//!             view.activity.end(lease);
//!             cx.notify();
//!         })
//!     });
//! }
//! ```

mod activity;
mod bounded;
mod slot;
mod throttle;

pub use activity::{ActivityLease, ActivityStore};
pub use bounded::bounded;
pub use slot::{Job, JobResult, SlotSelector, SlotToken, Stale, TaskSlot};
pub use throttle::Throttle;

/// Everything a view needs to run work: `use vitro_exec::prelude::*;`.
pub mod prelude {
	pub use crate::{ActivityLease, ActivityStore, Job, JobResult, Stale, TaskSlot, Throttle, bounded};
}

#[cfg(test)]
mod tests {
	use std::{
		cell::Cell,
		rc::Rc,
		time::{Duration, Instant},
	};

	use gpui::AppContext as _;

	use super::*;

	#[test]
	fn replaced_and_invalidated_tokens_are_stale() {
		let mut slot = TaskSlot::new();
		let first = slot.begin();
		let second = slot.begin();
		assert!(!slot.owns(first));
		assert!(slot.owns(second));
		slot.invalidate();
		assert!(!slot.owns(second));
	}

	#[derive(Clone, Default, PartialEq, Debug)]
	enum Activity {
		#[default]
		Idle,
		Installing,
		Launching,
	}

	#[test]
	fn activity_is_exclusive_and_a_stale_lease_never_sets_idle() {
		let mut store = ActivityStore::<Activity>::new();
		let install = store.begin(Activity::Installing).unwrap();
		assert_eq!(store.begin(Activity::Launching), None);
		assert!(store.end(install));
		let launch = store.begin(Activity::Launching).unwrap();
		assert!(!store.update(install, Activity::Installing));
		assert!(!store.end(install));
		assert_eq!(store.activity(), &Activity::Launching);
		assert!(store.end(launch));
		assert!(store.is_idle());
	}

	#[test]
	fn throttle_admits_one_value_per_interval() {
		let mut throttle = Throttle::progress();
		let start = Instant::now();
		let admitted = (0..20)
			.filter(|tick| throttle.admit(start + Duration::from_millis(25 * tick)))
			.count();
		assert_eq!(admitted, 5);
	}

	/// Suspends once and wakes itself, so other futures in the window get polled.
	async fn yield_now() {
		let mut yielded = false;
		futures::future::poll_fn(|cx| {
			if yielded {
				return std::task::Poll::Ready(());
			}
			yielded = true;
			cx.waker().wake_by_ref();
			std::task::Poll::Pending
		})
		.await
	}

	#[test]
	fn bounded_never_exceeds_its_limit() {
		let running = Rc::new(Cell::new(0));
		let peak = Rc::new(Cell::new(0));
		let results = futures::executor::block_on(bounded(3, 0..12, |item| {
			let running = running.clone();
			let peak = peak.clone();
			async move {
				running.set(running.get() + 1);
				peak.set(peak.get().max(running.get()));
				yield_now().await;
				running.set(running.get() - 1);
				item
			}
		}));
		assert_eq!(results.len(), 12);
		assert_eq!(peak.get(), 3);
	}

	struct Counter {
		slot: TaskSlot,
		value: u32,
	}

	#[gpui::test]
	async fn replaced_work_cannot_publish_and_the_slot_turns_idle(cx: &mut gpui::TestAppContext) {
		let view = cx.new(|_| Counter {
			slot: TaskSlot::new(),
			value: 0,
		});
		view.update(cx, |counter, cx| {
			counter.slot.run(
				cx,
				|counter| &mut counter.slot,
				async move |job| {
					job.sleep(Duration::from_millis(10)).await;
					job.update(|counter, _| counter.value = 1)
				},
			);
		});
		view.update(cx, |counter, cx| {
			counter.slot.run(
				cx,
				|counter| &mut counter.slot,
				async move |job| {
					job.sleep(Duration::from_millis(20)).await;
					job.update(|counter, _| counter.value = 2)
				},
			);
		});
		cx.executor().advance_clock(Duration::from_millis(50));
		cx.run_until_parked();
		view.read_with(cx, |counter, _| {
			assert_eq!(counter.value, 2);
			assert!(!counter.slot.is_running());
		});
	}

	#[gpui::test]
	async fn a_failed_guard_stops_publishing(cx: &mut gpui::TestAppContext) {
		#[derive(Clone, Default, PartialEq)]
		enum Work {
			#[default]
			Idle,
			Busy,
		}
		struct View {
			slot: TaskSlot,
			activity: ActivityStore<Work>,
			published: bool,
		}
		let view = cx.new(|_| View {
			slot: TaskSlot::new(),
			activity: ActivityStore::new(),
			published: false,
		});
		view.update(cx, |view, cx| {
			let lease = view.activity.begin(Work::Busy).unwrap();
			view.slot.run(
				cx,
				|view| &mut view.slot,
				async move |job| {
					job.guard(move |view: &View| view.activity.holds(lease));
					job.sleep(Duration::from_millis(10)).await;
					job.update(|view, _| view.published = true)
				},
			);
			view.activity.end(lease);
		});
		cx.executor().advance_clock(Duration::from_millis(20));
		cx.run_until_parked();
		view.read_with(cx, |view, _| assert!(!view.published));
	}
}
