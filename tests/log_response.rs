use gradium::protocol::{stt, tts};

#[test]
fn log_messages_decode_on_both_protocols() {
    let value = serde_json::json!({
        "type": "log", "level": "warning", "message": "unused json_config keys: tmep",
        "request_id": "abc"
    });
    match serde_json::from_value::<tts::Response>(value.clone()).unwrap() {
        tts::Response::Log(log) => {
            assert_eq!(log.level, "warning");
            assert_eq!(log.message, "unused json_config keys: tmep");
            assert_eq!(log.client_req_id, None);
        }
        other => panic!("unexpected {other:?}"),
    }
    match serde_json::from_value::<stt::Response>(value).unwrap() {
        stt::Response::Log(log) => assert_eq!(log.level, "warning"),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn unknown_stt_response_types_default_to_unknown_but_known_malformed_messages_fail() {
    let value = serde_json::json!({"type": "future_event", "payload": 1});
    assert!(matches!(
        serde_json::from_value::<stt::Response>(value).unwrap(),
        stt::Response::Unknown
    ));
    for value in [
        serde_json::json!({"type": "text"}),
        serde_json::json!({"type": "ready"}),
        serde_json::json!({"type": "log"}),
        serde_json::json!({}),
    ] {
        assert!(serde_json::from_value::<stt::Response>(value).is_err());
    }
}
