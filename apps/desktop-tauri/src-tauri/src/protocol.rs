use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BridgeEvent {
    ClientHello {
        role: ClientRole,
        #[serde(default, rename = "deviceId", skip_serializing_if = "Option::is_none")]
        device_id: Option<String>,
        #[serde(default, rename = "deviceName", skip_serializing_if = "Option::is_none")]
        device_name: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        capabilities: Vec<String>,
    },
    ServerHello {
        accepted: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
        #[serde(rename = "maxDevices")]
        max_devices: u8,
    },
    Ping { message: String },
    MouseActivity { source: MouseSource },
    MouseInput { event: MouseInputEvent },
    TargetRequest { target: TargetSide },
    TargetState { target: TargetSide },
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
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MouseInputEvent {
    MoveRelative {
        dx: i32,
        dy: i32,
    },
    MoveAbsolute {
        x: i32,
        y: i32,
    },
    MoveToLeftEdge {
        y_permille: u16,
    },
    Wheel {
        delta: i32,
    },
    HWheel {
        delta: i32,
    },
    Button {
        button: MouseButton,
        action: MouseButtonAction,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MouseButtonAction {
    Down,
    Up,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TargetSide {
    Local,
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

#[cfg(test)]
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
            device_id: Some("device-a".to_string()),
            device_name: Some("Office-PC".to_string()),
            capabilities: vec!["multi_remote_v1".to_string()],
        };

        let payload = encode_event(&event).unwrap();

        assert!(payload.ends_with(b"\n"));
        assert_eq!(decode_event(&payload).unwrap(), event);
    }

    #[test]
    fn legacy_client_hello_without_identity_is_accepted() {
        let payload = br#"{"type":"client_hello","role":"remote"}"#;

        assert_eq!(
            decode_event(payload).unwrap(),
            BridgeEvent::ClientHello {
                role: ClientRole::Remote,
                device_id: None,
                device_name: None,
                capabilities: Vec::new(),
            }
        );
    }

    #[test]
    fn server_hello_rejection_round_trips() {
        let event = BridgeEvent::ServerHello {
            accepted: false,
            reason: Some("??????????".to_string()),
            max_devices: 2,
        };

        assert_eq!(
            decode_event(&encode_event(&event).unwrap()).unwrap(),
            event
        );
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

    #[test]
    fn target_request_round_trips() {
        let event = BridgeEvent::TargetRequest {
            target: TargetSide::Remote,
        };

        let payload = encode_event(&event).unwrap();

        assert_eq!(decode_event(&payload).unwrap(), event);
    }

    #[test]
    fn target_state_round_trips() {
        let event = BridgeEvent::TargetState {
            target: TargetSide::Local,
        };

        let payload = encode_event(&event).unwrap();

        assert_eq!(decode_event(&payload).unwrap(), event);
    }

    #[test]
    fn mouse_input_round_trips() {
        let event = BridgeEvent::MouseInput {
            event: MouseInputEvent::Button {
                button: MouseButton::Left,
                action: MouseButtonAction::Down,
            },
        };

        let payload = encode_event(&event).unwrap();

        assert_eq!(decode_event(&payload).unwrap(), event);
    }

    #[test]
    fn mouse_relative_move_round_trips_signed_delta() {
        let event = BridgeEvent::MouseInput {
            event: MouseInputEvent::MoveRelative { dx: -12, dy: 7 },
        };

        let payload = encode_event(&event).unwrap();

        assert_eq!(decode_event(&payload).unwrap(), event);
    }

    #[test]
    fn mouse_absolute_move_round_trips_coordinates() {
        let event = BridgeEvent::MouseInput {
            event: MouseInputEvent::MoveAbsolute { x: 10, y: 200 },
        };

        let payload = encode_event(&event).unwrap();

        assert_eq!(decode_event(&payload).unwrap(), event);
    }

    #[test]
    fn mouse_left_edge_move_round_trips_vertical_ratio() {
        let event = BridgeEvent::MouseInput {
            event: MouseInputEvent::MoveToLeftEdge { y_permille: 500 },
        };

        let payload = encode_event(&event).unwrap();

        assert_eq!(decode_event(&payload).unwrap(), event);
    }
}
