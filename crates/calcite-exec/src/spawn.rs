// SPDX-License-Identifier: MPL-2.0

//! Where spawned work runs, and how to wait until it is really gone.

use std::{
	future::Future,
	pin::Pin,
	task::{Context, Poll},
};

use futures::channel::oneshot;
use gpui::{BackgroundExecutor, Task};

use crate::job::Stale;

/// The executor that runs spawned work.
#[derive(Clone)]
pub enum Spawner {
	/// The gpui background executor.
	Background(BackgroundExecutor),
	/// A tokio runtime. Needs the `tokio` feature.
	#[cfg(feature = "tokio")]
	Tokio(tokio::runtime::Handle),
}

impl From<&BackgroundExecutor> for Spawner {
	fn from(executor: &BackgroundExecutor) -> Self {
		Self::Background(executor.clone())
	}
}

impl From<BackgroundExecutor> for Spawner {
	fn from(executor: BackgroundExecutor) -> Self {
		Self::Background(executor)
	}
}

#[cfg(feature = "tokio")]
impl From<&tokio::runtime::Handle> for Spawner {
	fn from(handle: &tokio::runtime::Handle) -> Self {
		Self::Tokio(handle.clone())
	}
}

impl Spawner {
	pub(crate) fn spawn<R: Send + 'static>(
		&self,
		work: impl Future<Output = R> + Send + 'static,
	) -> Spawned<R> {
		let (gone_tx, gone) = oneshot::channel();
		// The sender lives in the future, so it drops with the future: after the last poll,
		// including a poll that was already running on another thread when the task was dropped.
		let work = async move {
			let _gone = gone_tx;
			work.await
		};
		let kind = match self {
			Self::Background(executor) => Kind::Task(Box::pin(executor.spawn(work))),
			#[cfg(feature = "tokio")]
			Self::Tokio(handle) => Kind::Tokio(Box::pin(tokio_util::task::AbortOnDropHandle::new(
				handle.spawn(work),
			))),
		};
		Spawned { kind, gone }
	}
}

enum Kind<R> {
	// Boxed so `Spawned` is `Unpin` for any `R`.
	Task(Pin<Box<Task<R>>>),
	#[cfg(feature = "tokio")]
	Tokio(Pin<Box<tokio_util::task::AbortOnDropHandle<R>>>),
}

/// Spawned work. Dropping it cancels the work; [`Spawned::stop`] also waits until it is gone.
pub(crate) struct Spawned<R> {
	kind: Kind<R>,
	gone: oneshot::Receiver<()>,
}

impl<R: 'static> Spawned<R> {
	/// Cancels the work now. The returned future resolves once the work is really gone.
	pub(crate) fn stop(self) -> impl Future<Output = ()> {
		let Self { kind, gone } = self;
		drop(kind);
		async move {
			// Resolves with `Canceled` when the sender drops; both mean the future is gone.
			let _ = gone.await;
		}
	}
}

/// Fails with [`Stale`] when the tokio runtime shut down before the work finished.
impl<R: 'static> Future for Spawned<R> {
	type Output = Result<R, Stale>;

	fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		match &mut self.kind {
			Kind::Task(task) => task.as_mut().poll(cx).map(Ok),
			#[cfg(feature = "tokio")]
			Kind::Tokio(handle) => handle.as_mut().poll(cx).map(|joined| match joined {
				Ok(value) => Ok(value),
				Err(error) => match error.try_into_panic() {
					Ok(payload) => std::panic::resume_unwind(payload),
					Err(_) => Err(Stale),
				},
			}),
		}
	}
}
