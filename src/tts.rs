//! Text-to-speech functionality and streaming support.
//!
//! This module provides both a simple one-shot TTS function and a streaming interface
//! for more advanced use cases.

use crate::client::{Client, WebSocket};
use crate::protocol::tts as p;
use anyhow::Result;

/// A streaming text-to-speech session.
///
/// `TtsStream` provides fine-grained control over the TTS process, allowing you to:
/// - Send text incrementally
/// - Receive audio chunks as they are generated
/// - Access timing information for each audio segment
///
/// # Example
///
/// ```no_run
/// use gradium::{Client, TtsStream, protocol::tts::Setup};
///
/// # async fn example() -> anyhow::Result<()> {
/// let client = Client::new("your-api-key");
/// let setup = Setup::new("m86j6D7UZpGzHsNu");
///
/// let mut stream = TtsStream::new(setup, &client).await?;
/// stream.send_text("Hello, world!").await?;
/// stream.send_eos().await?;
///
/// while let Some(msg) = stream.next_message().await? {
///     // Process audio/text messages
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct TtsStream {
    ws: WebSocket,
    ready: p::Ready,
}

#[derive(Debug)]
pub struct TtsStreamSender {
    ws: crate::client::WebSocketSender,
}

#[derive(Debug)]
pub struct TtsStreamReceiver {
    ws: crate::client::WebSocketReceiver,
}

impl TtsStream {
    pub fn split(self) -> (TtsStreamSender, TtsStreamReceiver) {
        use futures_util::StreamExt;
        let (ws_tx, ws_rx) = self.ws.split();
        (TtsStreamSender { ws: ws_tx }, TtsStreamReceiver { ws: ws_rx })
    }

    /// Creates a new TTS streaming session.
    ///
    /// This establishes a WebSocket connection to the Gradium API and initializes
    /// the TTS session with the provided configuration.
    ///
    /// # Arguments
    ///
    /// * `setup` - The TTS configuration (model, voice, output format)
    /// * `client` - The Gradium API client
    ///
    /// # Returns
    ///
    /// A new `TtsStream` ready to receive text input
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The WebSocket connection fails
    /// - The server returns an error during setup
    /// - The server sends an unexpected response
    pub async fn new(setup: p::Setup, client: &Client) -> Result<Self> {
        use futures_util::SinkExt;

        let mut ws = client.ws_connect("speech/tts").await?;
        let setup = serde_json::to_string(&p::Request::Setup(setup))?;
        ws.send(tokio_tungstenite::tungstenite::Message::Text(setup.into())).await?;
        let first_msg = crate::client::next_message(&mut ws).await?;
        let first_msg = match first_msg {
            None => anyhow::bail!("connection closed by server"),
            Some(m) => m,
        };
        let first_msg: p::Response = serde_json::from_str(&first_msg)?;
        let ready = match first_msg {
            p::Response::Ready(ready) => ready,
            p::Response::Error { code, message, .. } => {
                anyhow::bail!("error from server {code:?}: {message}")
            }
            _ => anyhow::bail!("unexpected first message from server: {:?}", first_msg),
        };
        Ok(Self { ws, ready })
    }

    /// Signals the end of the text input stream.
    ///
    /// After calling this, the server will finish processing any remaining text
    /// and send the final audio chunks before closing the stream.
    ///
    /// # Errors
    ///
    /// Returns an error if the message cannot be sent
    pub async fn send_eos(&mut self) -> Result<()> {
        use futures_util::SinkExt;

        let req = p::Request::EndOfStream { client_req_id: None };
        let req = serde_json::to_string(&req)?;
        self.ws.send(tokio_tungstenite::tungstenite::Message::Text(req.into())).await?;
        Ok(())
    }

