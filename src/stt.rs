//! Speech-to-text (STT) client functionality.
//!
//! This module provides a high-level interface for streaming audio to the Gradium
//! STT service and receiving transcriptions in real-time.

use crate::client::{Client, WebSocket};
use crate::protocol::stt as p;
use anyhow::Result;

/// A streaming STT session for real-time audio transcription.
///
/// This struct maintains an active WebSocket connection to the STT service
/// and provides methods to send audio data and receive transcription results.
#[derive(Debug)]
pub struct SttStream {
    /// WebSocket connection to the STT service
    ws: WebSocket,
    /// Session metadata from the server's Ready response
    ready: p::Ready,
}

#[derive(Debug)]
pub struct SttStreamSender {
    ws: crate::client::WebSocketSender,
}

#[derive(Debug)]
pub struct SttStreamReceiver {
    ws: crate::client::WebSocketReceiver,
}

impl SttStream {
    pub fn split(self) -> (SttStreamSender, SttStreamReceiver) {
        use futures_util::StreamExt;
        let (ws_tx, ws_rx) = self.ws.split();
        (SttStreamSender { ws: ws_tx }, SttStreamReceiver { ws: ws_rx })
    }

    /// Creates a new STT streaming session.
    ///
    /// This establishes a WebSocket connection to the STT service, sends the
    /// setup configuration, and waits for the server's Ready response.
    ///
    /// # Arguments
    ///
    /// * `setup` - Configuration for the STT session (model name, audio format)
    /// * `client` - The Gradium client to use for the connection
    ///
    /// # Returns
    ///
    /// A new `SttStream` ready to accept audio data
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The WebSocket connection fails
    /// - The server responds with an error
    /// - The server sends an unexpected response
    pub async fn new(setup: p::Setup, client: &Client) -> Result<Self> {
        use futures_util::SinkExt;

        // Connect to the STT WebSocket endpoint
        let mut ws = client.ws_connect("speech/asr").await?;

        // Send the setup configuration
        let setup = serde_json::to_string(&p::Request::Setup(setup))?;
        ws.send(tokio_tungstenite::tungstenite::Message::Text(setup.into())).await?;

        // Wait for the Ready response from the server
        let first_msg = crate::client::next_message(&mut ws).await?;
        let first_msg = match first_msg {
            None => anyhow::bail!("connection closed by server"),
            Some(m) => m,
        };
        let first_msg: p::Response = serde_json::from_str(&first_msg)?;
        let ready = match first_msg {
            p::Response::Ready(ready) => ready,
            p::Response::Error { code, message } => {
                anyhow::bail!("error from server {code}: {message}")
            }
            _ => anyhow::bail!("unexpected first message from server: {:?}", first_msg),
        };
        Ok(Self { ws, ready })
    }

    /// Signals the end of the audio stream.
    ///
    /// This tells the server that no more audio will be sent, allowing it to
    /// finalize the transcription and send any remaining results.
    ///
    /// # Errors
    ///
    /// Returns an error if the message cannot be sent
    pub async fn send_eos(&mut self) -> Result<()> {
        use futures_util::SinkExt;

        let req = p::Request::EndOfStream;
        let req = serde_json::to_string(&req)?;
        self.ws.send(tokio_tungstenite::tungstenite::Message::Text(req.into())).await?;
        Ok(())
    }

    /// Sends audio data to the server for transcription.
    ///
    /// Audio should be sent in chunks matching the format and sample rate
    /// specified in the Setup configuration.
    ///
    /// # Arguments
    ///
    /// * `data` - Raw audio bytes to transcribe
    ///
    /// # Errors
    ///
    /// Returns an error if the message cannot be sent
    pub async fn send_audio(&mut self, audio: Vec<u8>) -> Result<()> {
        use base64::prelude::*;
        use futures_util::SinkExt;

        // Base 64 encode the audio data
        let audio = base64::engine::general_purpose::STANDARD.encode(&audio);
        let req = p::Request::Audio(p::Audio { audio });
        let req = serde_json::to_string(&req)?;
        self.ws.send(tokio_tungstenite::tungstenite::Message::Text(req.into())).await?;
        Ok(())
    }

