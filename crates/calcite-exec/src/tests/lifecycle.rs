// SPDX-License-Identifier: MPL-2.0

use std::sync::mpsc;

use super::*;

#[gpui::test]
async fn cleanups_run_newest_first(cx: &mut TestAppContext) {
	let view = new_view(cx);
	let order = Rc::new(RefCell::new(Vec::new()));
	view.update(cx, |view, cx| {
		let order = order.clone();
		view.slot.run(cx, async move |job| {
			for name in ["first", "second", "third"] {
				let order = order.clone();
				job.on_cancel(async move { order.borrow_mut().push(name) });
			}
			job.sleep(Duration::from_secs(3600)).await;
			Ok(())
		});
	});
	cx.run_until_parked();
	let settled = view.update(cx, |view, _| view.slot.cancel());
	cx.run_until_parked();
	assert!(settled.is_settled());
	assert_eq!(*order.borrow(), ["third", "second", "first"]);
}

#[gpui::test]
async fn cleanup_still_runs_after_the_slot_and_the_settled_handle_dropped(cx: &mut TestAppContext) {
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

	let observer = view.update(cx, |view, _| {
		let settled = view.slot.cancel();
		let observer = settled.clone();
		drop(settled);
		drop(std::mem::take(&mut view.slot));
		observer
	});
	cx.run_until_parked();
	assert!(!observer.is_settled());

	release.send(()).unwrap();
	cx.run_until_parked();
	assert!(cleaned.get());
	assert!(observer.is_settled());
}

#[gpui::test]
async fn progress_set_before_an_update_is_published_before_it(cx: &mut TestAppContext) {
	let view = new_view(cx);
	view.update(cx, |view, cx| {
		view.slot.run(cx, async move |job| {
			let progress = job.progress(Duration::from_secs(3600), |view: &mut View, value, _| {
				view.published.push(value);
			});
			progress.set(1);
			job.update(|view, _| view.published.push(100))?;
			progress.set(2);
			job.update(|view, _| view.published.push(200))
		});
	});
	cx.run_until_parked();
	view.read_with(cx, |view, _| assert_eq!(view.published, [1, 100, 2, 200]));
}

#[gpui::test]
async fn dropping_a_child_future_cancels_its_task_and_spares_its_siblings(cx: &mut TestAppContext) {
	let view = new_view(cx);
	let (child_token, child_cancelled) = mpsc::channel::<CancellationToken>();
	let (sibling_token, sibling) = mpsc::channel::<CancellationToken>();
	view.update(cx, |view, cx| {
		view.slot.run(cx, async move |job| {
			let doomed = job.background(move |token| async move {
				child_token.send(token).unwrap();
				futures::future::pending::<()>().await;
			});
			let _sibling = job.background(move |token| async move {
				sibling_token.send(token).unwrap();
				futures::future::pending::<()>().await;
			});
			job.sleep(Duration::from_millis(10)).await;
			drop(doomed);
			job.sleep(Duration::from_secs(3600)).await;
			Ok(())
		});
	});
	cx.run_until_parked();
	let (doomed, sibling) = (child_cancelled.try_recv().unwrap(), sibling.try_recv().unwrap());
	assert!(!doomed.is_cancelled());
	cx.executor().advance_clock(Duration::from_millis(10));
	cx.run_until_parked();
	assert!(
		doomed.is_cancelled(),
		"the dropped child was cancelled while the run goes on"
	);
	assert!(!sibling.is_cancelled());
}
