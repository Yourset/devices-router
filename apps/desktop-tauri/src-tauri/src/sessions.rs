use crate::protocol::BridgeEvent;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::sync::mpsc;
use std::time::Instant;

pub const MAX_REMOTE_DEVICES: usize = 2;
const LEGACY_SLOT_ID: &str = "legacy";

#[derive(Clone, Debug)]
pub struct SessionIdentity {
    pub device_id: Option<String>,
    pub device_name: Option<String>,
    pub address: String,
}

#[derive(Clone, Debug)]
pub struct RegisterAcceptance {
    pub device_id: String,
    pub generation: u64,
    #[allow(dead_code)]
    pub legacy: bool,
    pub replaced: bool,
}

#[derive(Clone, Debug)]
pub enum RegisterResult {
    Accepted(RegisterAcceptance),
    Rejected(String),
}

#[cfg(test)]
impl RegisterResult {
    pub fn is_accepted(&self) -> bool {
        matches!(self, Self::Accepted(_))
    }

    pub fn accepted(self) -> Option<RegisterAcceptance> {
        match self {
            Self::Accepted(value) => Some(value),
            Self::Rejected(_) => None,
        }
    }

    pub fn rejection_reason(&self) -> Option<&str> {
        match self {
            Self::Accepted(_) => None,
            Self::Rejected(reason) => Some(reason),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceStatus {
    pub device_id: String,
    pub name: String,
    pub address: String,
    pub connected: bool,
    pub legacy: bool,
    pub last_activity_ago_ms: Option<u64>,
    pub latency_ms: Option<u64>,
}

#[derive(Clone, Debug)]
struct SessionRecord {
    generation: u64,
    device_name: String,
    address: String,
    legacy: bool,
    sender: mpsc::Sender<BridgeEvent>,
    last_activity: Option<Instant>,
    latency_ms: Option<u64>,
}

#[derive(Debug, Default)]
pub struct SessionRegistry {
    sessions: HashMap<String, SessionRecord>,
    next_generation: u64,
}

impl SessionRegistry {
    pub fn register(
        &mut self,
        identity: SessionIdentity,
        sender: mpsc::Sender<BridgeEvent>,
    ) -> RegisterResult {
        let supplied_id = identity
            .device_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let legacy = supplied_id.is_none();
        let device_id = supplied_id.unwrap_or(LEGACY_SLOT_ID).to_string();

        if legacy && self.sessions.contains_key(LEGACY_SLOT_ID) {
            return RegisterResult::Rejected("Studio PCStudio PCStudio PC?".to_string());
        }
        let replaced = self.sessions.contains_key(&device_id);
        if !replaced && self.sessions.len() >= MAX_REMOTE_DEVICES {
            return RegisterResult::Rejected("two remote device limit reached".to_string());
        }

        self.next_generation = self.next_generation.wrapping_add(1).max(1);
        let generation = self.next_generation;
        self.sessions.insert(
            device_id.clone(),
            SessionRecord {
                generation,
                device_name: identity
                    .device_name
                    .filter(|name| !name.trim().is_empty())
                    .unwrap_or_else(|| device_id.clone()),
                address: identity.address,
                legacy,
                sender,
                last_activity: None,
                latency_ms: None,
            },
        );
        RegisterResult::Accepted(RegisterAcceptance {
            device_id,
            generation,
            legacy,
            replaced,
        })
    }

    pub fn remove(&mut self, device_id: &str, generation: u64) -> bool {
        if self
            .sessions
            .get(device_id)
            .is_some_and(|session| session.generation == generation)
        {
            self.sessions.remove(device_id);
            return true;
        }
        false
    }

    pub fn contains(&self, device_id: &str) -> bool {
        self.sessions.contains_key(device_id)
    }

    pub fn generation_matches(&self, device_id: &str, generation: u64) -> bool {
        self.sessions
            .get(device_id)
            .is_some_and(|session| session.generation == generation)
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    pub fn clear(&mut self) {
        self.sessions.clear();
    }

    pub fn sender_for(&self, device_id: &str) -> Option<mpsc::Sender<BridgeEvent>> {
        self.sessions
            .get(device_id)
            .map(|session| session.sender.clone())
    }

    pub fn senders(&self) -> Vec<(String, mpsc::Sender<BridgeEvent>)> {
        self.sessions
            .iter()
            .map(|(id, session)| (id.clone(), session.sender.clone()))
            .collect()
    }

    pub fn mark_activity(&mut self, device_id: &str, now: Instant) -> bool {
        let Some(session) = self.sessions.get_mut(device_id) else {
            return false;
        };
        session.last_activity = Some(now);
        true
    }

    pub fn record_latency(&mut self, device_id: &str, generation: u64, sample_ms: u64) -> bool {
        let Some(session) = self.sessions.get_mut(device_id) else {
            return false;
        };
        if session.generation != generation {
            return false;
        }
        session.latency_ms = Some(smooth_latency(session.latency_ms, sample_ms));
        true
    }

    pub fn snapshots(&self, aliases: &BTreeMap<String, String>) -> Vec<DeviceStatus> {
        let now = Instant::now();
        let mut devices = self
            .sessions
            .iter()
            .map(|(device_id, session)| DeviceStatus {
                device_id: device_id.clone(),
                name: aliases
                    .get(device_id)
                    .filter(|alias| !alias.trim().is_empty())
                    .cloned()
                    .unwrap_or_else(|| session.device_name.clone()),
                address: session.address.clone(),
                connected: true,
                legacy: session.legacy,
                last_activity_ago_ms: session
                    .last_activity
                    .map(|last| now.saturating_duration_since(last).as_millis() as u64),
                latency_ms: session.latency_ms,
            })
            .collect::<Vec<_>>();
        devices.sort_by(|left, right| left.device_id.cmp(&right.device_id));
        devices
    }
}

pub(crate) fn smooth_latency(previous_ms: Option<u64>, sample_ms: u64) -> u64 {
    previous_ms.map_or(sample_ms, |previous| {
        previous.saturating_mul(3).saturating_add(sample_ms) / 4
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::BridgeEvent;
    use std::sync::mpsc;

    fn identity(id: Option<&str>, name: &str, address: &str) -> SessionIdentity {
        SessionIdentity {
            device_id: id.map(str::to_string),
            device_name: Some(name.to_string()),
            address: address.to_string(),
        }
    }

    #[test]
    fn accepts_two_distinct_devices_and_rejects_third() {
        let mut registry = SessionRegistry::default();
        let (tx, _) = mpsc::channel::<BridgeEvent>();

        assert!(registry
            .register(identity(Some("a"), "A", "10.0.0.1"), tx.clone())
            .is_accepted());
        assert!(registry
            .register(identity(Some("b"), "B", "10.0.0.2"), tx.clone())
            .is_accepted());
        let rejected = registry.register(identity(Some("c"), "C", "10.0.0.3"), tx);

        assert_eq!(registry.len(), 2);
        assert_eq!(
            rejected.rejection_reason(),
            Some("two remote device limit reached")
        );
    }

    #[test]
    fn reconnect_replaces_same_device_and_stale_cleanup_cannot_remove_it() {
        let mut registry = SessionRegistry::default();
        let (first_tx, _) = mpsc::channel::<BridgeEvent>();
        let first = registry
            .register(identity(Some("a"), "A", "10.0.0.1"), first_tx)
            .accepted()
            .unwrap();
        let (second_tx, _) = mpsc::channel::<BridgeEvent>();
        let second = registry
            .register(identity(Some("a"), "A2", "10.0.0.9"), second_tx)
            .accepted()
            .unwrap();

        assert!(second.generation > first.generation);
        assert!(!registry.remove(&first.device_id, first.generation));
        assert!(registry.contains(&second.device_id));
        assert!(registry.remove(&second.device_id, second.generation));
    }

    #[test]
    fn only_one_unidentified_legacy_device_is_allowed() {
        let mut registry = SessionRegistry::default();
        let (tx, _) = mpsc::channel::<BridgeEvent>();
        let first = registry
            .register(identity(None, "old-a", "10.0.0.1"), tx.clone())
            .accepted()
            .unwrap();
        let second = registry.register(identity(None, "old-b", "10.0.0.2"), tx);

        assert!(first.legacy);
        assert_eq!(
            second.rejection_reason(),
            Some("Studio PCStudio PCStudio PC?")
        );
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn device_snapshot_uses_host_alias_when_present() {
        let mut registry = SessionRegistry::default();
        let (tx, _) = mpsc::channel::<BridgeEvent>();
        registry.register(identity(Some("a"), "Windows-A", "10.0.0.1"), tx);
        let aliases =
            std::collections::BTreeMap::from([("a".to_string(), "Studio PC".to_string())]);

        let devices = registry.snapshots(&aliases);

        assert_eq!(devices[0].name, "Studio PC");
        assert_eq!(devices[0].device_id, "a");
        assert!(devices[0].connected);
    }

    #[test]
    fn latency_is_smoothed_per_device_and_stale_generation_is_ignored() {
        let mut registry = SessionRegistry::default();
        let (tx, _) = mpsc::channel::<BridgeEvent>();
        let first = registry
            .register(identity(Some("a"), "Windows-A", "10.0.0.1"), tx.clone())
            .accepted()
            .unwrap();
        let second = registry
            .register(identity(Some("b"), "Windows-B", "10.0.0.2"), tx.clone())
            .accepted()
            .unwrap();

        assert!(registry.record_latency("a", first.generation, 8));
        assert!(registry.record_latency("a", first.generation, 12));
        assert!(registry.record_latency("b", second.generation, 30));
        assert!(!registry.record_latency("a", first.generation.wrapping_add(1), 99));

        let devices = registry.snapshots(&BTreeMap::new());
        assert_eq!(devices[0].latency_ms, Some(9));
        assert_eq!(devices[1].latency_ms, Some(30));
    }

    #[test]
    fn reconnect_clears_previous_latency() {
        let mut registry = SessionRegistry::default();
        let (tx, _) = mpsc::channel::<BridgeEvent>();
        let first = registry
            .register(identity(Some("a"), "Windows-A", "10.0.0.1"), tx.clone())
            .accepted()
            .unwrap();
        assert!(registry.record_latency("a", first.generation, 8));

        registry.register(identity(Some("a"), "Windows-A", "10.0.0.1"), tx);

        assert_eq!(registry.snapshots(&BTreeMap::new())[0].latency_ms, None);
    }
}