    pub async fn send_audio_base64(&mut self, audio_b64: String) -> Result<()> {
        use futures_util::SinkExt;

        let req = p::Request::Audio(p::Audio { audio: audio_b64 });
        let req = serde_json::to_string(&req)?;
        self.ws.send(tokio_tungstenite::tungstenite::Message::Text(req.into())).await?;
        Ok(())
    }

    /// Receives the next message from the server.
    ///
    /// This method waits for and returns transcription results, VAD updates,
    /// or other server responses.
    ///
    /// # Returns
    ///
    /// - `Ok(Some(response))` - A response from the server
    /// - `Ok(None)` - The stream has ended (EndOfStream received or connection closed)
    /// - `Err(_)` - An error occurred or the server sent an error response
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The connection is closed unexpectedly
    /// - The server sends an error response
    /// - Message deserialization fails
    pub async fn next_message(&mut self) -> Result<Option<p::Response>> {
        let msg = crate::client::next_message(&mut self.ws).await?;
        let msg = match msg {
            None => return Ok(None),
            Some(m) => m,
        };
        let msg: p::Response = serde_json::from_str(&msg)?;

        match &msg {
            p::Response::EndOfStream => return Ok(None),
            p::Response::Error { code, message } => {
                anyhow::bail!("error from server {code}: {message}")
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
        self.ready.frame_size as u32
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

/// The complete result of a speech-to-text operation.
///
/// This struct contains all transcribed text segments with their timing information,
/// along with session metadata.
#[derive(Debug, Clone)]
pub struct SttResult {
    /// Unique identifier for this STT request
    request_id: String,
    /// Audio sample rate used for this transcription
    sample_rate: u32,
    /// Transcribed text segments with timing information
    text_with_timestamps: Vec<crate::TextWithTimestamps>,
}

impl SttResult {
    /// Returns the unique request ID for this transcription.
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

/// Transcribes audio data in a single batch operation.
///
/// This is a convenience function that creates an STT stream, sends all audio data,
/// waits for the complete transcription, and returns the result.
///
/// # Arguments
///
/// * `data` - Raw audio data to transcribe
/// * `setup` - STT configuration (model name, audio format)
/// * `client` - The Gradium client to use
///
/// # Returns
///
/// An `SttResult` containing all transcribed text segments with timing information
///
/// # Errors
///
/// Returns an error if the connection fails, the server returns an error,
/// or audio processing fails
///
/// # Example
///
/// ```no_run
/// use gradium::{Client, protocol::AudioFormat};
///
/// # async fn example() -> anyhow::Result<()> {
/// let client = Client::new("api_key");
/// let audio_data = vec![0u8; 48000]; // Example: 1 second of silence at 48kHz
/// let result = gradium::stt::stt(audio_data, Default::default(), &client).await?;
/// println!("Transcription: {:?}", result.text_with_timestamps());
/// # Ok(())
/// # }
/// ```
pub async fn stt(audio: Vec<u8>, setup: p::Setup, client: &Client) -> Result<SttResult> {
    let mut stream = SttStream::new(setup, client).await?;

    // Send audio in chunks (1920 bytes per chunk for typical 16kHz 16-bit audio)
    for audio in audio.chunks(1920) {
        stream.send_audio(audio.to_vec()).await?;
    }
    stream.send_eos().await?;

    // Collect transcription results
    let mut text_with_timestamps = vec![];
    while let Some(data) = stream.next_message().await? {
        match data {
            p::Response::Text(text) => {
                let twt = crate::TextWithTimestamps {
                    text: text.text,
                    start_s: text.start_s,
                    stop_s: text.start_s, // Initially set to start_s, updated by EndText
                };
                text_with_timestamps.push(twt)
            }
            p::Response::EndText(e) => {
                // Update the stop time for the last text segment
                text_with_timestamps.last_mut().iter_mut().for_each(|v| v.stop_s = e.stop_s)
            }
            _ => {}
        }
    }
    Ok(SttResult {
        text_with_timestamps,
        request_id: stream.request_id().to_string(),
        sample_rate: stream.sample_rate(),
    })
}

impl SttStreamSender {
    /// Signals the end of the audio stream.
    ///
    /// This tells the server that no more audio will be sent, allowing it to
    /// finalize the transcription and send any remaining results.
    ///
    /// # Errors
    ///
    /// Returns an error if the message cannot be sent
    pub async fn send_eos(&mut self) -> Result<()> {
        use futures_util::SinkExt;

        let req = p::Request::EndOfStream;
        let req = serde_json::to_string(&req)?;
        self.ws.send(tokio_tungstenite::tungstenite::Message::Text(req.into())).await?;
        Ok(())
    }

    /// Sends audio data to the server for transcription.
    ///
    /// Audio should be sent in chunks matching the format and sample rate
    /// specified in the Setup configuration.
    ///
    /// # Arguments
    ///
    /// * `data` - Raw audio bytes to transcribe
    ///
    /// # Errors
    ///
    /// Returns an error if the message cannot be sent
    pub async fn send_audio(&mut self, audio: Vec<u8>) -> Result<()> {
        use base64::prelude::*;
        use futures_util::SinkExt;

        // Base 64 encode the audio data
        let audio = base64::engine::general_purpose::STANDARD.encode(&audio);
        let req = p::Request::Audio(p::Audio { audio });
        let req = serde_json::to_string(&req)?;
        self.ws.send(tokio_tungstenite::tungstenite::Message::Text(req.into())).await?;
        Ok(())
    }

    pub async fn send_audio_base64(&mut self, audio_b64: String) -> Result<()> {
        use futures_util::SinkExt;

        let req = p::Request::Audio(p::Audio { audio: audio_b64 });
        let req = serde_json::to_string(&req)?;
        self.ws.send(tokio_tungstenite::tungstenite::Message::Text(req.into())).await?;
        Ok(())
    }
}

impl SttStreamReceiver {
    /// Receives the next message from the server.
    ///
    /// This method waits for and returns transcription results, VAD updates,
    /// or other server responses.
    ///
    /// # Returns
    ///
    /// - `Ok(Some(response))` - A response from the server
    /// - `Ok(None)` - The stream has ended (EndOfStream received or connection closed)
    /// - `Err(_)` - An error occurred or the server sent an error response
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The connection is closed unexpectedly
    /// - The server sends an error response
    /// - Message deserialization fails
    pub async fn next_message(&mut self) -> Result<Option<p::Response>> {
        let msg = crate::client::next_message_receiver(&mut self.ws).await?;
        let msg = match msg {
            None => return Ok(None),
            Some(m) => m,
        };
        let msg: p::Response = serde_json::from_str(&msg)?;

        match &msg {
            p::Response::EndOfStream => return Ok(None),
            p::Response::Error { code, message } => {
                anyhow::bail!("error from server {code}: {message}")
            }
            _ => {}
        }
        Ok(Some(msg))
    }
}

/// Creates a new STT stream for real-time audio transcription.
///
/// Use this function when you need more control over the transcription process,
/// such as streaming audio in real-time or processing results as they arrive.
/// For batch processing of complete audio files, consider using the `stt()` function instead.
///
/// # Arguments
///
/// * `setup` - STT configuration (model name, audio format)
/// * `client` - The Gradium client to use
///
/// # Returns
///
/// An `SttStream` that can be used to send audio chunks and receive transcription results
///
/// # Errors
///
/// Returns an error if the WebSocket connection fails or the server responds with an error
///
/// # Example
///
/// ```no_run
/// use gradium::{Client, protocol::AudioFormat};
/// use gradium::stt::stt_stream;
///
/// # async fn example() -> anyhow::Result<()> {
/// let client = Client::new("api_key");
/// let mut stream = stt_stream(Default::default(), &client).await?;
///
/// // Send audio chunks as they become available
/// let audio_chunk = vec![0u8; 1920];
/// stream.send_audio(audio_chunk).await?;
///
/// // Process results in real-time
/// while let Some(response) = stream.next_message().await? {
///     println!("Got response: {:?}", response);
/// }
/// # Ok(())
/// # }
/// ```
pub async fn stt_stream(setup: p::Setup, client: &Client) -> Result<SttStream> {
    SttStream::new(setup, client).await
}
