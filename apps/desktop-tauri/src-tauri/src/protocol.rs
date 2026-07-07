use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BridgeEvent {
    ClientHello { role: ClientRole },
    Ping { message: String },
    MouseActivity { source: MouseSource },
    Key { action: KeyAction, key: String },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ClientRole {
    Remote,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MouseSource {
    Host,
    Remote,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyAction {
    Down,
    Up,
}

pub fn encode_event(event: &BridgeEvent) -> anyhow::Result<Vec<u8>> {
    let mut payload = serde_json::to_vec(event)?;
    payload.push(b'\n');
    Ok(payload)
}

pub fn decode_event(payload: &[u8]) -> anyhow::Result<BridgeEvent> {
    let trimmed = trim_newline(payload);
    Ok(serde_json::from_slice(trimmed)?)
}

pub fn is_legacy_silent_lan_client(peer_ip: &str) -> bool {
    peer_ip != "127.0.0.1" && peer_ip != "::1"
}

pub fn encode_discovery(port: u16) -> String {
    format!("devices-router-host:{port}")
}

pub fn decode_discovery(payload: &str) -> Option<u16> {
    payload
        .trim()
        .strip_prefix("devices-router-host:")
        .and_then(|port| port.parse::<u16>().ok())
}

fn trim_newline(payload: &[u8]) -> &[u8] {
    payload
        .strip_suffix(b"\n")
        .unwrap_or(payload)
        .strip_suffix(b"\r")
        .unwrap_or(payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_hello_round_trips() {
        let event = BridgeEvent::ClientHello {
            role: ClientRole::Remote,
        };

        let payload = encode_event(&event).unwrap();

        assert!(payload.ends_with(b"\n"));
        assert_eq!(decode_event(&payload).unwrap(), event);
    }

    #[test]
    fn legacy_compatibility_allows_lan_but_rejects_loopback() {
        assert!(is_legacy_silent_lan_client("192.168.31.54"));
        assert!(!is_legacy_silent_lan_client("127.0.0.1"));
        assert!(!is_legacy_silent_lan_client("::1"));
    }

    #[test]
    fn discovery_round_trips_port() {
        let payload = encode_discovery(8765);

        assert_eq!(decode_discovery(&payload), Some(8765));
        assert_eq!(decode_discovery("not-devices-router"), None);
    }
}
