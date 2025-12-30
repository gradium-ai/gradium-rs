//! Protocol definitions for Gradium API communication.
//!
//! This module contains the types and structures used for communicating with the
//! Gradium API via WebSocket connections.

/// Audio output format for TTS generation.
#[derive(Debug, Clone)]
pub enum AudioFormat {
    /// Raw PCM audio data
    Pcm,
    /// WAV file format
    Wav,
    /// Opus compressed audio
    Opus,
    Other(String),
}

impl<'de> serde::Deserialize<'de> for AudioFormat {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let format = match s.as_str() {
            "pcm" => AudioFormat::Pcm,
            "wav" => AudioFormat::Wav,
            "opus" => AudioFormat::Opus,
            other => AudioFormat::Other(other.to_string()),
        };
        Ok(format)
    }
}

impl serde::Serialize for AudioFormat {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let s = match self {
            AudioFormat::Pcm => "pcm".to_string(),
            AudioFormat::Wav => "wav".to_string(),
            AudioFormat::Opus => "opus".to_string(),
            AudioFormat::Other(other) => other.clone(),
        };
        serializer.serialize_str(&s)
    }
}

/// Text-to-speech protocol types.
///
/// This module contains request and response types for the TTS WebSocket API.
pub mod tts {
    /// Configuration for initializing a TTS session.
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct Setup {
        /// The name of the TTS model to use (e.g., "default")
        pub model_name: String,
        /// Optional voice name to use
        pub voice: Option<String>,
        /// Optional voice ID for custom voices
        pub voice_id: Option<String>,
        /// The desired audio output format
        pub output_format: super::AudioFormat,
        /// Custom config for the TTS model in JSON format
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub json_config: Option<String>,
    }

    impl Default for Setup {
        fn default() -> Self {
            Self {
                model_name: "default".to_string(),
                voice: None,
                voice_id: None,
                output_format: super::AudioFormat::Pcm,
                json_config: None,
            }
        }
    }

    impl Setup {
        /// Creates a new `Setup` instance with default values.
        pub fn new(voice_id: &str) -> Self {
            Self::default().with_voice_id(voice_id)
        }

        pub fn with_model_name(mut self, model_name: &str) -> Self {
            self.model_name = model_name.to_string();
            self
        }

        pub fn with_voice_id(mut self, voice_id: &str) -> Self {
            self.voice_id = Some(voice_id.to_string());
            self
        }

        pub fn with_output_format(mut self, output_format: super::AudioFormat) -> Self {
            self.output_format = output_format;
            self
        }

        pub fn with_json_config(mut self, json_config: &serde_json::Value) -> Self {
            self.json_config = Some(json_config.to_string());
            self
        }
    }

    /// Text to be synthesized into speech.
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct Text {
        /// The text content to synthesize
        pub text: String,
    }

    /// Client-to-server request messages.
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    pub enum Request {
        /// Initialize the TTS session with configuration
        Setup(Setup),
        /// Send text to be synthesized
        Text(Text),
        /// Signal end of input stream
        EndOfStream,
    }

    /// Server response indicating the session is ready.
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct Ready {
        /// The model name being used
        pub model_name: String,
        /// Audio sample rate in Hz
        pub sample_rate: u32,
        /// Audio frame size in samples
        pub frame_size: u32,
        /// Names of available audio streams
        pub audio_stream_names: Vec<String>,
        /// Names of available text streams
        pub text_stream_names: Vec<String>,
        /// Optional request ID for tracking
        pub request_id: String,
    }

    /// Audio data response from the server.
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct Audio {
        /// Base64-encoded audio data
        pub audio: String,
        /// Start time of this audio segment in seconds
        pub start_s: f64,
        /// End time of this audio segment in seconds
        pub stop_s: f64,
        /// Stream identifier
        pub stream_id: u32,
    }

    impl Audio {
        /// Decodes the base64-encoded audio data into raw bytes.
        ///
        /// # Returns
        ///
        /// The decoded audio data as a `Vec<u8>`
        ///
        /// # Errors
        ///
        /// Returns an error if the base64 decoding fails
        pub fn raw_audio(&self) -> anyhow::Result<Vec<u8>> {
            use base64::prelude::*;

            Ok(base64::engine::general_purpose::STANDARD.decode(&self.audio)?)
        }
    }

