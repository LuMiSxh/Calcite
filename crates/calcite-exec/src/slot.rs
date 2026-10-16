// SPDX-License-Identifier: MPL-2.0

//! [`TaskSlot`]: one in-flight run per slot, with cancellation you can wait for.

use std::{
	cell::Cell,
	future::Future,
	pin::Pin,
	rc::Rc,
	task::{Context as TaskContext, Poll},
};

use futures::{FutureExt as _, channel::oneshot, future::Shared};
use gpui::Context;
use tokio_util::sync::CancellationToken;

use crate::job::{Job, JobResult, Shared as JobShared};

/// Resolves when a cancelled run is completely gone: its work future, its children, and its
/// [`Job::on_cancel`] cleanup. Cloning is cheap. Dropping it does not stop the cleanup.
#[derive(Clone)]
pub struct Settled {
	done: Rc<Cell<bool>>,
	signal: Shared<oneshot::Receiver<()>>,
}

impl Settled {
	fn ready() -> Self {
		let (tx, rx) = oneshot::channel();
		tx.send(()).ok();
		Self {
			done: Rc::new(Cell::new(true)),
			signal: rx.shared(),
		}
	}

	/// Whether the run is gone already.
	pub fn is_settled(&self) -> bool {
		self.done.get()
	}
}

impl Future for Settled {
	type Output = ();

	fn poll(mut self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<()> {
		Pin::new(&mut self.signal).poll(cx).map(|_| ())
	}
}

struct Run {
	root: CancellationToken,
	settled: Settled,
}

/// Owns at most one in-flight run. A new [`run`](Self::run) cancels the old one, and the new
/// work starts only after the old run settled, so two runs never overlap. Keep one slot per
/// key (for example in a map) when several runs may proceed side by side.
#[derive(Default)]
pub struct TaskSlot {
	run: Option<Run>,
}

impl TaskSlot {
	pub fn new() -> Self {
		Self::default()
	}

	/// Whether a run is in flight and not cancelled.
	pub fn is_running(&self) -> bool {
		self.run
			.as_ref()
			.is_some_and(|run| !run.root.is_cancelled() && !run.settled.is_settled())
	}

	/// Cancels the run. Its work future drops at the next `await`, its children are cancelled
	/// and awaited, and then its [`Job::on_cancel`] cleanup runs to completion. Await the
	/// result to wait for all of that. Dropping it is safe: the cleanup still runs.
	#[must_use = "await it when later work must not overlap the cancelled run"]
	pub fn cancel(&mut self) -> Settled {
		match &self.run {
			Some(run) => {
				run.root.cancel();
				run.settled.clone()
			}
			None => Settled::ready(),
		}
	}

	/// Cancels the current run and starts `work` after that run settled. Returns the
	/// [`Settled`] of the replaced run. The view redraws when the run ends.
	///
	/// ```ignore
	/// self.download.run(cx, async move |job| {
	///     job.sleep(Duration::from_millis(100)).await;
	///     job.update(|view, cx| { view.progress = 0.5; cx.notify(); })
	/// });
	/// ```
	pub fn run<V, F>(&mut self, cx: &mut Context<V>, work: F) -> Settled
	where
		V: 'static,
		F: AsyncFnOnce(&mut Job<'_, V>) -> JobResult + 'static,
	{
		let replaced = self.cancel();
		let root = CancellationToken::new();
		let (finished, signal) = oneshot::channel();
		let settled = Settled {
			done: Rc::new(Cell::new(false)),
			signal: signal.shared(),
		};
		self.run = Some(Run {
			root: root.clone(),
			settled: settled.clone(),
		});

		let waiting_for = replaced.clone();
		cx.spawn(async move |view, cx| {
			waiting_for.await;
			let shared = Rc::new(JobShared::default());
			let result = if root.is_cancelled() {
				None
			} else {
				let mut job = Job {
					view: view.clone(),
					cx: &mut *cx,
					root: root.clone(),
					shared: shared.clone(),
				};
				root.run_until_cancelled(work(&mut job)).await
			};
			let completed = result.is_some() && !root.is_cancelled();

			if completed && shared.has_progress() {
				view.update(cx, |view, cx| shared.flush(view, cx)).ok();
			}
			root.cancel();
			shared.reap().await;
			if !completed {
				shared.clean_up().await;
			}
			shared.release_leases();
			settled.done.set(true);
			view.update(cx, |_, cx| cx.notify()).ok();
			let _ = finished.send(());
		})
		.detach();
		replaced
	}
}

impl Drop for TaskSlot {
	fn drop(&mut self) {
		if let Some(run) = &self.run {
			run.root.cancel();
		}
	}
}
