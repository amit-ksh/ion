use std::time::Duration;

/// Short bounded idle delay keeps mobile controls responsive. Connectivity and
/// revocation failures back off independently; local changes cannot defeat it.
#[derive(Default)]
pub struct Cadence {
    idle: u8,
    failures: u8,
}
impl Cadence {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn can_wake(&self) -> bool {
        self.failures == 0
    }
    pub fn should_poll(
        &self,
        elapsed: Duration,
        delay: Duration,
        rotated: bool,
        changed: bool,
    ) -> bool {
        rotated || elapsed >= delay || (changed && self.can_wake())
    }
    pub fn observe<E>(&mut self, result: &Result<bool, E>) -> Duration {
        match result {
            Err(_) => {
                self.idle = 0;
                self.failures = self.failures.saturating_add(1);
                Duration::from_secs((5u64 << self.failures.min(6)).min(300))
            }
            Ok(true) => {
                self.reset();
                Duration::from_secs(5)
            }
            Ok(false) => {
                self.failures = 0;
                self.idle = self.idle.saturating_add(1);
                Duration::from_secs(if self.idle == 1 { 5 } else { 10 })
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn idle_is_bounded_and_activity_and_reconnect_recover_promptly() {
        let mut c = Cadence::default();
        assert_eq!(c.observe::<()>(&Ok(false)).as_secs(), 5);
        for _ in 0..1000 {
            assert_eq!(c.observe::<()>(&Ok(false)).as_secs(), 10);
        }
        assert_eq!(c.observe::<()>(&Ok(true)).as_secs(), 5);
        for expected in [10, 20, 40, 80, 160, 300, 300] {
            assert_eq!(c.observe(&Err::<bool, _>(())).as_secs(), expected);
            assert!(!c.can_wake());
        }
        c.reset();
        assert!(c.can_wake());
        assert_eq!(c.observe::<()>(&Ok(true)).as_secs(), 5);
        assert!(c.should_poll(Duration::ZERO, Duration::from_secs(10), false, true));
        c.observe(&Err::<bool, _>(()));
        assert!(!c.should_poll(Duration::ZERO, Duration::from_secs(10), false, true));
        assert!(c.should_poll(Duration::ZERO, Duration::from_secs(10), true, false));
        assert!(c.should_poll(
            Duration::from_secs(10),
            Duration::from_secs(10),
            false,
            false
        ));
    }
}