    /// Sends text to be synthesized into speech.
    ///
    /// You can call this method multiple times to stream text to the TTS engine.
    ///
    /// # Arguments
    ///
    /// * `text` - The text to synthesize
    ///
    /// # Errors
    ///
    /// Returns an error if the message cannot be sent
    pub async fn send_text(&mut self, text: &str) -> Result<()> {
        use futures_util::SinkExt;

        let req = p::Request::Text(p::Text { text: text.to_string(), client_req_id: None });
        let req = serde_json::to_string(&req)?;
        self.ws.send(tokio_tungstenite::tungstenite::Message::Text(req.into())).await?;
        Ok(())
    }

    /// Receives the next message from the server.
    ///
    /// This can be an audio chunk, text with timing information, or end-of-stream signal.
    ///
    /// # Returns
    ///
    /// * `Ok(Some(response))` - A message was received
    /// * `Ok(None)` - The stream has ended
    /// * `Err(_)` - An error occurred
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The WebSocket encounters an error
    /// - The server returns an error response
    /// - Message deserialization fails
    pub async fn next_message(&mut self) -> Result<Option<p::Response>> {
        let msg = crate::client::next_message(&mut self.ws).await?;
        let msg = match msg {
            None => return Ok(None),
            Some(m) => m,
        };
        let msg: p::Response = serde_json::from_str(&msg)?;

        match &msg {
            p::Response::EndOfStream { .. } => return Ok(None),
            p::Response::Error { code, message, .. } => {
                anyhow::bail!("error from server {code:?}: {message}")
            }
            _ => {}
        }
        Ok(Some(msg))
    }

    /// Returns the audio sample rate in Hz.
    pub fn sample_rate(&self) -> u32 {
        self.ready.sample_rate
    }

    /// Returns the audio frame size in samples.
    pub fn frame_size(&self) -> u32 {
        self.ready.frame_size
    }

    /// Returns the names of available audio streams.
    pub fn audio_stream_names(&self) -> &[String] {
        &self.ready.audio_stream_names
    }

    /// Returns the names of available text streams.
    pub fn text_stream_names(&self) -> &[String] {
        &self.ready.text_stream_names
    }

    /// Returns the request ID for this session.
    pub fn request_id(&self) -> &str {
        &self.ready.request_id
    }
}

/// Result of a one-shot TTS request.
///
/// Contains the complete audio data and timing information for the synthesized text.
#[derive(Debug, Clone)]
pub struct TtsResult {
    raw_data: Vec<u8>,
    request_id: String,
    sample_rate: u32,
    text_with_timestamps: Vec<crate::TextWithTimestamps>,
}

impl TtsResult {
    /// Returns the raw audio data as bytes.
    ///
    /// The format of the data depends on the `AudioFormat` specified in the setup.
    pub fn raw_data(&self) -> &[u8] {
        &self.raw_data
    }

    /// Returns the unique request ID for this TTS operation.
    pub fn request_id(&self) -> &str {
        &self.request_id
    }

    /// Returns the audio sample rate in Hz.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Returns text segments with their timing information.
    pub fn text_with_timestamps(&self) -> &[crate::TextWithTimestamps] {
        &self.text_with_timestamps
    }
}

impl TtsStreamSender {
    /// Signals the end of the text input stream.
    ///
    /// After calling this, the server will finish processing any remaining text
    /// and send the final audio chunks before closing the stream.
    ///
    /// # Errors
    ///
    /// Returns an error if the message cannot be sent
    pub async fn send_eos(&mut self) -> Result<()> {
        use futures_util::SinkExt;

        let req = p::Request::EndOfStream { client_req_id: None };
        let req = serde_json::to_string(&req)?;
        self.ws.send(tokio_tungstenite::tungstenite::Message::Text(req.into())).await?;
        Ok(())
    }

    /// Sends text to be synthesized into speech.
    ///
    /// You can call this method multiple times to stream text to the TTS engine.
    ///
    /// # Arguments
    ///
    /// * `text` - The text to synthesize
    ///
    /// # Errors
    ///
    /// Returns an error if the message cannot be sent
    pub async fn send_text(&mut self, text: &str) -> Result<()> {
        use futures_util::SinkExt;

        let req = p::Request::Text(p::Text { text: text.to_string(), client_req_id: None });
        let req = serde_json::to_string(&req)?;
        self.ws.send(tokio_tungstenite::tungstenite::Message::Text(req.into())).await?;
        Ok(())
    }
}

