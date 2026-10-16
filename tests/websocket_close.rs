use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use gradium::{
    Client,
    protocol::{stt, tts},
};
use serde_json::json;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{
    WebSocketStream,
    tungstenite::{Message, protocol::frame::coding::CloseCode},
};

async fn serve_result(listener: TcpListener) -> anyhow::Result<WebSocketStream<TcpStream>> {
    let (socket, _) = listener.accept().await?;
    let mut ws = tokio_tungstenite::accept_async(socket).await?;
    let setup = ws.next().await.unwrap()?.into_text()?;
    assert_eq!(serde_json::from_str::<serde_json::Value>(&setup)?["type"], "setup");
    ws.send(Message::Text(
        json!({
            "type":"ready", "model_name":"default", "sample_rate":24000,
            "frame_size":480, "audio_stream_names":["audio"], "text_stream_names":["text"],
            "request_id":"close-test"
        })
        .to_string()
        .into(),
    ))
    .await?;
    for _ in 0..2 {
        ws.next().await.unwrap()?;
    }
    ws.send(Message::Text(
        json!({
            "type":"audio", "audio":"AQIDBA==", "start_s":0.0, "stop_s":1.0, "stream_id":0
        })
        .to_string()
        .into(),
    ))
    .await?;
    ws.send(Message::Text(json!({"type":"end_of_stream"}).to_string().into())).await?;
    Ok(ws)
}

#[tokio::test]
async fn tts_completes_the_websocket_close_handshake() -> anyhow::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server = tokio::spawn(async move {
        let mut ws = serve_result(listener).await?;
        let close = tokio::time::timeout(Duration::from_secs(2), ws.next()).await?.unwrap()?;
        assert!(matches!(close, Message::Close(Some(frame)) if frame.code == CloseCode::Normal));
        ws.flush().await?;
        Ok::<(), anyhow::Error>(())
    });
    let client = Client::new("test").with_base_url(&format!("http://{address}/api"))?;
    let result = client.tts("Hello", tts::Setup::new("voice")).await?;
    assert_eq!(result.raw_data(), [1, 2, 3, 4]);
    assert_eq!(result.request_id(), "close-test");
    server.await??;
    Ok(())
}

#[tokio::test]
async fn stt_completes_the_close_handshake_and_preserves_the_transcript() -> anyhow::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server = tokio::spawn(async move {
        let (socket, _) = listener.accept().await?;
        let mut ws = tokio_tungstenite::accept_async(socket).await?;
        ws.next().await.unwrap()?;
        ws.send(Message::Text(
            json!({
                "type":"ready", "model_name":"default", "sample_rate":24000,
                "frame_size":480, "delay_in_frames":0.0, "request_id":"close-test",
                "text_stream_names":["transcript"]
            })
            .to_string()
            .into(),
        ))
        .await?;
        for _ in 0..2 {
            ws.next().await.unwrap()?;
        }
        for response in [
            json!({"type":"text", "text":"Hello", "start_s":0.0, "stream_id":0}),
            json!({"type":"end_text", "stop_s":1.0, "stream_id":0}),
            json!({"type":"end_of_stream"}),
        ] {
            ws.send(Message::Text(response.to_string().into())).await?;
        }
        let close = tokio::time::timeout(Duration::from_secs(2), ws.next()).await?.unwrap()?;
        assert!(matches!(close, Message::Close(Some(frame)) if frame.code == CloseCode::Normal));
        ws.flush().await?;
        Ok::<(), anyhow::Error>(())
    });
    let client = Client::new("test").with_base_url(&format!("http://{address}/api"))?;
    let result = client.stt(vec![1, 2], stt::Setup::default()).await?;
    assert_eq!(result.request_id(), "close-test");
    assert_eq!(result.text_with_timestamps().len(), 1);
    assert_eq!(result.text_with_timestamps()[0].text, "Hello");
    assert_eq!(result.text_with_timestamps()[0].stop_s, 1.0);
    server.await??;
    Ok(())
}

#[tokio::test]
async fn tts_acknowledges_a_server_initiated_close() -> anyhow::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server = tokio::spawn(async move {
        let mut ws = serve_result(listener).await?;
        ws.close(None).await?;
        let close = tokio::time::timeout(Duration::from_secs(2), ws.next()).await?.unwrap()?;
        assert!(matches!(close, Message::Close(_)));
        Ok::<(), anyhow::Error>(())
    });
    let client = Client::new("test").with_base_url(&format!("http://{address}/api"))?;
    let result = client.tts("Hello", tts::Setup::new("voice")).await?;
    assert_eq!(result.raw_data(), [1, 2, 3, 4]);
    server.await??;
    Ok(())
}

#[tokio::test]
async fn tts_preserves_completed_audio_when_the_peer_drops_after_eos() -> anyhow::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server = tokio::spawn(async move {
        drop(serve_result(listener).await?);
        Ok::<(), anyhow::Error>(())
    });
    let client = Client::new("test").with_base_url(&format!("http://{address}/api"))?;
    let result = client.tts("Hello", tts::Setup::new("voice")).await?;
    assert_eq!(result.raw_data(), [1, 2, 3, 4]);
    server.await??;
    Ok(())
}

#[tokio::test]
async fn tts_bounds_the_wait_for_a_peer_close_response() -> anyhow::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let (close_seen, received_close) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let mut ws = serve_result(listener).await?;
        let close = ws.next().await.unwrap()?;
        assert!(matches!(close, Message::Close(_)));
        close_seen.send(()).unwrap();
        std::future::pending::<()>().await;
        Ok::<(), anyhow::Error>(())
    });
    let client = Client::new("test").with_base_url(&format!("http://{address}/api"))?;
    let result =
        tokio::time::timeout(Duration::from_secs(7), client.tts("Hello", tts::Setup::new("voice")))
            .await??;
    assert_eq!(result.raw_data(), [1, 2, 3, 4]);
    received_close.await?;
    server.abort();
    Ok(())
}
