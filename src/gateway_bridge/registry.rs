use std::sync::{Arc, Weak};

/// Weak ownership: a registry never keeps an idle publisher alive after its last
/// consumer closes. Call under the bridge's creation mutex.
pub(super) struct Registry<T> {
    workers: Vec<Weak<T>>,
}
impl<T> Default for Registry<T> {
    fn default() -> Self {
        Self {
            workers: Vec::new(),
        }
    }
}
impl<T> Registry<T> {
    pub fn get_or_try_insert<E>(
        &mut self,
        matches: impl Fn(&T) -> bool,
        create: impl FnOnce() -> Result<Arc<T>, E>,
    ) -> Result<Arc<T>, E> {
        self.workers.retain(|worker| worker.strong_count() > 0);
        if let Some(worker) = self
            .workers
            .iter()
            .filter_map(Weak::upgrade)
            .find(|worker| matches(worker))
        {
            return Ok(worker);
        }
        let worker = create()?;
        self.workers.push(Arc::downgrade(&worker));
        Ok(worker)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc,
    };
    struct Worker {
        key: u8,
        updates: mpsc::Sender<u8>,
        stopped: Arc<AtomicUsize>,
    }
    impl Drop for Worker {
        fn drop(&mut self) {
            self.stopped.fetch_add(1, Ordering::SeqCst);
        }
    }
    #[test]
    fn windows_share_one_worker_but_both_can_publish_and_last_close_releases_it() {
        let mut registry = Registry::default();
        let (updates, receiver) = mpsc::channel();
        let stopped = Arc::new(AtomicUsize::new(0));
        let first = registry
            .get_or_try_insert(
                |w: &Worker| w.key == 1,
                || {
                    Ok::<_, ()>(Arc::new(Worker {
                        key: 1,
                        updates,
                        stopped: stopped.clone(),
                    }))
                },
            )
            .unwrap();
        let second = registry
            .get_or_try_insert(|w: &Worker| w.key == 1, || Err(()))
            .unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        first.updates.send(1).unwrap();
        second.updates.send(2).unwrap();
        assert_eq!(receiver.recv().unwrap(), 1);
        assert_eq!(receiver.recv().unwrap(), 2);
        drop(first);
        assert_eq!(stopped.load(Ordering::SeqCst), 0);
        drop(second);
        assert_eq!(stopped.load(Ordering::SeqCst), 1);
        let (updates, _) = mpsc::channel();
        let next = registry
            .get_or_try_insert(
                |w: &Worker| w.key == 1,
                || {
                    Ok::<_, ()>(Arc::new(Worker {
                        key: 1,
                        updates,
                        stopped: stopped.clone(),
                    }))
                },
            )
            .unwrap();
        assert_eq!(registry.workers.len(), 1);
        drop(next);
        assert_eq!(stopped.load(Ordering::SeqCst), 2);
    }
}
