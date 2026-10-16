// SPDX-License-Identifier: MPL-2.0

use std::{
	cell::{Cell, RefCell},
	num::NonZeroUsize,
	rc::Rc,
	sync::{
		Arc,
		atomic::{AtomicBool, AtomicUsize, Ordering},
	},
	time::Duration,
};

use futures::{FutureExt as _, StreamExt as _, channel::oneshot};
use gpui::{AppContext as _, TestAppContext};

use crate::prelude::*;

#[derive(Default, Debug)]
enum Work {
	#[default]
	Idle,
	Busy,
}

#[derive(Default)]
struct View {
	slot: TaskSlot,
	activity: ActivityStore<Work>,
	value: u32,
	published: Vec<u32>,
}

fn new_view(cx: &mut TestAppContext) -> gpui::Entity<View> {
	cx.new(|_| View::default())
}

/// Sets its flag when dropped, so a test can see that a future was destroyed.
struct OnDrop(Arc<AtomicBool>);

impl Drop for OnDrop {
	fn drop(&mut self) {
		self.0.store(true, Ordering::SeqCst);
	}
}

#[gpui::test]
async fn a_lease_in_cancelled_work_is_released(cx: &mut TestAppContext) {
	let view = new_view(cx);
	view.update(cx, |view, cx| {
		let lease = view.activity.begin(Work::Busy).unwrap();
		view.slot.run(cx, async move |job| {
			job.hold(lease);
			job.sleep(Duration::from_secs(3600)).await;
			Ok(())
		});
	});
	cx.run_until_parked();
	view.read_with(cx, |view, _| assert!(!view.activity.is_idle()));

	let settled = view.update(cx, |view, _| view.slot.cancel());
	cx.run_until_parked();
	assert!(settled.is_settled());
	view.read_with(cx, |view, _| {
		assert!(view.activity.is_idle());
		assert!(!view.slot.is_running());
	});
}

#[test]
fn a_stale_lease_never_sets_idle() {
	let mut store = ActivityStore::<Work>::new();
	let old = store.begin(Work::Busy).unwrap();
	assert!(store.begin(Work::Busy).is_err());
	store.revoke();
	assert!(store.is_idle());
	let new = store.begin(Work::Busy).unwrap();
	assert!(!old.is_current());
	assert!(!old.set(Work::Idle));
	assert!(!old.end());
	assert!(new.is_current());
	assert!(!store.is_idle());
}

#[gpui::test]
async fn settled_resolves_only_after_the_cleanup_completed(cx: &mut TestAppContext) {
	let view = new_view(cx);
	let (release, gate) = oneshot::channel::<()>();
	let cleaned = Rc::new(Cell::new(false));
	view.update(cx, |view, cx| {
		let cleaned = cleaned.clone();
		view.slot.run(cx, async move |job| {
			job.on_cancel(async move {
				let _ = gate.await;
				cleaned.set(true);
			});
			job.sleep(Duration::from_secs(3600)).await;
			Ok(())
		});
	});
	cx.run_until_parked();

	let settled = view.update(cx, |view, _| view.slot.cancel());
	cx.run_until_parked();
	assert!(!settled.is_settled(), "the cleanup still waits for its gate");

	release.send(()).unwrap();
	cx.run_until_parked();
	assert!(settled.is_settled());
	assert!(cleaned.get());
}

#[gpui::test]
async fn replaced_work_cannot_publish_and_the_new_run_waits_for_the_old_one(cx: &mut TestAppContext) {
	let view = new_view(cx);
	let (release, gate) = oneshot::channel::<()>();
	let order = Rc::new(RefCell::new(Vec::new()));
	view.update(cx, |view, cx| {
		let order = order.clone();
		view.slot.run(cx, async move |job| {
			job.on_cancel(async move {
				let _ = gate.await;
				order.borrow_mut().push("cleanup");
			});
			job.sleep(Duration::from_millis(10)).await;
			job.update(|view, _| view.value = 1)
		});
	});
	cx.run_until_parked();
	view.update(cx, |view, cx| {
		let order = order.clone();
		view.slot.run(cx, async move |job| {
			order.borrow_mut().push("second");
			job.update(|view, _| view.value = 2)
		});
	});
	cx.executor().advance_clock(Duration::from_millis(50));
	cx.run_until_parked();
	assert!(order.borrow().is_empty(), "the second run waits for the cleanup");

	release.send(()).unwrap();
	cx.run_until_parked();
	assert_eq!(*order.borrow(), ["cleanup", "second"]);
	view.read_with(cx, |view, _| {
		assert_eq!(view.value, 2);
		assert!(!view.slot.is_running());
	});
}

