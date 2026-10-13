use futures_util::{SinkExt, StreamExt};
use gradium::{Client, protocol::stt};
use serde_json::json;
use tokio_tungstenite::tungstenite::Message;

#[tokio::test]
async fn setup_audio_flush_and_eos_round_trip() -> anyhow::Result<()> {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        for split in [false, true] {
            for correlated in [false, true] {
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
                let address = listener.local_addr()?;
                let server = tokio::spawn(async move {
                    let (socket, _) = listener.accept().await?;
                    let mut ws = tokio_tungstenite::accept_async(socket).await?;
                    let setup: serde_json::Value =
                        serde_json::from_str(&ws.next().await.unwrap()?.into_text()?)?;
                    assert_eq!(setup["type"], "setup");
                    assert_eq!(setup["keyword_boost_id"], "keywords");
                    assert_eq!(setup["retry_for_s"], 2.5);
                    assert_eq!(setup["close_ws_on_eos"], false);
                    assert_eq!(setup["json_config"], r#"{"temperature":0.2}"#);
                    assert_eq!(
                        setup.get("client_req_id"),
                        correlated.then(|| json!("request")).as_ref()
                    );
                    ws.send(Message::Text(
                        json!({
                            "type":"ready", "model_name":"default", "sample_rate":24000,
                            "frame_size":480, "delay_in_frames":0.0,
                            "request_id":"server-request", "text_stream_names":["transcript"]
                        })
                        .to_string()
                        .into(),
                    ))
                    .await?;
                    for expected in [
                        json!({"type":"audio","audio":"AQI="}),
                        json!({"type":"audio","audio":"AwQ="}),
                        json!({"type":"flush","flush_id":42}),
                        json!({"type":"audio","audio":"BQY="}),
                        json!({"type":"end_of_stream"}),
                    ] {
                        let mut expected = expected;
                        if correlated {
                            expected["client_req_id"] = json!("request");
                        }
                        let message: serde_json::Value =
                            serde_json::from_str(&ws.next().await.unwrap()?.into_text()?)?;
                        assert_eq!(message, expected);
                    }
                    for message in [
                        json!({"type":"flushed","flush_id":42}),
                        json!({"type":"text","text":"hello","start_s":0.0,"stream_id":0}),
                        json!({"type":"end_of_stream"}),
                    ] {
                        ws.send(Message::Text(message.to_string().into())).await?;
                    }
                    Ok::<(), anyhow::Error>(())
                });
                let client =
                    Client::new("test-key").with_base_url(&format!("http://{address}/api"))?;
                let mut setup = stt::Setup::default()
                    .with_keyword_boost_id("keywords")
                    .with_retry_for_s(2.5)
                    .with_close_ws_on_eos(false)
                    .with_json_config(&json!({"temperature":0.2}));
                if correlated {
                    setup = setup.with_client_req_id("request");
                }
                let mut stream = client.stt_stream(setup).await?;
                if split {
                    let (mut sender, mut receiver) = stream.split();
                    sender.send_audio(vec![1, 2]).await?;
                    sender.send_audio_base64("AwQ=".into()).await?;
                    sender.send_flush(42).await?;
                    sender.send_audio(vec![5, 6]).await?;
                    sender.send_eos().await?;
                    assert!(matches!(
                        receiver.next_message().await?,
                        Some(stt::Response::Flushed { flush_id: 42, .. })
                    ));
                    assert!(matches!(receiver.next_message().await?, Some(stt::Response::Text(_))));
                    assert!(receiver.next_message().await?.is_none());
                } else {
                    stream.send_audio(vec![1, 2]).await?;
                    stream.send_audio_base64("AwQ=".into()).await?;
                    stream.send_flush(42).await?;
                    stream.send_audio(vec![5, 6]).await?;
                    stream.send_eos().await?;
                    assert!(matches!(
                        stream.next_message().await?,
                        Some(stt::Response::Flushed { flush_id: 42, .. })
                    ));
                    assert!(matches!(stream.next_message().await?, Some(stt::Response::Text(_))));
                    assert!(stream.next_message().await?.is_none());
                }
                server.await??;
            }
        }
        Ok::<(), anyhow::Error>(())
    })
    .await??;
    Ok(())
}

#[test]
fn default_setup_preserves_server_defaults() {
    let setup = serde_json::to_value(stt::Setup::default()).unwrap();
    for field in ["keyword_boost_id", "client_req_id", "retry_for_s", "close_ws_on_eos"] {
        assert!(setup.get(field).is_none());
    }
    let setup: stt::Setup =
        serde_json::from_value(json!({"model_name":"default", "input_format":"pcm"})).unwrap();
    assert!(setup.close_ws_on_eos.is_none());
    assert!(setup.client_req_id.is_none());
}

#[test]
fn request_types_round_trip_with_optional_correlation() {
    for request in [
        json!({"type":"audio", "audio":"AQI="}),
        json!({"type":"flush", "flush_id":42}),
        json!({"type":"end_of_stream"}),
    ] {
        for client_req_id in [None, Some("request")] {
            let mut expected = request.clone();
            if let Some(client_req_id) = client_req_id {
                expected["client_req_id"] = json!(client_req_id);
            }
            let parsed: stt::Request = serde_json::from_value(expected.clone()).unwrap();
            let actual_client_req_id = match &parsed {
                stt::Request::Audio(audio) => audio.client_req_id.as_deref(),
                stt::Request::Flush { client_req_id, .. }
                | stt::Request::EndOfStream { client_req_id } => client_req_id.as_deref(),
                stt::Request::Setup(_) => panic!("unexpected setup"),
            };
            assert_eq!(actual_client_req_id, client_req_id);
            assert_eq!(serde_json::to_value(parsed).unwrap(), expected);
        }
    }
}
