// SPDX-License-Identifier: MPL-2.0

//! [`Job`]: the handle work receives from [`crate::TaskSlot::run`].

use std::{
	any::Any,
	cell::RefCell,
	fmt,
	future::Future,
	pin::Pin,
	rc::Rc,
	task::{Context as TaskContext, Poll},
	time::{Duration, Instant},
};

use futures::{FutureExt as _, future::LocalBoxFuture};
use gpui::{AsyncApp, Context, WeakEntity};
use tokio_util::sync::CancellationToken;

use crate::{
	activity::ActivityLease,
	progress::Progress,
	spawn::{Spawned, Spawner},
};

/// The work was replaced or cancelled, or its view is gone. Return it with `?` to stop
/// quietly; it is not an error to report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stale;

impl fmt::Display for Stale {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		formatter.write_str("the work was cancelled or its view was dropped")
	}
}

impl std::error::Error for Stale {}

/// The result type of work run in a slot.
pub type JobResult<T = ()> = Result<T, Stale>;

type Reaper = Rc<RefCell<Vec<LocalBoxFuture<'static, ()>>>>;
type Flusher<V> = Rc<dyn Fn(&mut V, &mut Context<V>)>;

/// What the runner needs after the work future is gone.
pub(crate) struct Shared<V> {
	/// Waits for children that were dropped or abandoned until they are really gone.
	reaper: Reaper,
	cleanups: RefCell<Vec<LocalBoxFuture<'static, ()>>>,
	leases: RefCell<Vec<Box<dyn Any>>>,
	flushers: RefCell<Vec<Flusher<V>>>,
}

impl<V> Default for Shared<V> {
	fn default() -> Self {
		Self {
			reaper: Reaper::default(),
			cleanups: RefCell::default(),
			leases: RefCell::default(),
			flushers: RefCell::default(),
		}
	}
}

impl<V> Shared<V> {
	pub(crate) fn has_progress(&self) -> bool {
		!self.flushers.borrow().is_empty()
	}

	pub(crate) fn flush(&self, view: &mut V, cx: &mut Context<V>) {
		for flusher in self.flushers.borrow().iter() {
			flusher(view, cx);
		}
	}

	pub(crate) async fn reap(&self) {
		loop {
			let next = self.reaper.borrow_mut().pop();
			match next {
				Some(waiting) => waiting.await,
				None => break,
			}
		}
	}

	/// Runs `on_cancel` futures, newest first. Only a cancelled run needs them.
	pub(crate) async fn clean_up(&self) {
		let cleanups = std::mem::take(&mut *self.cleanups.borrow_mut());
		for cleanup in cleanups.into_iter().rev() {
			cleanup.await;
		}
	}

	pub(crate) fn release_leases(&self) {
		self.leases.borrow_mut().clear();
	}
}

/// Work started on a [`Spawner`] by a [`Job`]. Dropping it cancels the work, and the run waits
/// until the work is gone before it finishes.
struct Child<R: 'static> {
	spawned: Option<Spawned<R>>,
	/// The child's own token, so cancelling one child leaves its siblings running.
	token: CancellationToken,
	reaper: Reaper,
}

impl<R: 'static> Future for Child<R> {
	type Output = JobResult<R>;

	fn poll(mut self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
		let spawned = self.spawned.as_mut().expect("polled after completion");
		let output = Pin::new(spawned).poll(cx);
		if output.is_ready() {
			self.spawned = None;
		}
		output
	}
}

impl<R: 'static> Drop for Child<R> {
	fn drop(&mut self) {
		self.token.cancel();
		if let Some(spawned) = self.spawned.take() {
			self.reaper.borrow_mut().push(spawned.stop().boxed_local());
		}
	}
}

/// The handle work receives in [`crate::TaskSlot::run`].
///
/// Every publish checks that the run is still current. Replaced or cancelled work drops at
/// its next `await`, its children are cancelled and awaited, and then its
/// [`on_cancel`](Self::on_cancel) cleanup runs.
pub struct Job<'a, V> {
	pub(crate) view: WeakEntity<V>,
	pub(crate) cx: &'a mut AsyncApp,
	pub(crate) root: CancellationToken,
	pub(crate) shared: Rc<Shared<V>>,
}

impl<V: 'static> Job<'_, V> {
	/// Whether the run is still current. It touches no entity, so it is cheap in a hot loop.
	pub fn is_current(&self) -> bool {
		!self.root.is_cancelled()
	}

	/// A token for cooperative work. It is cancelled when the run is cancelled, replaced, or ends.
	pub fn token(&self) -> CancellationToken {
		self.root.child_token()
	}

	/// The view, for cleanup futures.
	pub fn view(&self) -> WeakEntity<V> {
		self.view.clone()
	}

	/// The async context, for cleanup futures and APIs this handle does not wrap.
	pub fn app(&self) -> AsyncApp {
		self.cx.clone()
	}

