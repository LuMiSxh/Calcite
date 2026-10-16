// SPDX-License-Identifier: MPL-2.0

//! Execution layer that keeps asynchronous work honest in a gpui-ce view.
//!
//! Work runs as ordinary gpui tasks owned by a view, and results reach state only through
//! `Entity::update`. Each type maps to a Swift counterpart in Arknights Client:
//!
//! | Rust                                   | Swift                                        |
//! | -------------------------------------- | -------------------------------------------- |
//! | [`TaskSlot`], [`Job`], [`Settled`]     | `TaskSlot` (`replace`, `owns`, `invalidate`) |
//! | [`ActivityStore`], [`ActivityLease`]   | `LauncherLifecycleStore`, `ActivityLease`    |
//! | [`Progress`], [`Counter`]              | progress throttling to about 10 Hz           |
//! | [`try_bounded`]                        | windowed task group with an `AppConstants` limit |
//!
//! Rules:
//!
//! - Cancellation is real. [`TaskSlot::cancel`] and a replacing [`TaskSlot::run`] drop the work
//!   at its next `await`, cancel its children (background tasks, tokio tasks), and run its
//!   [`Job::on_cancel`] cleanup to completion. Await the returned [`Settled`] to know it is
//!   all gone. A new run never starts before the run it replaced settled.
//! - A lease releases when it drops, so cancelled work cannot leave the store busy. A stale
//!   lease never sets idle.
//! - [`Job::update`] fails with [`Stale`] for replaced work. Use `?` to end it quietly.
//!
//! The default build has no async runtime. The `tokio` feature adds [`Job::tokio`] and
//! [`Spawner::Tokio`].
//!
//! ```ignore
//! use calcite_exec::prelude::*;
//!
//! fn start(&mut self, cx: &mut Context<Self>) {
//!     let Ok(lease) = self.activity.begin(Activity::Downloading) else { return };
//!     self.download.run(cx, async move |job| {
//!         let progress = job.progress(Duration::from_millis(100), |view: &mut Self, value, cx| {
//!             view.progress = value;
//!             cx.notify();
//!         });
//!         for step in 1..=100 {
//!             job.sleep(Duration::from_millis(20)).await;
//!             progress.set(step as f32 / 100.);
//!         }
//!         job.update(|view, cx| {
//!             lease.end();
//!             cx.notify();
//!         })
//!     });
//! }
//! ```

mod activity;
mod bounded;
mod job;
mod progress;
mod slot;
mod spawn;

pub use activity::{ActivityLease, ActivityStore, Busy};
pub use bounded::{TryBounded, try_bounded};
pub use job::{Job, JobResult, Stale};
pub use progress::{Counter, Progress};
pub use slot::{Settled, TaskSlot};
pub use spawn::Spawner;
pub use tokio_util::sync::CancellationToken;

/// Everything a view needs to run work: `use calcite_exec::prelude::*;`.
pub mod prelude {
	pub use crate::{
		ActivityLease, ActivityStore, Busy, CancellationToken, Counter, Job, JobResult, Progress, Settled,
		Spawner, Stale, TaskSlot, TryBounded, try_bounded,
	};
}

#[cfg(test)]
mod tests;
