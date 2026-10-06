use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use gradium::{Client, protocol::tts};
use tokio_tungstenite::tungstenite::Message;

#[derive(Clone)]
struct LogBuffer(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for LogBuffer {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(data);
        Ok(data.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn flushed_preserves_optional_request_identity() {
    for client_req_id in [None, Some("request-1")] {
        let mut value = serde_json::json!({"type": "flushed"});
        if let Some(id) = client_req_id {
            value["client_req_id"] = id.into();
        }
        let response: tts::Response = serde_json::from_value(value.clone()).unwrap();
        assert!(matches!(&response, tts::Response::Flushed { client_req_id: id }
            if id.as_deref() == client_req_id));
        assert_eq!(serde_json::to_value(response).unwrap(), value);
    }
}

#[tokio::test]
async fn one_shot_tts_keeps_audio_before_and_after_flush() -> anyhow::Result<()> {
    let logs = LogBuffer(Arc::new(Mutex::new(Vec::new())));
    let writer = logs.clone();
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_writer(move || writer.clone())
        .finish();
    let _logging = tracing::subscriber::set_default(subscriber);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server = tokio::spawn(async move {
        let (socket, _) = listener.accept().await?;
        let mut ws = tokio_tungstenite::accept_async(socket).await?;
        let setup = ws.next().await.unwrap()?.into_text()?;
        assert_eq!(serde_json::from_str::<serde_json::Value>(&setup)?["type"], "setup");
        ws.send(Message::Text(serde_json::json!({"type":"future_setup_event"}).to_string().into()))
            .await?;
        ws.send(Message::Text(
            serde_json::json!({
                "type": "ready", "model_name": "default", "sample_rate": 24000,
                "frame_size": 480, "audio_stream_names": ["audio"],
                "text_stream_names": ["text"], "request_id": "flush-test",
            })
            .to_string()
            .into(),
        ))
        .await?;
        let text = ws.next().await.unwrap()?.into_text()?;
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&text)?["text"],
            "Hello and welcome. <flush> How can I help you today?"
        );
        let eos = ws.next().await.unwrap()?.into_text()?;
        assert_eq!(serde_json::from_str::<serde_json::Value>(&eos)?["type"], "end_of_stream");
        for value in [
            serde_json::json!({"type": "audio", "audio": "AQI=", "start_s": 0.0,
                              "stop_s": 1.0, "stream_id": 0}),
            serde_json::json!({"type": "flushed"}),
            serde_json::json!({"type": "future_audio_event", "payload": {"new": true}}),
            serde_json::json!({"type": "audio", "audio": "AwQ=", "start_s": 1.0,
                              "stop_s": 2.0, "stream_id": 0}),
            serde_json::json!({"type": "end_of_stream"}),
        ] {
            ws.send(Message::Text(value.to_string().into())).await?;
        }
        Ok::<(), anyhow::Error>(())
    });
    let client = Client::new("test-key").with_base_url(&format!("http://{address}/api"))?;
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        client
            .tts("Hello and welcome. <flush> How can I help you today?", tts::Setup::new("voice")),
    )
    .await??;
    assert_eq!(result.raw_data(), [1, 2, 3, 4]);
    assert_eq!(result.sample_rate(), 24000);
    assert_eq!(result.request_id(), "flush-test");
    server.await??;
    let warnings = String::from_utf8(logs.0.lock().unwrap().clone())?;
    assert!(warnings.contains("WARN"));
    assert!(warnings.contains("future_setup_event"));
    assert!(warnings.contains("future_audio_event"));
    assert!(!warnings.contains("payload"));
    Ok(())
}

#[test]
fn unknown_response_types_default_to_unknown_but_known_malformed_messages_fail() {
    for value in [
        serde_json::json!({"type": "future_event"}),
        serde_json::json!({"type": "stats", "json_stats": "{}", "client_req_id": "request-1"}),
    ] {
        assert!(matches!(
            serde_json::from_value::<tts::Response>(value).unwrap(),
            tts::Response::Unknown
        ));
    }
    for value in [
        serde_json::json!({"type": "audio"}),
        serde_json::json!({"type": "ready"}),
        serde_json::json!({}),
        serde_json::json!({"type": 42}),
    ] {
        assert!(serde_json::from_value::<tts::Response>(value).is_err());
    }
}
