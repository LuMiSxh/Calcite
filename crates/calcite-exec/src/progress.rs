// SPDX-License-Identifier: MPL-2.0

//! Values that cross threads without touching the view: byte counters and latest-value progress.

use std::{
	sync::{
		Arc, Mutex,
		atomic::{AtomicU64, Ordering},
	},
	task::Poll,
};

use futures::task::AtomicWaker;

/// A shared total, for example bytes downloaded by N workers.
#[derive(Clone, Debug, Default)]
pub struct Counter(Arc<AtomicU64>);

impl Counter {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn add(&self, amount: u64) -> u64 {
		self.0.fetch_add(amount, Ordering::Relaxed) + amount
	}

	pub fn get(&self) -> u64 {
		self.0.load(Ordering::Relaxed)
	}
}

pub(crate) struct Cell<P> {
	value: Mutex<Option<P>>,
	waker: AtomicWaker,
}

impl<P> Cell<P> {
	pub(crate) fn take(&self) -> Option<P> {
		self.value
			.lock()
			.unwrap_or_else(|poisoned| poisoned.into_inner())
			.take()
	}

	fn is_pending(&self) -> bool {
		self.value
			.lock()
			.unwrap_or_else(|poisoned| poisoned.into_inner())
			.is_some()
	}

	/// Resolves while a value waits to be published. It does not take the value.
	pub(crate) async fn pending(&self) {
		futures::future::poll_fn(|cx| {
			if self.is_pending() {
				return Poll::Ready(());
			}
			self.waker.register(cx.waker());
			// A value set between the check and the registration would otherwise be missed.
			if self.is_pending() {
				Poll::Ready(())
			} else {
				Poll::Pending
			}
		})
		.await
	}
}

/// The latest value of a quantity that changes faster than the view should redraw. Any
/// thread can [`set`](Progress::set) it. The job publishes it at most once per interval,
/// and always publishes the last value before the job ends. See [`crate::Job::progress`].
pub struct Progress<P>(pub(crate) Arc<Cell<P>>);

impl<P> Clone for Progress<P> {
	fn clone(&self) -> Self {
		Self(self.0.clone())
	}
}

impl<P> Progress<P> {
	pub(crate) fn new() -> Self {
		Self(Arc::new(Cell {
			value: Mutex::new(None),
			waker: AtomicWaker::new(),
		}))
	}

	/// Replaces the pending value. Values that nobody published yet are lost on purpose.
	pub fn set(&self, value: P) {
		*self
			.0
			.value
			.lock()
			.unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(value);
		self.0.waker.wake();
	}
}