impl TtsStreamReceiver {
    /// Receives the next message from the server.
    ///
    /// This can be an audio chunk, text with timing information, or end-of-stream signal.
    ///
    /// # Returns
    ///
    /// * `Ok(Some(response))` - A message was received
    /// * `Ok(None)` - The stream has ended
    /// * `Err(_)` - An error occurred
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The WebSocket encounters an error
    /// - The server returns an error response
    /// - Message deserialization fails
    pub async fn next_message(&mut self) -> Result<Option<p::Response>> {
        let msg = crate::client::next_message_receiver(&mut self.ws).await?;
        let msg = match msg {
            None => return Ok(None),
            Some(m) => m,
        };
        let msg: p::Response = serde_json::from_str(&msg)?;

        match &msg {
            p::Response::EndOfStream { .. } => return Ok(None),
            p::Response::Error { code, message, .. } => {
                anyhow::bail!("error from server {code:?}: {message}")
            }
            _ => {}
        }
        Ok(Some(msg))
    }
}

/// Performs a one-shot text-to-speech conversion.
///
/// This is a convenience function that handles the entire TTS workflow:
/// 1. Creates a TTS stream
/// 2. Sends the text
/// 3. Signals end-of-stream
/// 4. Collects all audio and timing data
/// 5. Returns the complete result
///
/// For more control over the TTS process, use [`TtsStream`] directly.
///
/// # Arguments
///
/// * `text` - The text to synthesize
/// * `setup` - The TTS configuration (model, voice, output format)
/// * `client` - The Gradium API client
///
/// # Returns
///
/// A `TtsResult` containing the complete audio data and timing information
///
/// # Errors
///
/// Returns an error if any step of the TTS process fails
///
/// # Example
///
/// ```no_run
/// use gradium::{Client, tts, protocol::tts::Setup, protocol::AudioFormat};
///
/// # async fn example() -> anyhow::Result<()> {
/// let client = Client::new("your-api-key");
/// let setup = Setup::new("m86j6D7UZpGzHsNu").with_output_format(AudioFormat::Wav);
/// let result = tts("Hello, world!", setup, &client).await?;
/// println!("Generated {} bytes of audio", result.raw_data().len());
/// # Ok(())
/// # }
/// ```
pub async fn tts(text: &str, setup: p::Setup, client: &Client) -> Result<TtsResult> {
    let mut stream = TtsStream::new(setup, client).await?;
    stream.send_text(text).await?;
    stream.send_eos().await?;
    let mut all_raw = vec![];
    let mut text_with_timestamps = vec![];
    while let Some(data) = stream.next_message().await? {
        match data {
            p::Response::Audio(audio) => {
                let raw = audio.raw_audio()?;
                all_raw.push(raw)
            }
            p::Response::Text(text) => {
                text_with_timestamps.push(crate::TextWithTimestamps {
                    text: text.text.clone(),
                    start_s: text.start_s,
                    stop_s: text.stop_s,
                });
            }
            _ => {}
        }
    }
    let raw_data = all_raw.concat();
    Ok(TtsResult {
        raw_data,
        text_with_timestamps,
        request_id: stream.request_id().to_string(),
        sample_rate: stream.sample_rate(),
    })
}

/// A multiplexing TTS session over a single WebSocket connection.
///
/// Unlike `TtsStream`, this type allows sending multiple independent TTS requests
/// (each with its own `client_req_id`) over a single WebSocket. The caller is
/// responsible for routing responses by inspecting `client_req_id` on each message.
///
/// Key differences from `TtsStream`:
/// - `next_message()` returns `EndOfStream` and `Error` as values (not `None`/`Err`),
///   so the caller can read `client_req_id` to know which request finished.
/// - Returns `Ok(None)` only when the WebSocket itself closes.
/// - No `Ready` is stored — each `send_setup()` gets its own `Ready` response.
#[derive(Debug)]
pub struct TtsMultiplexStream {
    ws: WebSocket,
}

