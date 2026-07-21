use std::time::{Duration, Instant};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KeyboardTarget {
    Local,
    Remote,
    Device(String),
}

impl KeyboardTarget {
    pub fn as_status_value(&self) -> String {
        match self {
            Self::Local => "local".to_string(),
            Self::Remote => "remote".to_string(),
            Self::Device(device_id) => device_id.clone(),
        }
    }

    pub fn device_id(&self) -> Option<&str> {
        match self {
            Self::Device(device_id) => Some(device_id),
            Self::Local | Self::Remote => None,
        }
    }

    pub fn is_remote(&self) -> bool {
        !matches!(self, Self::Local)
    }
}

#[derive(Clone, Debug)]
pub struct ActivityArbiter {
    current: KeyboardTarget,
    pending: Option<KeyboardTarget>,
    debounce: Duration,
    last_switch: Instant,
}

impl ActivityArbiter {
    pub fn ready(current: KeyboardTarget, debounce: Duration, now: Instant) -> Self {
        Self {
            current,
            pending: None,
            debounce,
            last_switch: now.checked_sub(debounce).unwrap_or(now),
        }
    }

    #[cfg(test)]
    pub fn current(&self) -> &KeyboardTarget {
        &self.current
    }

    pub fn observe(&mut self, target: KeyboardTarget, now: Instant) -> Option<KeyboardTarget> {
        if target == self.current {
            self.pending = None;
            return None;
        }
        if now.saturating_duration_since(self.last_switch) >= self.debounce {
            self.current = target.clone();
            self.pending = None;
            self.last_switch = now;
            return Some(target);
        }
        self.pending = Some(target);
        None
    }

    pub fn poll(&mut self, now: Instant) -> Option<KeyboardTarget> {
        if now.saturating_duration_since(self.last_switch) < self.debounce {
            return None;
        }
        let target = self.pending.take()?;
        if target == self.current {
            return None;
        }
        self.current = target.clone();
        self.last_switch = now;
        Some(target)
    }

    pub fn force(&mut self, target: KeyboardTarget, now: Instant) -> Option<KeyboardTarget> {
        self.pending = None;
        if target == self.current {
            return None;
        }
        self.current = target.clone();
        self.last_switch = now;
        Some(target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn device(id: &str) -> KeyboardTarget {
        KeyboardTarget::Device(id.to_string())
    }

    #[test]
    fn ready_arbiter_switches_to_remote_immediately() {
        let start = Instant::now();
        let mut arbiter =
            ActivityArbiter::ready(KeyboardTarget::Local, Duration::from_millis(30), start);

        assert_eq!(arbiter.observe(device("a"), start), Some(device("a")));
        assert_eq!(arbiter.current(), &device("a"));
    }

    #[test]
    fn activity_during_debounce_is_applied_when_window_expires() {
        let start = Instant::now();
        let mut arbiter =
            ActivityArbiter::ready(KeyboardTarget::Local, Duration::from_millis(30), start);
        arbiter.observe(device("a"), start);

        assert_eq!(
            arbiter.observe(device("b"), start + Duration::from_millis(10)),
            None
        );
        assert_eq!(arbiter.poll(start + Duration::from_millis(29)), None);
        assert_eq!(
            arbiter.poll(start + Duration::from_millis(30)),
            Some(device("b"))
        );
    }

    #[test]
    fn latest_activity_wins_inside_debounce_window() {
        let start = Instant::now();
        let mut arbiter =
            ActivityArbiter::ready(KeyboardTarget::Local, Duration::from_millis(30), start);
        arbiter.observe(device("a"), start);
        arbiter.observe(device("b"), start + Duration::from_millis(10));
        arbiter.observe(KeyboardTarget::Local, start + Duration::from_millis(20));

        assert_eq!(
            arbiter.poll(start + Duration::from_millis(30)),
            Some(KeyboardTarget::Local)
        );
        assert_eq!(arbiter.current(), &KeyboardTarget::Local);
    }
}
