//! # Gradium Rust Client
//!
//! A Rust client library for the Gradium API, providing text-to-speech (TTS) capabilities
//! via WebSocket connections.
//!
//! ## Quick Start
//!
//! ```no_run
//! use gradium::{Client, protocol::tts::Setup, protocol::AudioFormat};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let api_key = gradium::api_key_from_env().expect("GRADIUM_API_KEY not set");
//!     let client = Client::new(&api_key);
//!
//!     let setup = Setup::new("m86j6D7UZpGzHsNu").with_output_format(AudioFormat::Wav);
//!
//!     let result = gradium::tts("Hello, world!", setup, &client).await?;
//!     let filename = "output.wav";
//!     std::fs::write(filename, result.raw_data())?;
//!     println!("Generated {} bytes of audio and saved to {}", result.raw_data().len(), filename);
//!     Ok(())
//! }
//! ```
//!
//! ## Features
//!
//! - **Simple API**: High-level `tts()` function for one-shot text-to-speech conversion
//! - **Streaming Support**: `TtsStream` for advanced use cases with real-time audio streaming
//! - **Multiple Formats**: Support for PCM, WAV, and Opus audio formats
//! - **Async/Await**: Built on tokio for efficient async I/O
//!
//! ## Environment Configuration
//!
//! The client expects the `GRADIUM_API_KEY` environment variable to be set with your API key.
//! You can retrieve it using the [`api_key_from_env()`] function.

pub mod client;
pub mod protocol;
pub mod stt;
pub mod tts;

pub use client::Client;
pub use stt::{SttResult, SttStream, stt, stt_stream};
pub use tts::{TtsResult, TtsStream, tts, tts_stream};

/// Represents text with associated timing information.
///
/// This structure is returned as part of TTS results to provide timing data
/// for the generated audio segments.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TextWithTimestamps {
    /// The text content.
    pub text: String,
    /// Start time in seconds.
    pub start_s: f64,
    /// Stop time in seconds.
    pub stop_s: f64,
}

/// Retrieves the Gradium API key from the `GRADIUM_API_KEY` environment variable.
///
/// # Returns
///
/// `Some(String)` if the environment variable is set, `None` otherwise.
///
/// # Example
///
/// ```no_run
/// let api_key = gradium::api_key_from_env().expect("GRADIUM_API_KEY not set");
/// ```
pub fn api_key_from_env() -> Option<String> {
    std::env::var("GRADIUM_API_KEY").ok()
}

/// Retrieves the Gradium base URL from the `GRADIUM_BASE_URL` environment variable.
///
/// # Returns
///
/// `Some(String)` if the environment variable is set, `None` otherwise.
///
/// # Example
///
/// ```no_run
/// let base_url = gradium::base_url_from_env();
/// ```
pub fn base_url_from_env() -> Option<String> {
    std::env::var("GRADIUM_BASE_URL").ok()
}
