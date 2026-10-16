// SPDX-License-Identifier: MPL-2.0

//! One exclusive activity at a time, claimed and released through leases.

use std::{
	cell::{Ref, RefCell},
	fmt,
	rc::Rc,
};

#[derive(Debug, Default)]
struct State<A> {
	activity: A,
	current: Option<u64>,
	issued: u64,
}

/// Another activity holds the lease.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Busy;

impl fmt::Display for Busy {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		formatter.write_str("another activity is running")
	}
}

impl std::error::Error for Busy {}

/// One exclusive activity at a time (install, launch, run, stop...). `A::default()` is idle.
///
/// The store does not redraw anything. A lease that a [`crate::Job`] holds releases during the
/// job's teardown, and the job then notifies its view. After any other release or a
/// [`revoke`](Self::revoke), call `cx.notify()` yourself.
#[derive(Debug, Default)]
pub struct ActivityStore<A> {
	state: Rc<RefCell<State<A>>>,
}

impl<A: Default> ActivityStore<A> {
	pub fn new() -> Self {
		Self::default()
	}

	/// The current activity. Do not hold the borrow across `begin`, `revoke`, or a lease call.
	pub fn activity(&self) -> Ref<'_, A> {
		Ref::map(self.state.borrow(), |state| &state.activity)
	}

	pub fn is_idle(&self) -> bool {
		self.state.borrow().current.is_none()
	}

	/// Claims the exclusive activity.
	pub fn begin(&mut self, activity: A) -> Result<ActivityLease<A>, Busy> {
		let mut state = self.state.borrow_mut();
		if state.current.is_some() {
			return Err(Busy);
		}
		state.issued += 1;
		let id = state.issued;
		state.current = Some(id);
		state.activity = activity;
		Ok(ActivityLease {
			id,
			state: self.state.clone(),
		})
	}

	/// Makes every outstanding lease stale and sets the store idle at once. Later calls on a
	/// stale lease change nothing, so the old holder can never set idle over a newer activity.
	pub fn revoke(&mut self) {
		let mut state = self.state.borrow_mut();
		state.current = None;
		state.activity = A::default();
	}
}

/// Proof that one operation owns the exclusive activity. Dropping the lease releases the
/// activity, so work that is cancelled or dropped can never leave the store busy.
#[must_use = "dropping a lease releases the activity at once"]
#[derive(Debug)]
pub struct ActivityLease<A: Default> {
	id: u64,
	state: Rc<RefCell<State<A>>>,
}

impl<A: Default> ActivityLease<A> {
	/// Whether this lease still owns the activity.
	pub fn is_current(&self) -> bool {
		self.state.borrow().current == Some(self.id)
	}

	/// Changes the activity for the owner. A stale lease changes nothing and returns `false`.
	pub fn set(&self, activity: A) -> bool {
		let mut state = self.state.borrow_mut();
		if state.current != Some(self.id) {
			return false;
		}
		state.activity = activity;
		true
	}

	/// Releases the activity now. Returns `false` for a stale lease, which changes nothing.
	pub fn end(self) -> bool {
		self.release()
	}

	fn release(&self) -> bool {
		let mut state = self.state.borrow_mut();
		if state.current != Some(self.id) {
			return false;
		}
		state.current = None;
		state.activity = A::default();
		true
	}
}

impl<A: Default> Drop for ActivityLease<A> {
	fn drop(&mut self) {
		self.release();
	}
}
