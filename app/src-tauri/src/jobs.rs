//! Long computations the interface can cancel, by request id.
//!
//! A job registers its id with [`Jobs::start`] and polls the returned flag;
//! `cancel_job` sets it with [`Jobs::cancel`]. A cancel can reach the shell
//! before the job's own command does, so it leaves a raised flag that
//! `start` then picks up. The guard removes the entry when the job ends. A
//! cancel that arrives after its job ended leaves one flag behind, which no
//! later job reads: the interface takes job ids from the clock, so it never
//! reuses one, not even after a reload.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// The cancel flags of the jobs in flight, and of cancels that arrived
/// before their job.
#[derive(Debug, Default)]
pub struct Jobs(Mutex<HashMap<u64, Arc<AtomicBool>>>);

/// A running job's handle: its cancel flag, unregistered on drop.
#[derive(Debug)]
pub struct Job<'a> {
    jobs: &'a Jobs,
    id: u64,
    flag: Arc<AtomicBool>,
}

impl Jobs {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers job `id`, already cancelled if a cancel for it came first.
    pub fn start(&self, id: u64) -> Job<'_> {
        let flag = self.lock().entry(id).or_default().clone();
        Job {
            jobs: self,
            id,
            flag,
        }
    }

    /// Cancels job `id`, now or as soon as it starts. Idempotent.
    pub fn cancel(&self, id: u64) {
        self.lock()
            .entry(id)
            .or_default()
            .store(true, Ordering::Relaxed);
    }

    /// How many ids are registered.
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<u64, Arc<AtomicBool>>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl Job<'_> {
    pub fn cancelled(&self) -> bool {
        self.flag.load(Ordering::Relaxed)
    }
}

impl Drop for Job<'_> {
    fn drop(&mut self) {
        self.jobs.lock().remove(&self.id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cancel_reaches_the_running_job_and_the_entry_goes_with_it() {
        let jobs = Jobs::new();
        let job = jobs.start(7);
        assert!(!job.cancelled());
        jobs.cancel(7);
        assert!(job.cancelled());
        drop(job);
        assert!(jobs.is_empty());
    }

    #[test]
    fn a_cancel_before_the_job_starts_still_cancels_it() {
        let jobs = Jobs::new();
        jobs.cancel(3);
        jobs.cancel(3);
        let job = jobs.start(3);
        assert!(job.cancelled());
        drop(job);
        assert_eq!(jobs.len(), 0);
    }

    #[test]
    fn other_jobs_are_untouched() {
        let jobs = Jobs::new();
        let a = jobs.start(1);
        let b = jobs.start(2);
        jobs.cancel(2);
        assert!(!a.cancelled());
        assert!(b.cancelled());
    }
}