/// Sender half of a split `TtsMultiplexStream`.
#[derive(Debug)]
pub struct TtsMultiplexSender {
    ws: crate::client::WebSocketSender,
}

/// Receiver half of a split `TtsMultiplexStream`.
#[derive(Debug)]
pub struct TtsMultiplexReceiver {
    ws: crate::client::WebSocketReceiver,
}

impl TtsMultiplexStream {
    /// Opens a multiplexing WebSocket connection without sending Setup.
    pub async fn connect(client: &Client) -> Result<Self> {
        let ws = client.ws_connect("speech/tts").await?;
        Ok(Self { ws })
    }

    /// Sends a Setup message for a new multiplexed request.
    ///
    /// The setup must have `client_req_id` set (to route responses) and
    /// `close_ws_on_eos` set to `false` (to keep the connection open for
    /// subsequent requests). Returns an error if either is misconfigured.
    pub async fn send_setup(&mut self, setup: p::Setup) -> Result<()> {
        use futures_util::SinkExt;

        if setup.client_req_id.is_none() {
            anyhow::bail!("client_req_id must be set for multiplexed requests");
        }
        if setup.close_ws_on_eos != Some(false) {
            anyhow::bail!("close_ws_on_eos must be set to false for multiplexed requests");
        }
        let req = serde_json::to_string(&p::Request::Setup(setup))?;
        self.ws.send(tokio_tungstenite::tungstenite::Message::Text(req.into())).await?;
        Ok(())
    }

    /// Sends text to be synthesized, tagged with a `client_req_id`.
    pub async fn send_text(&mut self, text: &str, client_req_id: &str) -> Result<()> {
        use futures_util::SinkExt;

        let req = p::Request::Text(p::Text {
            text: text.to_string(),
            client_req_id: Some(client_req_id.to_string()),
        });
        let req = serde_json::to_string(&req)?;
        self.ws.send(tokio_tungstenite::tungstenite::Message::Text(req.into())).await?;
        Ok(())
    }

    /// Signals end-of-stream for a specific request.
    pub async fn send_eos(&mut self, client_req_id: &str) -> Result<()> {
        use futures_util::SinkExt;

        let req = p::Request::EndOfStream { client_req_id: Some(client_req_id.to_string()) };
        let req = serde_json::to_string(&req)?;
        self.ws.send(tokio_tungstenite::tungstenite::Message::Text(req.into())).await?;
        Ok(())
    }

    /// Receives the next message from the server.
    ///
    /// Unlike `TtsStream::next_message()`, this returns `EndOfStream` and `Error`
    /// as `Ok(Some(response))` values so the caller can inspect `client_req_id`.
    /// Returns `Ok(None)` only when the WebSocket connection itself closes, which
    /// is not expected during normal operation — typically only on the server's
    /// 5-minute inactivity timeout.
    pub async fn next_message(&mut self) -> Result<Option<p::Response>> {
        let msg = crate::client::next_message(&mut self.ws).await?;
        let msg = match msg {
            None => return Ok(None),
            Some(m) => m,
        };
        let msg: p::Response = serde_json::from_str(&msg)?;
        Ok(Some(msg))
    }

    /// Splits into sender and receiver halves for concurrent use with `tokio::spawn`.
    pub fn split(self) -> (TtsMultiplexSender, TtsMultiplexReceiver) {
        use futures_util::StreamExt;
        let (ws_tx, ws_rx) = self.ws.split();
        (TtsMultiplexSender { ws: ws_tx }, TtsMultiplexReceiver { ws: ws_rx })
    }
}