	/// Publishes into the view. Fails with [`Stale`] when the run is no longer current. Pending
	/// [`Progress`] values publish first, so an update always sees the newest progress.
	pub fn update<R>(&mut self, update: impl FnOnce(&mut V, &mut Context<V>) -> R) -> JobResult<R> {
		let root = &self.root;
		let shared = &self.shared;
		self.view
			.update(self.cx, |view, cx| {
				if root.is_cancelled() {
					return Err(Stale);
				}
				shared.flush(view, cx);
				Ok(update(view, cx))
			})
			.unwrap_or(Err(Stale))
	}

	/// Reads from the view without the cost of an update and its effect flush.
	pub fn read<R>(&self, read: impl FnOnce(&V) -> R) -> JobResult<R> {
		if !self.is_current() {
			return Err(Stale);
		}
		self.view
			.read_with(&*self.cx, |view, _| read(view))
			.map_err(|_| Stale)
	}

	/// The executor clock, which tests can advance.
	pub fn now(&self) -> Instant {
		self.cx.background_executor().now()
	}

	/// Waits on the executor clock, without blocking the UI thread.
	pub async fn sleep(&self, duration: Duration) {
		self.cx.background_executor().timer(duration).await;
	}

	/// Releases `lease` when the run ends, after its children and cleanup are gone. A lease
	/// moved into the work future instead releases as soon as that future drops.
	pub fn hold<A: Default + 'static>(&self, lease: ActivityLease<A>) {
		self.shared.leases.borrow_mut().push(Box::new(lease));
	}

	/// Registers `cleanup`, which runs to completion when the run is cancelled or replaced,
	/// after the work and its children are gone. [`crate::Settled`] resolves after it.
	/// A run that finishes normally drops it unpolled. Newest cleanup runs first.
	pub fn on_cancel(&self, cleanup: impl Future<Output = ()> + 'static) {
		self.shared.cleanups.borrow_mut().push(cleanup.boxed_local());
	}

	/// Runs `work` on the gpui background executor. The token is cancelled with the run, so
	/// blocking work can poll it. The run waits for `work` to stop before it finishes.
	pub fn background<R, Fut, W>(&self, work: W) -> impl Future<Output = R> + use<R, Fut, W, V>
	where
		W: FnOnce(CancellationToken) -> Fut + Send + 'static,
		R: Send + 'static,
		Fut: Future<Output = R> + Send + 'static,
	{
		let spawner = Spawner::Background(self.cx.background_executor().clone());
		self.child(&spawner, work)
			.map(|output| output.unwrap_or_else(|Stale| unreachable!("gpui tasks do not fail")))
	}

	/// Runs `work` on a tokio runtime. Cancelling the run aborts the task and the run waits for it.
	/// Dropping the returned future cancels the task. It fails with [`Stale`] when the runtime
	/// shuts down before the work finishes.
	#[cfg(feature = "tokio")]
	pub fn tokio<R, Fut, W>(
		&self,
		handle: &tokio::runtime::Handle,
		work: W,
	) -> impl Future<Output = JobResult<R>> + use<R, Fut, W, V>
	where
		W: FnOnce(CancellationToken) -> Fut + Send + 'static,
		R: Send + 'static,
		Fut: Future<Output = R> + Send + 'static,
	{
		self.child(&Spawner::from(handle), work)
	}

	fn child<R, Fut, W>(&self, spawner: &Spawner, work: W) -> Child<R>
	where
		W: FnOnce(CancellationToken) -> Fut + Send + 'static,
		R: Send + 'static,
		Fut: Future<Output = R> + Send + 'static,
	{
		let token = self.token();
		let child_token = token.clone();
		Child {
			spawned: Some(spawner.spawn(async move { work(child_token).await })),
			token,
			reaper: self.shared.reaper.clone(),
		}
	}

	/// Creates a [`Progress`] cell that any thread can set. `publish` runs on the view at most
	/// once per `every`: the first value at once, later values after the interval, and the
	/// last value before the run ends. A value set after the run ended is dropped.
	pub fn progress<P: Send + 'static>(
		&self,
		every: Duration,
		publish: impl Fn(&mut V, P, &mut Context<V>) + 'static,
	) -> Progress<P> {
		let progress = Progress::<P>::new();
		let publish = Rc::new(publish);
		let flusher: Flusher<V> = {
			let cell = progress.0.clone();
			Rc::new(move |view, cx| {
				if let Some(value) = cell.take() {
					publish(view, value, cx);
				}
			})
		};
		self.shared.flushers.borrow_mut().push(flusher.clone());

		let (view, root, cell) = (self.view.clone(), self.root.clone(), progress.0.clone());
		let (gone_tx, gone) = futures::channel::oneshot::channel::<()>();
		let pump = self.cx.spawn(async move |cx| {
			let _gone = gone_tx;
			loop {
				cell.pending().await;
				let published = view.update(cx, |view, cx| {
					let current = !root.is_cancelled();
					if current {
						flusher(view, cx);
					}
					current
				});
				if !matches!(published, Ok(true)) {
					break;
				}
				// New values wait out the interval; the loop then publishes the newest one.
				cx.background_executor().timer(every).await;
			}
		});
		self.shared.reaper.borrow_mut().push(
			async move {
				drop(pump);
				let _ = gone.await;
			}
			.boxed_local(),
		);
		progress
	}
}
