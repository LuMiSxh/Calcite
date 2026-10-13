// SPDX-License-Identifier: MPL-2.0

//! One exclusive activity at a time, claimed and released through leases.

/// Proof that one operation owns the exclusive activity. The store rejects any change that
/// carries a lease it did not issue last.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ActivityLease(u64);

/// One exclusive activity at a time (install, launch, run, stop...). `A::default()` is idle.
#[derive(Debug, Default)]
pub struct ActivityStore<A> {
	activity: A,
	lease: Option<ActivityLease>,
	issued: u64,
}

impl<A: Clone + Default + PartialEq> ActivityStore<A> {
	pub fn new() -> Self {
		Self {
			activity: A::default(),
			lease: None,
			issued: 0,
		}
	}

	pub fn activity(&self) -> &A {
		&self.activity
	}

	pub fn is_idle(&self) -> bool {
		self.lease.is_none()
	}

	/// Whether `lease` is the current lease. Use it as a [`crate::Job::guard`].
	pub fn holds(&self, lease: ActivityLease) -> bool {
		self.lease == Some(lease)
	}

	/// Claims the exclusive activity. Returns `None` while another activity runs.
	pub fn begin(&mut self, activity: A) -> Option<ActivityLease> {
		if self.lease.is_some() {
			return None;
		}
		self.issued += 1;
		let lease = ActivityLease(self.issued);
		self.lease = Some(lease);
		self.activity = activity;
		Some(lease)
	}

	/// Changes the activity for the lease holder. A stale lease changes nothing.
	pub fn update(&mut self, lease: ActivityLease, activity: A) -> bool {
		if !self.holds(lease) {
			return false;
		}
		self.activity = activity;
		true
	}

	/// Releases the activity for the lease holder. A stale lease never sets idle.
	pub fn end(&mut self, lease: ActivityLease) -> bool {
		if !self.holds(lease) {
			return false;
		}
		self.lease = None;
		self.activity = A::default();
		true
	}
}
