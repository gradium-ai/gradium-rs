#![doc = include_str!("../README.md")]
pub mod client;
pub mod protocol;
pub mod stt;
pub mod tts;

pub use client::Client;
pub use stt::{SttResult, SttStream, stt, stt_stream};
pub use tts::{TtsMultiplexStream, TtsResult, TtsStream, tts, tts_stream};

/// Represents text with associated timing information.
///
/// This structure is returned as part of TTS and STT results to provide timing data
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