#[gpui::test]
async fn progress_is_throttled_on_the_executor_clock_and_ends_with_the_final_value(cx: &mut TestAppContext) {
	let view = new_view(cx);
	view.update(cx, |view, cx| {
		view.slot.run(cx, async move |job| {
			let progress = job.progress(Duration::from_millis(100), |view: &mut View, value, cx| {
				view.published.push(value);
				cx.notify();
			});
			for value in 1..=5 {
				if value > 1 {
					job.sleep(Duration::from_millis(30)).await;
				}
				progress.set(value);
			}
			Ok(())
		});
	});
	cx.run_until_parked();
	for _ in 0..12 {
		cx.executor().advance_clock(Duration::from_millis(10));
		cx.run_until_parked();
	}
	// The first value at once, the newest value when the interval ended, and the final value.
	view.read_with(cx, |view, _| assert_eq!(view.published, [1, 4, 5]));
}

/// Suspends once and wakes itself, so other tasks in the window get polled.
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

fn window(limit: usize) -> NonZeroUsize {
	NonZeroUsize::new(limit).unwrap()
}

#[gpui::test]
async fn try_bounded_never_exceeds_its_window(cx: &mut TestAppContext) {
	let running = Arc::new(AtomicUsize::new(0));
	let peak = Arc::new(AtomicUsize::new(0));
	let work = {
		let (running, peak) = (running.clone(), peak.clone());
		move |item: u32, _token: CancellationToken| {
			let (running, peak) = (running.clone(), peak.clone());
			async move {
				peak.fetch_max(running.fetch_add(1, Ordering::SeqCst) + 1, Ordering::SeqCst);
				for _ in 0..3 {
					yield_now().await;
				}
				running.fetch_sub(1, Ordering::SeqCst);
				Ok::<_, ()>(item)
			}
		}
	};
	let mut results: Vec<_> = try_bounded(cx.executor(), window(3), 0..12, work)
		.map(Result::unwrap)
		.collect()
		.await;
	results.sort();
	assert_eq!(results, (0..12).collect::<Vec<_>>());
	assert_eq!(peak.load(Ordering::SeqCst), 3);
}

#[gpui::test]
async fn try_bounded_cancels_the_rest_on_error_and_on_drop(cx: &mut TestAppContext) {
	let started = Arc::new(AtomicUsize::new(0));
	let dropped = Arc::new(AtomicBool::new(false));
	let work = {
		let (started, dropped) = (started.clone(), dropped.clone());
		move |item: u32, _token: CancellationToken| {
			let (started, dropped) = (started.clone(), dropped.clone());
			async move {
				started.fetch_add(1, Ordering::SeqCst);
				if item == 1 {
					return Err("failed");
				}
				let _guard = OnDrop(dropped);
				futures::future::pending::<()>().await;
				Ok(item)
			}
		}
	};
	let mut stream = try_bounded(cx.executor(), window(2), 0..10, work.clone()).fail_fast();
	assert_eq!(stream.next().await, Some(Err("failed")));
	assert_eq!(stream.next().await, None);
	cx.run_until_parked();
	assert_eq!(
		started.load(Ordering::SeqCst),
		2,
		"the remaining items never started"
	);
	assert!(dropped.load(Ordering::SeqCst), "the running task was cancelled");

	dropped.store(false, Ordering::SeqCst);
	let mut stream = try_bounded(cx.executor(), window(1), [0u32, 5, 6], work);
	assert!(stream.next().now_or_never().is_none());
	cx.run_until_parked();
	drop(stream);
	cx.run_until_parked();
	assert!(
		dropped.load(Ordering::SeqCst),
		"dropping the stream cancelled the task"
	);
}

#[cfg(feature = "tokio")]
#[gpui::test]
async fn a_replaced_run_aborts_its_tokio_task(cx: &mut TestAppContext) {
	cx.executor().allow_parking();
	let runtime = tokio::runtime::Builder::new_multi_thread()
		.worker_threads(1)
		.build()
		.unwrap();
	let handle = runtime.handle().clone();
	let aborted = Arc::new(AtomicBool::new(false));
	let view = new_view(cx);
	view.update(cx, |view, cx| {
		let aborted = aborted.clone();
		view.slot.run(cx, async move |job| {
			// The guard is captured by the closure, so it drops with the future even if the task never started.
			let guard = OnDrop(aborted);
			job.tokio(&handle, move |_token| async move {
				let _guard = guard;
				futures::future::pending::<()>().await;
			})
			.await;
			Ok(())
		});
	});
	cx.run_until_parked();
	view.update(cx, |view, _| view.slot.cancel()).await;
	assert!(aborted.load(Ordering::SeqCst));
}
