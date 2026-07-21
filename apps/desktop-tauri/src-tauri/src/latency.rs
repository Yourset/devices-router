use serde::Serialize;
use std::collections::VecDeque;
use std::time::Instant;

const WINDOW_SIZE: usize = 20;
pub const HOST_LATENCY_CAPABILITY: &str = "host_latency_v2";

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkStats {
    pub current_rtt_ms: Option<u64>,
    pub median_rtt_ms: Option<u64>,
    pub jitter_ms: Option<u64>,
    pub loss_percent: u8,
    pub sample_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProbeOutcome {
    Success(u64),
    Loss,
}

#[derive(Debug, Default)]
pub struct LatencyProbeTracker {
    next_probe_id: u64,
    pending: Option<(u64, Instant)>,
    recent_attempts: VecDeque<ProbeOutcome>,
}

#[cfg_attr(not(test), allow(dead_code))]
impl LatencyProbeTracker {
    pub fn start_probe(&mut self, now: Instant) -> u64 {
        if self.pending.is_some() {
            self.record_attempt(ProbeOutcome::Loss);
        }
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
        let sample_ms = now.saturating_duration_since(sent_at).as_millis() as u64;
        self.record_attempt(ProbeOutcome::Success(sample_ms));
        Some(sample_ms)
    }

    pub fn mark_timed_out(&mut self) -> Option<u64> {
        let (probe_id, _) = self.pending.take()?;
        self.record_attempt(ProbeOutcome::Loss);
        Some(probe_id)
    }

    pub fn stats(&self) -> LinkStats {
        let successful_samples = self.successful_samples();
        let current_rtt_ms = successful_samples.last().copied();
        let median_rtt_ms = lower_median(&successful_samples);
        let jitter_ms = match successful_samples.len() {
            0 => None,
            1 => Some(0),
            _ => {
                let jitter_samples = successful_samples
                    .windows(2)
                    .map(|pair| pair[0].abs_diff(pair[1]))
                    .collect::<Vec<_>>();
                lower_median(&jitter_samples)
            }
        };
        let attempt_count = self.recent_attempts.len();
        let loss_count = self
            .recent_attempts
            .iter()
            .filter(|outcome| matches!(outcome, ProbeOutcome::Loss))
            .count();

        LinkStats {
            current_rtt_ms,
            median_rtt_ms,
            jitter_ms,
            loss_percent: loss_count
                .checked_mul(100)
                .and_then(|loss_percent| loss_percent.checked_div(attempt_count))
                .and_then(|loss_percent| u8::try_from(loss_percent).ok())
                .unwrap_or(0),
            sample_count: successful_samples.len(),
        }
    }

    pub fn reset(&mut self) {
        self.pending = None;
        self.recent_attempts.clear();
    }

    fn record_attempt(&mut self, outcome: ProbeOutcome) {
        if self.recent_attempts.len() == WINDOW_SIZE {
            self.recent_attempts.pop_front();
        }
        self.recent_attempts.push_back(outcome);
    }

    fn successful_samples(&self) -> Vec<u64> {
        self.recent_attempts
            .iter()
            .filter_map(|outcome| match outcome {
                ProbeOutcome::Success(sample_ms) => Some(*sample_ms),
                ProbeOutcome::Loss => None,
            })
            .collect()
    }
}

#[cfg_attr(not(test), allow(dead_code))]
fn lower_median(samples: &[u64]) -> Option<u64> {
    if samples.is_empty() {
        return None;
    }
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    Some(sorted[(sorted.len() - 1) / 2])
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

    #[test]
    fn tracker_reports_recent_link_stats_and_overwrite_loss() {
        let start = Instant::now();
        let mut tracker = LatencyProbeTracker::default();

        let first = tracker.start_probe(start);
        assert_eq!(
            tracker.complete_probe(first, start + Duration::from_millis(10)),
            Some(10)
        );

        let second = tracker.start_probe(start + Duration::from_millis(20));
        let third = tracker.start_probe(start + Duration::from_millis(30));
        assert_ne!(second, third);
        assert_eq!(
            tracker.complete_probe(third, start + Duration::from_millis(55)),
            Some(25)
        );

        let fourth = tracker.start_probe(start + Duration::from_millis(60));
        assert_eq!(
            tracker.complete_probe(fourth, start + Duration::from_millis(100)),
            Some(40)
        );

        assert_eq!(
            tracker.stats(),
            LinkStats {
                current_rtt_ms: Some(40),
                median_rtt_ms: Some(25),
                jitter_ms: Some(15),
                loss_percent: 25,
                sample_count: 3,
            }
        );
    }

    #[test]
    fn tracker_ignores_bad_replies_supports_timeout_and_reset() {
        let start = Instant::now();
        let mut tracker = LatencyProbeTracker::default();

        let first = tracker.start_probe(start);
        assert_eq!(
            tracker.complete_probe(first + 1, start + Duration::from_millis(5)),
            None
        );
        assert_eq!(tracker.mark_timed_out(), Some(first));
        assert_eq!(
            tracker.complete_probe(first, start + Duration::from_millis(10)),
            None
        );

        let second = tracker.start_probe(start + Duration::from_millis(20));
        assert_eq!(
            tracker.complete_probe(second, start + Duration::from_millis(32)),
            Some(12)
        );
        assert_eq!(
            tracker.complete_probe(second, start + Duration::from_millis(35)),
            None
        );
        assert_eq!(
            tracker.stats(),
            LinkStats {
                current_rtt_ms: Some(12),
                median_rtt_ms: Some(12),
                jitter_ms: Some(0),
                loss_percent: 50,
                sample_count: 1,
            }
        );

        tracker.reset();
        assert_eq!(tracker.stats(), LinkStats::default());
    }

    #[test]
    fn tracker_keeps_only_last_twenty_attempts() {
        let start = Instant::now();
        let mut tracker = LatencyProbeTracker::default();

        for index in 0..21 {
            let sent_at = start + Duration::from_millis(index * 10);
            let probe_id = tracker.start_probe(sent_at);
            assert_eq!(
                tracker.complete_probe(probe_id, sent_at + Duration::from_millis(10 + index)),
                Some(10 + index)
            );
        }

        assert_eq!(
            tracker.stats(),
            LinkStats {
                current_rtt_ms: Some(30),
                median_rtt_ms: Some(20),
                jitter_ms: Some(1),
                loss_percent: 0,
                sample_count: 20,
            }
        );
    }
}
