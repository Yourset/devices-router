use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BridgeEvent {
    ClientHello {
        role: ClientRole,
        #[serde(default, rename = "deviceId", skip_serializing_if = "Option::is_none")]
        device_id: Option<String>,
        #[serde(
            default,
            rename = "deviceName",
            skip_serializing_if = "Option::is_none"
        )]
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
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        capabilities: Vec<String>,
        #[serde(
            default,
            rename = "activityToken",
            skip_serializing_if = "Option::is_none"
        )]
        activity_token: Option<String>,
        #[serde(
            default,
            rename = "activityPort",
            skip_serializing_if = "Option::is_none"
        )]
        activity_port: Option<u16>,
    },
    Ping {
        message: String,
        #[serde(default, rename = "probeId", skip_serializing_if = "Option::is_none")]
        probe_id: Option<u64>,
        #[serde(default, rename = "replyTo", skip_serializing_if = "Option::is_none")]
        reply_to: Option<u64>,
    },
    MouseActivity {
        source: MouseSource,
        #[serde(
            default,
            rename = "activityId",
            skip_serializing_if = "Option::is_none"
        )]
        activity_id: Option<u64>,
        #[serde(
            default,
            rename = "targetEpoch",
            skip_serializing_if = "Option::is_none"
        )]
        target_epoch: Option<u64>,
    },
    MouseInput {
        event: MouseInputEvent,
    },
    TargetRequest {
        target: TargetSide,
    },
    TargetState {
        target: TargetSide,
        #[serde(
            default,
            rename = "targetEpoch",
            skip_serializing_if = "Option::is_none"
        )]
        target_epoch: Option<u64>,
    },
    ActivityChannelState {
        active: bool,
    },
    Key {
        action: KeyAction,
        key: String,
    },
    LinkStatsState {
        #[serde(
            default,
            rename = "currentRttMs",
            skip_serializing_if = "Option::is_none"
        )]
        current_rtt_ms: Option<u64>,
        #[serde(
            default,
            rename = "medianRttMs",
            skip_serializing_if = "Option::is_none"
        )]
        median_rtt_ms: Option<u64>,
        #[serde(default, rename = "jitterMs", skip_serializing_if = "Option::is_none")]
        jitter_ms: Option<u64>,
        #[serde(
            default,
            rename = "lossPercent",
            skip_serializing_if = "Option::is_none"
        )]
        loss_percent: Option<u64>,
        #[serde(rename = "sampleCount")]
        sample_count: u8,
    },
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ActivityDatagram {
    Hello {
        #[serde(rename = "deviceId")]
        device_id: String,
        #[serde(rename = "activityToken")]
        activity_token: String,
    },
    Activity {
        #[serde(rename = "deviceId")]
        device_id: String,
        #[serde(rename = "activityToken")]
        activity_token: String,
        #[serde(rename = "activityId")]
        activity_id: u64,
    },
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

#[allow(dead_code)]
pub fn encode_activity_datagram(datagram: &ActivityDatagram) -> anyhow::Result<Vec<u8>> {
    Ok(serde_json::to_vec(datagram)?)
}

#[allow(dead_code)]
pub fn decode_activity_datagram(payload: &[u8]) -> anyhow::Result<ActivityDatagram> {
    Ok(serde_json::from_slice(payload)?)
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
    let trimmed = payload.strip_suffix(b"\n").unwrap_or(payload);
    trimmed.strip_suffix(b"\r").unwrap_or(trimmed)
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
            reason: Some("two remote device limit reached".to_string()),
            max_devices: 2,
            capabilities: Vec::new(),
            activity_token: None,
            activity_port: None,
        };

        assert_eq!(decode_event(&encode_event(&event).unwrap()).unwrap(), event);
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
    fn latency_probe_and_reply_round_trip_without_breaking_legacy_ping() {
        let legacy = br#"{"type":"ping","message":"host-heartbeat"}"#;
        assert_eq!(
            decode_event(legacy).unwrap(),
            BridgeEvent::Ping {
                message: "host-heartbeat".to_string(),
                probe_id: None,
                reply_to: None,
            }
        );
        let probe = BridgeEvent::Ping {
            message: "latency-probe".to_string(),
            probe_id: Some(42),
            reply_to: None,
        };
        let reply = BridgeEvent::Ping {
            message: "latency-reply".to_string(),
            probe_id: None,
            reply_to: Some(42),
        };
        assert_eq!(decode_event(&encode_event(&probe).unwrap()).unwrap(), probe);
        assert_eq!(decode_event(&encode_event(&reply).unwrap()).unwrap(), reply);
    }

    #[test]
    fn target_state_round_trips() {
        let event = BridgeEvent::TargetState {
            target: TargetSide::Local,
            target_epoch: None,
        };

        let payload = encode_event(&event).unwrap();

        assert_eq!(decode_event(&payload).unwrap(), event);
    }

    #[test]
    fn activity_channel_state_round_trips() {
        let event = BridgeEvent::ActivityChannelState { active: true };

        let payload = encode_event(&event).unwrap();
        let json: serde_json::Value = serde_json::from_slice(payload.trim_ascii_end()).unwrap();

        assert_eq!(json["type"], "activity_channel_state");
        assert_eq!(json["active"], true);
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

    #[test]
    fn server_hello_supports_optional_activity_metadata() {
        let legacy = br#"{"type":"server_hello","accepted":true,"maxDevices":2}"#;
        assert_eq!(
            decode_event(legacy).unwrap(),
            BridgeEvent::ServerHello {
                accepted: true,
                reason: None,
                max_devices: 2,
                capabilities: Vec::new(),
                activity_token: None,
                activity_port: None,
            }
        );

        let event = BridgeEvent::ServerHello {
            accepted: true,
            reason: None,
            max_devices: 2,
            capabilities: vec!["udp_activity_v1".to_string()],
            activity_token: Some("token-123".to_string()),
            activity_port: Some(4567),
        };

        let payload = encode_event(&event).unwrap();
        let json: serde_json::Value = serde_json::from_slice(payload.trim_ascii_end()).unwrap();
        assert_eq!(json["capabilities"][0], "udp_activity_v1");
        assert_eq!(json["activityToken"], "token-123");
        assert_eq!(json["activityPort"], 4567);
        assert_eq!(decode_event(&payload).unwrap(), event);
    }

    #[test]
    fn mouse_activity_and_target_state_support_optional_epoch_metadata() {
        let legacy_activity = br#"{"type":"mouse_activity","source":"remote"}"#;
        let legacy_target = br#"{"type":"target_state","target":"remote"}"#;
        assert_eq!(
            decode_event(legacy_activity).unwrap(),
            BridgeEvent::MouseActivity {
                source: MouseSource::Remote,
                activity_id: None,
                target_epoch: None,
            }
        );
        assert_eq!(
            decode_event(legacy_target).unwrap(),
            BridgeEvent::TargetState {
                target: TargetSide::Remote,
                target_epoch: None,
            }
        );

        let activity = BridgeEvent::MouseActivity {
            source: MouseSource::Remote,
            activity_id: Some(77),
            target_epoch: Some(9),
        };
        let target = BridgeEvent::TargetState {
            target: TargetSide::Remote,
            target_epoch: Some(9),
        };

        let activity_payload = encode_event(&activity).unwrap();
        let target_payload = encode_event(&target).unwrap();

        assert_eq!(decode_event(&activity_payload).unwrap(), activity);
        assert_eq!(decode_event(&target_payload).unwrap(), target);
    }

    #[test]
    fn link_stats_state_round_trips() {
        let event = BridgeEvent::LinkStatsState {
            current_rtt_ms: Some(18),
            median_rtt_ms: Some(20),
            jitter_ms: Some(3),
            loss_percent: Some(25),
            sample_count: 4,
        };

        let payload = encode_event(&event).unwrap();
        let json: serde_json::Value = serde_json::from_slice(payload.trim_ascii_end()).unwrap();

        assert_eq!(json["currentRttMs"], 18);
        assert_eq!(json["medianRttMs"], 20);
        assert_eq!(json["jitterMs"], 3);
        assert_eq!(json["lossPercent"], 25);
        assert_eq!(json["sampleCount"], 4);
        assert_eq!(decode_event(&payload).unwrap(), event);
    }

    #[test]
    fn protocol_decoding_ignores_unknown_fields() {
        let payload = br#"{
            "type":"server_hello",
            "accepted":true,
            "maxDevices":2,
            "capabilities":["udp_activity_v1"],
            "activityToken":"token-123",
            "activityPort":4567,
            "ignored":"value"
        }"#;

        assert_eq!(
            decode_event(payload).unwrap(),
            BridgeEvent::ServerHello {
                accepted: true,
                reason: None,
                max_devices: 2,
                capabilities: vec!["udp_activity_v1".to_string()],
                activity_token: Some("token-123".to_string()),
                activity_port: Some(4567),
            }
        );
    }

    #[test]
    fn activity_datagrams_use_camel_case_without_tcp_newlines() {
        let hello = ActivityDatagram::Hello {
            device_id: "device-a".to_string(),
            activity_token: "token-123".to_string(),
        };
        let activity = ActivityDatagram::Activity {
            device_id: "device-a".to_string(),
            activity_token: "token-123".to_string(),
            activity_id: 42,
        };

        let hello_payload = encode_activity_datagram(&hello).unwrap();
        let activity_payload = encode_activity_datagram(&activity).unwrap();
        let hello_json: serde_json::Value = serde_json::from_slice(&hello_payload).unwrap();
        let activity_json: serde_json::Value = serde_json::from_slice(&activity_payload).unwrap();

        assert!(!hello_payload.ends_with(b"\n"));
        assert_eq!(hello_json["deviceId"], "device-a");
        assert_eq!(hello_json["activityToken"], "token-123");
        assert_eq!(activity_json["activityId"], 42);
        assert_eq!(decode_activity_datagram(&hello_payload).unwrap(), hello);
        assert_eq!(
            decode_activity_datagram(&activity_payload).unwrap(),
            activity
        );
    }
}
