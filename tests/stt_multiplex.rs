use futures_util::{SinkExt, StreamExt};
use gradium::{Client, protocol::stt};
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::Message;

fn responses() -> Vec<Value> {
    vec![
        json!({"type":"ready", "model_name":"default", "sample_rate":24000,
            "frame_size":480, "delay_in_frames":0.0, "text_stream_names":["text"],
            "request_id":"server-b", "client_req_id":"b"}),
        json!({"type":"error", "code":1008, "message":"request a failed", "client_req_id":"a"}),
        json!({"type":"end_of_stream", "client_req_id":"a"}),
        json!({"type":"text", "text":"hello", "start_s":0.0, "stream_id":0, "client_req_id":"b"}),
        json!({"type":"step", "step_idx":1, "step_duration_s":0.08, "total_duration_s":0.08,
            "vad":[{"horizon_s":1.0,"inactivity_prob":0.1}], "client_req_id":"b"}),
        json!({"type":"flushed", "flush_id":42, "client_req_id":"b"}),
        json!({"type":"end_text", "stop_s":1.0, "stream_id":0, "client_req_id":"b"}),
        json!({"type":"end_of_stream", "client_req_id":"b"}),
    ]
}

fn setup(id: &str) -> stt::Setup {
    stt::Setup::default().with_client_req_id(id).with_close_ws_on_eos(false)
}

#[tokio::test]
async fn multiplex_routes_interleaved_responses_and_preserves_request_failures()
-> anyhow::Result<()> {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        for split in [false, true] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
            let address = listener.local_addr()?;
            let server = tokio::spawn(async move {
                let (socket, _) = listener.accept().await?;
                let mut ws = tokio_tungstenite::accept_async(socket).await?;
                for expected in [
                    json!({"type":"setup", "model_name":"default", "input_format":"pcm", "client_req_id":"a", "close_ws_on_eos":false}),
                    json!({"type":"setup", "model_name":"default", "input_format":"pcm", "client_req_id":"b", "close_ws_on_eos":false}),
                    json!({"type":"audio", "audio":"AQI=", "client_req_id":"a"}),
                    json!({"type":"audio", "audio":"AwQ=", "client_req_id":"b"}),
                    json!({"type":"flush", "flush_id":42, "client_req_id":"b"}),
                    json!({"type":"end_of_stream", "client_req_id":"a"}),
                    json!({"type":"audio", "audio":"BQY=", "client_req_id":"b"}),
                    json!({"type":"end_of_stream", "client_req_id":"b"}),
                ] {
                    let actual: Value = serde_json::from_str(&ws.next().await.unwrap()?.into_text()?)?;
                    assert_eq!(actual, expected);
                }
                for response in responses() {
                    ws.send(Message::Text(response.to_string().into())).await?;
                }
                ws.close(None).await?;
                Ok::<(), anyhow::Error>(())
            });
            let client = Client::new("test-key").with_base_url(&format!("http://{address}/api"))?;
            let mut stream = client.stt_multiplex().await?;
            if split {
                let (mut sender, mut receiver) = stream.split();
                assert!(sender.send_setup(stt::Setup::default()).await.is_err());
                assert!(sender.send_setup(stt::Setup::default().with_client_req_id("invalid")).await.is_err());
                sender.send_setup(setup("a")).await?;
                sender.send_setup(setup("b")).await?;
                sender.send_audio(vec![1, 2], "a").await?;
                sender.send_audio_base64("AwQ=".into(), "b").await?;
                sender.send_flush(42, "b").await?;
                sender.send_eos("a").await?;
                sender.send_audio(vec![5, 6], "b").await?;
                sender.send_eos("b").await?;
                for expected in responses() {
                    assert_eq!(serde_json::to_value(receiver.next_message().await?.unwrap())?, expected);
                }
                assert!(receiver.next_message().await?.is_none());
            } else {
                assert!(stream.send_setup(stt::Setup::default()).await.is_err());
                assert!(stream.send_setup(stt::Setup::default().with_client_req_id("invalid")).await.is_err());
                stream.send_setup(setup("a")).await?;
                stream.send_setup(setup("b")).await?;
                stream.send_audio(vec![1, 2], "a").await?;
                stream.send_audio_base64("AwQ=".into(), "b").await?;
                stream.send_flush(42, "b").await?;
                stream.send_eos("a").await?;
                stream.send_audio(vec![5, 6], "b").await?;
                stream.send_eos("b").await?;
                for expected in responses() {
                    assert_eq!(serde_json::to_value(stream.next_message().await?.unwrap())?, expected);
                }
                assert!(stream.next_message().await?.is_none());
            }
            server.await??;
        }
        Ok::<(), anyhow::Error>(())
    }).await??;
    Ok(())
}
