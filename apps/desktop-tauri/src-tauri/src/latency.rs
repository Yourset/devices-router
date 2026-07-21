use std::time::Instant;

#[derive(Debug, Default)]
pub struct LatencyProbeTracker {
    next_probe_id: u64,
    pending: Option<(u64, Instant)>,
}

impl LatencyProbeTracker {
    pub fn start_probe(&mut self, now: Instant) -> u64 {
        self.next_probe_id = self.next_probe_id.wrapping_add(1).max(1);
        self.pending = Some((self.next_probe_id, now));
        self.next_probe_id
    }

    pub fn complete_probe(&mut self, reply_to: u64, now: Instant) -> Option<u64> {
        let (probe_id, sent_at) = self.pending?;
        if probe_id != reply_to {
            return None;
        }
        self.pending = None;
        Some(now.saturating_duration_since(sent_at).as_millis() as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn tracker_accepts_only_the_current_probe_reply() {
        let start = Instant::now();
        let mut tracker = LatencyProbeTracker::default();
        let first = tracker.start_probe(start);
        let second = tracker.start_probe(start + Duration::from_millis(5));

        assert_ne!(first, second);
        assert_eq!(
            tracker.complete_probe(first, start + Duration::from_millis(8)),
            None
        );
        assert_eq!(
            tracker.complete_probe(second, start + Duration::from_millis(17)),
            Some(12)
        );
        assert_eq!(
            tracker.complete_probe(second, start + Duration::from_millis(18)),
            None
        );
    }
}
