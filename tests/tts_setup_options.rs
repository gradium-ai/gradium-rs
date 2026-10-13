use futures_util::{SinkExt, StreamExt};
use gradium::{Client, protocol::tts};
use serde_json::json;
use tokio_tungstenite::tungstenite::Message;

#[tokio::test]
async fn custom_setup_and_multiple_audio_streams_round_trip() -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server = tokio::spawn(async move {
        let (socket, _) = listener.accept().await?;
        let mut websocket = tokio_tungstenite::accept_async(socket).await?;
        let setup: serde_json::Value =
            serde_json::from_str(&websocket.next().await.unwrap()?.into_text()?)?;
        assert_eq!(setup["voice_emb"], "AQI=");
        assert_eq!(setup["pronunciation_id"], "pronunciation");
        assert_eq!(setup["additional_voice_ids"], json!(["speaker-2"]));
        assert_eq!(setup["additional_voice_embs"], json!(["AwQ="]));
        assert_eq!(setup["retry_for_s"], 2.0);
        websocket
            .send(Message::Text(
                json!({
                    "type":"ready", "model_name":"default", "sample_rate":24000,
                    "frame_size":480, "request_id":"multi-test",
                    "audio_stream_names":["speaker-1","speaker-2"], "text_stream_names":[]
                })
                .to_string()
                .into(),
            ))
            .await?;
        for _ in 0..2 {
            let message: serde_json::Value =
                serde_json::from_str(&websocket.next().await.unwrap()?.into_text()?)?;
            assert_eq!(message["client_req_id"], "custom-request");
        }
        for (stream_id, audio) in [(1, "BQY="), (0, "AQI="), (1, "Bwg="), (0, "AwQ=")] {
            websocket
                .send(Message::Text(
                    json!({
                        "type":"audio", "stream_id":stream_id, "audio":audio,
                        "start_s":0.0, "stop_s":1.0
                    })
                    .to_string()
                    .into(),
                ))
                .await?;
        }
        websocket.send(Message::Text(json!({"type":"end_of_stream"}).to_string().into())).await?;
        Ok::<(), anyhow::Error>(())
    });
    let client = Client::new("test-key").with_base_url(&format!("http://{address}/api"))?;
    let setup = tts::Setup::default()
        .with_voice_emb("AQI=")
        .with_pronunciation_id("pronunciation")
        .with_additional_voice_ids(vec!["speaker-2".into()])
        .with_additional_voice_embs(vec!["AwQ=".into()])
        .with_retry_for_s(2.0)
        .with_client_req_id("custom-request");
    let result =
        tokio::time::timeout(std::time::Duration::from_secs(5), client.tts("Hello", setup))
            .await??;
    assert_eq!(result.raw_data(), [1, 2, 3, 4]);
    assert_eq!(result.audio_streams().len(), 2);
    assert_eq!(result.audio_streams()[0].name.as_deref(), Some("speaker-1"));
    assert_eq!(result.audio_streams()[1].stream_id, 1);
    assert_eq!(result.audio_streams()[1].name.as_deref(), Some("speaker-2"));
    assert_eq!(result.audio_streams()[1].data, [5, 6, 7, 8]);
    server.await??;
    Ok(())
}

#[test]
fn default_setup_omits_custom_voice_options() {
    let setup = serde_json::to_value(tts::Setup::default()).unwrap();
    for field in ["voice_emb", "additional_voice_ids", "additional_voice_embs", "retry_for_s"] {
        assert!(setup.get(field).is_none());
    }
}
