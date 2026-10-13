// SPDX-License-Identifier: MPL-2.0

//! The smallest execution layer that keeps asynchronous work honest in a gpui-ce view.
//!
//! It adds no runtime of its own: work runs as ordinary GPUI tasks owned by a view, and
//! results reach state only through `Entity::update`. Each type maps one-to-one to a
//! Swift counterpart in Arknights Client, so a feature reads the same on both sides:
//!
//! | Rust              | Swift                                  | Contract                                         |
//! | ----------------- | -------------------------------------- | ------------------------------------------------ |
//! | [`TaskSlot`]      | `TaskSlot`                             | Replacing work cancels it; stale results lose.   |
//! | [`ActivityStore`] | `LauncherLifecycleStore` + `ActivityLease` | One exclusive activity; a stale lease changes nothing. |
//! | [`Throttle`]      | progress throttling (~10 Hz)           | High-frequency values reach the UI at a bound rate. |
//! | [`bounded`]       | windowed task group + `AppConstants` limit | Seed N, add one per completion; never unbounded. |
//!
//! Rules, as in the Swift code: capture a token or lease before every `await` and check it
//! after; cancellation alone is not ownership. Call [`TaskSlot::invalidate`] before you clear
//! or replace the state that work would publish into.

use std::{
	future::Future,
	time::{Duration, Instant},
};

use futures::{StreamExt, stream};
use gpui::{AsyncApp, Context, Task, WeakEntity};

// MARK: - TaskSlot

/// Identity of one run of a [`TaskSlot`]. Compare it with [`TaskSlot::owns`] after each
/// suspension before publishing anything.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlotToken(u64);

/// Owns at most one in-flight task and rejects results from replaced or invalidated work.
#[derive(Default)]
pub struct TaskSlot {
	task: Option<Task<()>>,
	current: u64,
	active: bool,
}

impl TaskSlot {
	pub fn new() -> Self {
		Self::default()
	}

	/// Cancels the running task and returns a fresh token for work the caller runs itself.
	pub fn begin(&mut self) -> SlotToken {
		// Dropping a GPUI task cancels it at its next suspension point.
		self.task = None;
		self.current += 1;
		self.active = true;
		SlotToken(self.current)
	}

	/// Cancels the running task and starts `operation` under a fresh token. The task lives as
	/// long as the slot, so dropping the view also cancels it.
	pub fn replace<V, F>(&mut self, cx: &mut Context<V>, operation: F) -> SlotToken
	where
		V: 'static,
		F: AsyncFnOnce(SlotToken, WeakEntity<V>, &mut AsyncApp) + 'static,
	{
		let token = self.begin();
		self.task = Some(cx.spawn(async move |this, cx| operation(token, this, cx).await));
		token
	}

	pub fn owns(&self, token: SlotToken) -> bool {
		self.active && self.current == token.0
	}

	pub fn is_running(&self) -> bool {
		self.active && self.task.is_some()
	}

	/// Marks the slot idle once its work finished normally. A stale token changes nothing.
	pub fn finish(&mut self, token: SlotToken) -> bool {
		if !self.owns(token) {
			return false;
		}
		self.active = false;
		self.task = None;
		true
	}

	/// Cancels the running task and makes every outstanding token stale.
	pub fn invalidate(&mut self) {
		self.task = None;
		self.active = false;
		self.current += 1;
	}
}

/// Publishes into the view only while `token` still owns the slot that `slot` selects.
/// Returns `false` when the view is gone or the work was replaced or invalidated.
pub fn publish<V: 'static>(
	this: &WeakEntity<V>,
	cx: &mut AsyncApp,
	slot: impl FnOnce(&V) -> &TaskSlot,
	token: SlotToken,
	update: impl FnOnce(&mut V, &mut Context<V>),
) -> bool {
	this.update(cx, |view, cx| {
		if !slot(view).owns(token) {
			return false;
		}
		update(view, cx);
		true
	})
	.unwrap_or(false)
}

// MARK: - Activity leases

/// Proof that one operation owns the exclusive activity. The store rejects any change that
/// carries a lease it did not issue last.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ActivityLease(u64);

/// One exclusive activity at a time (install, launch, run, stop...). `A::default()` is idle.
#[derive(Debug, Default)]
pub struct ActivityStore<A> {
	activity: A,
	lease: Option<ActivityLease>,
	issued: u64,
}

impl<A: Clone + Default + PartialEq> ActivityStore<A> {
	pub fn new() -> Self {
		Self {
			activity: A::default(),
			lease: None,
			issued: 0,
		}
	}

	pub fn activity(&self) -> &A {
		&self.activity
	}

	pub fn is_idle(&self) -> bool {
		self.lease.is_none()
	}

	/// Claims the exclusive activity. Returns `None` while another activity runs.
	pub fn begin(&mut self, activity: A) -> Option<ActivityLease> {
		if self.lease.is_some() {
			return None;
		}
		self.issued += 1;
		let lease = ActivityLease(self.issued);
		self.lease = Some(lease);
		self.activity = activity;
		Some(lease)
	}

	/// Changes the activity for the lease holder. A stale lease changes nothing.
	pub fn update(&mut self, lease: ActivityLease, activity: A) -> bool {
		if self.lease != Some(lease) {
			return false;
		}
		self.activity = activity;
		true
	}

	/// Releases the activity for the lease holder. A stale lease never sets idle.
	pub fn end(&mut self, lease: ActivityLease) -> bool {
		if self.lease != Some(lease) {
			return false;
		}
		self.lease = None;
		self.activity = A::default();
		true
	}
}

// MARK: - Throttle

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

	/// Whether a value produced at `now` should reach the UI. Final values bypass it via
	/// [`Throttle::force`].
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

// MARK: - Bounded parallelism

/// Runs `work` for every item with at most `limit` futures in flight: it seeds `limit` and
/// starts one more per completion. Results arrive in completion order.
pub async fn bounded<I, T, F, Fut>(limit: usize, items: I, work: F) -> Vec<T>
where
	I: IntoIterator,
	F: FnMut(I::Item) -> Fut,
	Fut: Future<Output = T>,
{
	stream::iter(items)
		.map(work)
		.buffer_unordered(limit.max(1))
		.collect()
		.await
}

#[cfg(test)]
mod tests {
	use std::{
		cell::Cell,
		rc::Rc,
		time::{Duration, Instant},
	};

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
}