    /// Text response with timing information.
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct TextResponse {
        /// The text content
        pub text: String,
        /// Start time in seconds
        pub start_s: f64,
        /// End time in seconds
        pub stop_s: f64,
        /// Stream identifier
        pub stream_id: u32,
    }

    /// Server-to-client response messages.
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    pub enum Response {
        /// Session is ready for text input
        Ready(Ready),
        /// Audio data chunk
        Audio(Audio),
        /// Text with timing information
        Text(TextResponse),
        /// Error occurred during processing
        Error {
            /// Error code
            code: Option<i64>,
            /// Error message
            message: String,
        },
        /// End of output stream
        EndOfStream,
    }
}

/// Speech-to-text protocol types.
///
/// This module contains request and response types for the STT (ASR) WebSocket API.
pub mod stt {
    /// Configuration for initializing an STT session.
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct Setup {
        /// The name of the STT model to use (e.g., "default")
        pub model_name: String,
        /// The desired audio input format (how the client will send audio)
        pub input_format: super::AudioFormat,
        /// Custom config for the STT model in JSON format
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub json_config: Option<String>,
    }

    impl Default for Setup {
        fn default() -> Self {
            Self {
                model_name: "default".to_string(),
                input_format: super::AudioFormat::Pcm,
                json_config: None,
            }
        }
    }

    impl Setup {
        /// Creates a new `Setup` instance with default values.
        pub fn new() -> Self {
            Self::default()
        }

        pub fn with_model_name(mut self, model_name: &str) -> Self {
            self.model_name = model_name.to_string();
            self
        }

        pub fn with_input_format(mut self, input_format: super::AudioFormat) -> Self {
            self.input_format = input_format;
            self
        }

        pub fn with_json_config(mut self, json_config: &serde_json::Value) -> Self {
            self.json_config = Some(json_config.to_string());
            self
        }
    }

    /// Server response indicating the STT session is ready.
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct Ready {
        /// The model name being used
        pub model_name: String,
        /// Audio sample rate in Hz (expected input rate)
        pub sample_rate: u32,
        /// Audio frame size in samples (recommended chunk size)
        pub frame_size: u32,
        /// Delay in frames before transcription begins
        pub delay_in_frames: f64,
        /// Names of available text streams
        pub text_stream_names: Vec<String>,
        /// Request ID for tracking this session
        pub request_id: String,
    }

    /// Audio data sent from client to server for transcription.
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct Audio {
        /// Raw audio data bytes, base64 encoded.
        pub audio: String,
    }

    /// Client-to-server request messages.
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    pub enum Request {
        /// Initialize the STT session with configuration
        Setup(Setup),
        /// Send audio data for transcription
        Audio(Audio),
        /// Signal end of audio input stream
        EndOfStream,
    }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct VadPrediction {
        pub horizon_s: f64,
        pub inactivity_prob: f64,
    }

    /// Voice Activity Detection information.
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct Vad {
        /// Current step index
        pub step_idx: u64,
        /// Duration of this step in seconds
        pub step_duration_s: f64,
        /// Total duration processed in seconds
        pub total_duration_s: f64,
        pub vad: Vec<VadPrediction>,
    }

    /// Transcribed text with timing information.
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct Text {
        /// The transcribed text content
        pub text: String,
        /// Start time of this text segment in seconds
        pub start_s: f64,
        /// Stream identifier
        pub stream_id: u32,
    }

    /// Marks the end of a text segment with final timing.
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct EndText {
        /// End time of the text segment in seconds
        pub stop_s: f64,
        /// Stream identifier
        pub stream_id: u32,
    }

    /// Server-to-client response messages.
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    pub enum Response {
        /// Session is ready for audio input
        Ready(Ready),
        /// Voice activity detection update
        #[serde(rename = "step")]
        Vad(Vad),
        /// Transcribed text segment
        Text(Text),
        /// End of a text segment with final timing
        EndText(EndText),
        /// Error occurred during processing
        Error {
            /// Error code
            code: i64,
            /// Error message
            message: String,
        },
        /// End of output stream
        EndOfStream,
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CreditsResponse {
    pub remaining_credits: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UsageResponse {
    pub consumed_credits: i64,
    pub egress_audio_duration: f64,
    pub egress_messages: i64,
    pub egress_text_size: i64,
    pub ingress_audio_duration: f64,
    pub ingress_messages: i64,
    pub ingress_text_size: i64,
    pub sessions: i64,
}