impl TtsMultiplexSender {
    /// Sends a Setup message for a new multiplexed request.
    ///
    /// The setup must have `client_req_id` set (to route responses) and
    /// `close_ws_on_eos` set to `false` (to keep the connection open for
    /// subsequent requests). Returns an error if either is misconfigured.
    pub async fn send_setup(&mut self, setup: p::Setup) -> Result<()> {
        use futures_util::SinkExt;

        if setup.client_req_id.is_none() {
            anyhow::bail!("client_req_id must be set for multiplexed requests");
        }
        if setup.close_ws_on_eos != Some(false) {
            anyhow::bail!("close_ws_on_eos must be set to false for multiplexed requests");
        }
        let req = serde_json::to_string(&p::Request::Setup(setup))?;
        self.ws.send(tokio_tungstenite::tungstenite::Message::Text(req.into())).await?;
        Ok(())
    }

    /// Sends text to be synthesized, tagged with a `client_req_id`.
    pub async fn send_text(&mut self, text: &str, client_req_id: &str) -> Result<()> {
        use futures_util::SinkExt;

        let req = p::Request::Text(p::Text {
            text: text.to_string(),
            client_req_id: Some(client_req_id.to_string()),
        });
        let req = serde_json::to_string(&req)?;
        self.ws.send(tokio_tungstenite::tungstenite::Message::Text(req.into())).await?;
        Ok(())
    }

    /// Signals end-of-stream for a specific request.
    pub async fn send_eos(&mut self, client_req_id: &str) -> Result<()> {
        use futures_util::SinkExt;

        let req = p::Request::EndOfStream { client_req_id: Some(client_req_id.to_string()) };
        let req = serde_json::to_string(&req)?;
        self.ws.send(tokio_tungstenite::tungstenite::Message::Text(req.into())).await?;
        Ok(())
    }
}

impl TtsMultiplexReceiver {
    /// Receives the next message from the server.
    ///
    /// Returns `EndOfStream` and `Error` as values (not `None`/`Err`).
    /// Returns `Ok(None)` only when the WebSocket connection itself closes, which
    /// is not expected during normal operation — typically only on the server's
    /// 5-minute inactivity timeout.
    pub async fn next_message(&mut self) -> Result<Option<p::Response>> {
        let msg = crate::client::next_message_receiver(&mut self.ws).await?;
        let msg = match msg {
            None => return Ok(None),
            Some(m) => m,
        };
        let msg: p::Response = serde_json::from_str(&msg)?;
        Ok(Some(msg))
    }
}

/// Creates a new TTS stream for real-time text-to-speech.
///
/// Use this function when you need more control over the TTS process,
/// such as streaming text incrementally or processing audio chunks as they arrive.
/// For simple one-shot TTS, consider using the `tts()` function instead.
///
/// # Arguments
///
/// * `setup` - TTS configuration (model, voice, output format)
/// * `client` - The Gradium client to use
///
/// # Returns
///
/// A `TtsStream` that can be used to send text and receive audio chunks
///
/// # Errors
///
/// Returns an error if the WebSocket connection fails or the server responds with an error
///
/// # Example
///
/// ```no_run
/// use gradium::{Client, protocol::tts::Setup};
/// use gradium::tts::tts_stream;
///
/// # async fn example() -> anyhow::Result<()> {
/// let client = Client::new("api_key");
/// let setup = Setup::new("m86j6D7UZpGzHsNu");
/// let mut stream = tts_stream(setup, &client).await?;
///
/// // Send text incrementally
/// stream.send_text("Hello, ").await?;
/// stream.send_text("world!").await?;
/// stream.send_eos().await?;
///
/// // Process audio chunks as they arrive
/// while let Some(response) = stream.next_message().await? {
///     println!("Got response: {:?}", response);
/// }
/// # Ok(())
/// # }
/// ```
pub async fn tts_stream(setup: p::Setup, client: &Client) -> Result<TtsStream> {
    TtsStream::new(setup, client).await
}
