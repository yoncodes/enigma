use std::sync::{Condvar, Mutex, OnceLock};

struct WorkerSlots {
    available: Mutex<usize>,
    changed: Condvar,
    capacity: usize,
}

impl WorkerSlots {
    fn new() -> Self {
        let capacity = std::thread::available_parallelism().map_or(1, |count| count.get());
        Self {
            available: Mutex::new(capacity),
            changed: Condvar::new(),
            capacity,
        }
    }

    fn acquire(&self, requested: usize) -> WorkerPermit<'_> {
        let mut available = self.available.lock().expect("worker slots poisoned");
        while *available == 0 {
            available = self.changed.wait(available).expect("worker slots poisoned");
        }
        let count = requested.min(*available);
        *available -= count;
        WorkerPermit { slots: self, count }
    }
}

struct WorkerPermit<'a> {
    slots: &'a WorkerSlots,
    count: usize,
}

impl Drop for WorkerPermit<'_> {
    fn drop(&mut self) {
        let mut available = self.slots.available.lock().expect("worker slots poisoned");
        *available = (*available + self.count).min(self.slots.capacity);
        self.slots.changed.notify_all();
    }
}

pub(super) fn run<T: Default>(job_count: usize, work: impl FnOnce(usize) -> T) -> T {
    if job_count == 0 {
        return T::default();
    }
    static SLOTS: OnceLock<WorkerSlots> = OnceLock::new();
    let permit = SLOTS.get_or_init(WorkerSlots::new).acquire(job_count);
    work(permit.count)
}
