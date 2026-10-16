// SPDX-License-Identifier: MPL-2.0

//! Windowed parallelism: never an unbounded fan-out.

use std::{
	future::Future,
	num::NonZeroUsize,
	pin::Pin,
	task::{Context, Poll},
};

use futures::{Stream, StreamExt as _, future::join_all, stream::FuturesUnordered};
use tokio_util::sync::CancellationToken;

use crate::{
	job::Stale,
	spawn::{Spawned, Spawner},
};

type Next<T, E> = Box<dyn FnMut(&Spawner, &CancellationToken) -> Option<Spawned<Result<T, E>>>>;

/// Runs `work` for every item on `spawner`, with at most `limit` items in flight: it seeds
/// `limit` and starts one more per completion. Results arrive in completion order.
///
/// Each item gets a child token of the stream. Dropping the stream cancels every task that
/// still runs and starts no more. [`TryBounded::fail_fast`] does the same after the first error.
pub fn try_bounded<I, T, E, F, Fut>(
	spawner: impl Into<Spawner>,
	limit: NonZeroUsize,
	items: I,
	mut work: F,
) -> TryBounded<T, E>
where
	I: IntoIterator + 'static,
	F: FnMut(I::Item, CancellationToken) -> Fut + 'static,
	Fut: Future<Output = Result<T, E>> + Send + 'static,
	T: Send + 'static,
	E: Send + 'static,
{
	let mut items = items.into_iter();
	TryBounded {
		spawner: spawner.into(),
		limit: limit.get(),
		token: CancellationToken::new(),
		next: Some(Box::new(move |spawner, token| {
			let item = items.next()?;
			Some(spawner.spawn(work(item, token.child_token())))
		})),
		running: FuturesUnordered::new(),
		fail_fast: false,
	}
}

/// The stream [`try_bounded`] returns.
#[must_use = "a stream does nothing unless polled"]
pub struct TryBounded<T: 'static, E: 'static> {
	spawner: Spawner,
	limit: usize,
	token: CancellationToken,
	/// `None` once the items ran out or the stream stopped.
	next: Option<Next<T, E>>,
	running: FuturesUnordered<Spawned<Result<T, E>>>,
	fail_fast: bool,
}

impl<T: 'static, E: 'static> TryBounded<T, E> {
	/// Ends the stream after the first error: the error is the last item, the running tasks
	/// are cancelled, and the remaining items never start.
	pub fn fail_fast(mut self) -> Self {
		self.fail_fast = true;
		self
	}

	/// Cancels every running task and waits until each one is gone.
	pub async fn cancel(mut self) {
		self.stop();
		join_all(std::mem::take(&mut self.running).into_iter().map(Spawned::stop)).await;
	}

	fn stop(&mut self) {
		self.token.cancel();
		self.next = None;
	}

	fn fill(&mut self) {
		while self.running.len() < self.limit {
			let Some(next) = self.next.as_mut() else { break };
			match next(&self.spawner, &self.token) {
				Some(spawned) => self.running.push(spawned),
				None => self.next = None,
			}
		}
	}
}

impl<T: 'static, E: 'static> Stream for TryBounded<T, E> {
	type Item = Result<T, E>;

	fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
		let this = &mut *self;
		loop {
			this.fill();
			match this.running.poll_next_unpin(cx) {
				Poll::Ready(Some(Ok(result))) => {
					if this.fail_fast && result.is_err() {
						this.stop();
						this.running = FuturesUnordered::new();
					} else {
						this.fill();
					}
					return Poll::Ready(Some(result));
				}
				// The tokio runtime shut down: no item can finish, so start no more.
				Poll::Ready(Some(Err(Stale))) => this.stop(),
				Poll::Ready(None) => return Poll::Ready(None),
				Poll::Pending => return Poll::Pending,
			}
		}
	}
}

impl<T: 'static, E: 'static> Drop for TryBounded<T, E> {
	fn drop(&mut self) {
		self.token.cancel();
	}
}
