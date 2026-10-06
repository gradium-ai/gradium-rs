use futures_util::{SinkExt, StreamExt};
use gradium::{Client, protocol::tts};
use tokio_tungstenite::tungstenite::Message;

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
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server = tokio::spawn(async move {
        let (socket, _) = listener.accept().await?;
        let mut ws = tokio_tungstenite::accept_async(socket).await?;
        let setup = ws.next().await.unwrap()?.into_text()?;
        assert_eq!(serde_json::from_str::<serde_json::Value>(&setup)?["type"], "setup");
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
    Ok(())
}
