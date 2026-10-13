// SPDX-License-Identifier: MPL-2.0

//! [`TaskSlot`] and [`Job`]: one in-flight task per slot, and a handle that publishes into
//! the view only while the work still owns its slot.

use std::{future::Future, time::Duration};

use gpui::{AppContext as _, AsyncApp, Context, Task, WeakEntity};

use crate::Throttle;

/// Identity of one run of a [`TaskSlot`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlotToken(u64);

/// The work no longer owns its slot, a guard failed, or the view is gone. Return it with
/// `?` to stop quietly; it is not an error to report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stale;

impl std::fmt::Display for Stale {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter.write_str("the work was replaced, invalidated, or its view was dropped")
	}
}

impl std::error::Error for Stale {}

/// The result type of work run in a slot.
pub type JobResult<T = ()> = Result<T, Stale>;

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

	/// Cancels the running task and runs `work` under a fresh token. `slot` selects this slot
	/// from the view, for example `|view| &mut view.download`. The slot turns idle when the
	/// work returns, unless newer work replaced it.
	///
	/// ```ignore
	/// self.download.run(cx, |view| &mut view.download, async move |job| {
	///     job.sleep(Duration::from_millis(100)).await;
	///     job.update(|view, cx| { view.progress = 0.5; cx.notify(); })?;
	///     Ok(())
	/// });
	/// ```
	pub fn run<V, F>(&mut self, cx: &mut Context<V>, slot: SlotSelector<V>, work: F) -> SlotToken
	where
		V: 'static,
		F: AsyncFnOnce(&mut Job<'_, V>) -> JobResult + 'static,
	{
		let token = self.begin();
		self.task = Some(cx.spawn(async move |view, cx| {
			let mut job = Job {
				token,
				view,
				cx,
				slot,
				guards: Vec::new(),
			};
			// Stale work ends quietly; current work releases its slot either way.
			let _ = work(&mut job).await;
			job.view
				.update(job.cx, |view, cx| {
					if slot(view).finish(token) {
						cx.notify();
					}
				})
				.ok();
		}));
		token
	}

	/// Cancels the running task and starts `operation` under a fresh token. Prefer
	/// [`TaskSlot::run`]; this is the lower-level form with a plain token.
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

	/// Whether work started from this slot is still running.
	pub fn is_running(&self) -> bool {
		self.active
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

type Guard<V> = Box<dyn Fn(&V) -> bool>;

/// Selects a slot inside its view, for example `|view| &mut view.download`.
pub type SlotSelector<V> = fn(&mut V) -> &mut TaskSlot;

/// The handle work receives in [`TaskSlot::run`]. Every read and write rechecks that the
/// work still owns its slot and that every guard still holds.
pub struct Job<'a, V> {
	token: SlotToken,
	view: WeakEntity<V>,
	cx: &'a mut AsyncApp,
	slot: SlotSelector<V>,
	guards: Vec<Guard<V>>,
}

impl<V: 'static> Job<'_, V> {
	pub fn token(&self) -> SlotToken {
		self.token
	}

	/// Adds an identity check, for example that an activity lease is still current. Like the
	/// slot token, it is rechecked before every publish.
	pub fn guard(&mut self, guard: impl Fn(&V) -> bool + 'static) -> &mut Self {
		self.guards.push(Box::new(guard));
		self
	}

	/// Publishes into the view if the work is still current.
	pub fn update<R>(&mut self, update: impl FnOnce(&mut V, &mut Context<V>) -> R) -> JobResult<R> {
		let slot = self.slot;
		let token = self.token;
		let guards = &self.guards;
		self.view
			.update(self.cx, |view, cx| {
				if !slot(view).owns(token) || !guards.iter().all(|guard| guard(view)) {
					return Err(Stale);
				}
				Ok(update(view, cx))
			})
			.unwrap_or(Err(Stale))
	}

	/// Reads from the view if the work is still current.
	pub fn read<R>(&mut self, read: impl FnOnce(&V) -> R) -> JobResult<R> {
		self.update(|view, _| read(view))
	}

	/// Publishes only when `throttle` admits a value now; returns whether it published.
	pub fn update_throttled(
		&mut self,
		throttle: &mut Throttle,
		update: impl FnOnce(&mut V, &mut Context<V>),
	) -> JobResult<bool> {
		if !throttle.admit(std::time::Instant::now()) {
			// Still fail fast when the work was replaced while it skipped frames.
			self.check()?;
			return Ok(false);
		}
		self.update(update).map(|()| true)
	}

	/// Fails with [`Stale`] when the work is no longer current.
	pub fn check(&mut self) -> JobResult {
		self.read(|_| ())
	}

	/// Waits without blocking the UI thread.
	pub async fn sleep(&mut self, duration: Duration) {
		self.cx.background_executor().timer(duration).await;
	}

	/// Runs CPU or blocking work off the UI thread and returns its result.
	pub async fn background<R: Send + 'static>(
		&mut self,
		work: impl Future<Output = R> + Send + 'static,
	) -> R {
		self.cx.background_spawn(work).await
	}

	/// The underlying async context, for APIs this handle does not wrap.
	pub fn async_cx(&mut self) -> &mut AsyncApp {
		self.cx
	}
}
