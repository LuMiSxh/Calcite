// SPDX-License-Identifier: MPL-2.0

use std::sync::Barrier;

use super::*;

fn runtime() -> tokio::runtime::Runtime {
	tokio::runtime::Builder::new_multi_thread()
		.worker_threads(4)
		.build()
		.unwrap()
}

#[gpui::test]
async fn try_bounded_on_tokio_runs_a_full_window_in_parallel_and_never_more(cx: &mut TestAppContext) {
	cx.executor().allow_parking();
	let runtime = runtime();
	let running = Arc::new(AtomicUsize::new(0));
	let peak = Arc::new(AtomicUsize::new(0));
	// Every task in a window waits for the others, so the window must really run in parallel.
	let barrier = Arc::new(Barrier::new(3));
	let work = {
		let (running, peak, barrier) = (running.clone(), peak.clone(), barrier.clone());
		move |item: u32, _token: CancellationToken| {
			let (running, peak, barrier) = (running.clone(), peak.clone(), barrier.clone());
			async move {
				peak.fetch_max(running.fetch_add(1, Ordering::SeqCst) + 1, Ordering::SeqCst);
				barrier.wait();
				running.fetch_sub(1, Ordering::SeqCst);
				Ok::<_, ()>(item)
			}
		}
	};
	let mut results: Vec<_> = try_bounded(runtime.handle(), window(3), 0..9, work)
		.map(Result::unwrap)
		.collect()
		.await;
	results.sort();
	assert_eq!(results, (0..9).collect::<Vec<_>>());
	assert_eq!(peak.load(Ordering::SeqCst), 3);
}

#[gpui::test]
async fn dropping_a_tokio_try_bounded_stream_aborts_the_in_flight_tasks(cx: &mut TestAppContext) {
	cx.executor().allow_parking();
	let runtime = runtime();
	let dropped = Arc::new(AtomicUsize::new(0));
	let (token_tx, tokens) = std::sync::mpsc::channel::<CancellationToken>();
	let work = {
		let dropped = dropped.clone();
		move |_item: u32, token: CancellationToken| {
			let guard = DropCount(dropped.clone());
			let token_tx = token_tx.clone();
			async move {
				let _guard = guard;
				token_tx.send(token).unwrap();
				futures::future::pending::<Result<(), ()>>().await
			}
		}
	};
	let mut stream = try_bounded(runtime.handle(), window(2), 0..6, work);
	assert!(stream.next().now_or_never().is_none());
	let tokens: Vec<_> = (0..2).map(|_| tokens.recv().unwrap()).collect();
	drop(stream);

	assert!(tokens.iter().all(CancellationToken::is_cancelled));
	let deadline = std::time::Instant::now() + Duration::from_secs(10);
	while dropped.load(Ordering::SeqCst) < 2 {
		assert!(std::time::Instant::now() < deadline, "the tasks were not aborted");
		std::thread::yield_now();
	}
	assert_eq!(dropped.load(Ordering::SeqCst), 2, "the rest never started");
}

#[gpui::test]
async fn settled_waits_for_a_child_that_is_still_running_on_another_thread(cx: &mut TestAppContext) {
	cx.executor().allow_parking();
	let runtime = runtime();
	let handle = runtime.handle().clone();
	let view = new_view(cx);
	let (started_tx, started) = std::sync::mpsc::channel::<()>();
	let (release, gate) = std::sync::mpsc::channel::<()>();
	let (finished_tx, finished) = std::sync::mpsc::channel::<()>();
	view.update(cx, |view, cx| {
		view.slot.run(cx, async move |job| {
			// The child blocks its thread inside one poll, so aborting its task cannot stop it.
			job.tokio(&handle, move |_token| async move {
				started_tx.send(()).unwrap();
				gate.recv().unwrap();
				finished_tx.send(()).unwrap();
			})
			.await?;
			Ok(())
		});
	});
	while started.try_recv().is_err() {
		cx.run_until_parked();
		std::thread::yield_now();
	}

	let settled = view.update(cx, |view, _| view.slot.cancel());
	cx.run_until_parked();
	assert!(!settled.is_settled(), "the child still runs");
	assert!(finished.try_recv().is_err());

	release.send(()).unwrap();
	settled.await;
	assert!(finished.try_recv().is_ok());
}

struct DropCount(Arc<AtomicUsize>);

impl Drop for DropCount {
	fn drop(&mut self) {
		self.0.fetch_add(1, Ordering::SeqCst);
	}
}
