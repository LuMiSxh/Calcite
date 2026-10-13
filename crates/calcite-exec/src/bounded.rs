// SPDX-License-Identifier: MPL-2.0

//! Windowed parallelism: never an unbounded fan-out.

use std::future::Future;

use futures::{StreamExt, stream};

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
