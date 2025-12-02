//! Client module for connecting to the Gradium API.

use crate::protocol as p;
use anyhow::Result;
use tokio_tungstenite::tungstenite as ws;
use tokio_tungstenite::tungstenite::Utf8Bytes;

/// Type alias for the WebSocket connection.
pub type WebSocket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

pub type WebSocketSender = futures_util::stream::SplitSink<WebSocket, ws::Message>;
pub type WebSocketReceiver = futures_util::stream::SplitStream<WebSocket>;

/// Client for interacting with the Gradium API.
///
/// The client handles authentication and WebSocket connection management.
#[derive(Clone)]
pub struct Client {
    api_key: String,
    server_addr: String,
    use_https: bool,
    path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Location {
    EU,
    US,
}

impl Location {
    pub fn as_str(&self) -> &'static str {
        match self {
            Location::EU => "eu",
            Location::US => "us",
        }
    }

    pub fn server_addr(&self) -> &'static str {
        match self {
            Location::EU => "eu.api.gradium.ai",
            Location::US => "us.api.gradium.ai",
        }
    }
}

impl Client {
    /// Creates a new client for the specified Gradium API location.
    ///
    /// # Arguments
    ///
    /// * `api_key` - Your Gradium API key
    /// * `location` - The API location (EU or US)
    ///
    pub fn from_location(api_key: &str, location: Location) -> Self {
        Client {
            api_key: api_key.to_string(),
            server_addr: location.server_addr().to_string(),
            use_https: true,
            path: "api".to_string(),
        }
    }

    /// Creates a new client for the production Gradium API.
    ///
    /// # Arguments
    ///
    /// * `api_key` - Your Gradium API key
    ///
    /// # Example
    ///
    /// ```no_run
    /// use gradium::Client;
    ///
    /// let client = Client::new("your-api-key");
    /// ```
    pub fn new(api_key: &str) -> Self {
        Self::from_location(api_key, Location::EU)
    }

    /// Creates a new client for the US production Gradium API endpoint.
    ///
    /// # Arguments
    ///
    /// * `api_key` - Your Gradium API key
    ///
    /// # Example
    ///
    /// ```no_run
    /// use gradium::Client;
    ///
    /// let client = Client::us_prod("your-api-key");
    /// ```
    pub fn us_prod(api_key: &str) -> Self {
        Client {
            api_key: api_key.to_string(),
            server_addr: "us.api.gradium.ai".to_string(),
            use_https: true,
            path: "api".to_string(),
        }
    }

    /// Creates a new client for the EU production Gradium API endpoint.
    ///
    /// # Arguments
    ///
    /// * `api_key` - Your Gradium API key
    ///
    /// # Example
    ///
    /// ```no_run
    /// use gradium::Client;
    ///
    /// let client = Client::eu_prod("your-api-key");
    /// ```
    pub fn eu_prod(api_key: &str) -> Self {
        Client {
            api_key: api_key.to_string(),
            server_addr: "eu.api.gradium.ai".to_string(),
            use_https: true,
            path: "api".to_string(),
        }
    }

    /// Creates a new client from environment variables.
    ///
    /// Uses `GRADIUM_API_KEY` and `GRADIUM_BASE_URL` environment variables if
    /// the corresponding parameters are `None`.
    ///
    /// # Arguments
    ///
    /// * `base_url` - Optional base URL override. If `None`, uses `GRADIUM_BASE_URL` env var or default
    /// * `api_key` - Optional API key override. If `None`, uses `GRADIUM_API_KEY` env var
    ///
    /// # Returns
    ///
    /// A configured `Client` instance
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - API key is not provided and `GRADIUM_API_KEY` is not set
    /// - Base URL parsing fails
    ///
    /// # Example
    ///
    /// ```no_run
    /// use gradium::Client;
    ///
    /// // Uses environment variables
    /// let client = Client::from_env(None, None)?;
    ///
    /// // Override API key
    /// let client = Client::from_env(None, Some("my-key".to_string()))?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn from_env(base_url: Option<String>, api_key: Option<String>) -> Result<Self> {
        let api_key = match api_key {
            None => match crate::api_key_from_env() {
                None => anyhow::bail!("API key not provided and GRADIUM_API_KEY not set"),
                Some(key) => key,
            },
            Some(key) => key.to_string(),
        };
        let client = Client::new(&api_key);
        let client = match base_url {
            None => match crate::base_url_from_env() {
                None => client,
                Some(base_url) => client.with_base_url(&base_url)?,
            },
            Some(base_url) => client.with_base_url(&base_url)?,
        };
        Ok(client)
    }

    /// Sets the API key for this client (builder pattern).
    ///
    /// # Arguments
    ///
    /// * `api_key` - The API key to use
    ///
    /// # Example
    ///
    /// ```no_run
    /// use gradium::Client;
    ///
    /// let client = Client::new("old-key")
    ///     .with_api_key("new-key");
    /// ```
    pub fn with_api_key(mut self, api_key: &str) -> Self {
        self.api_key = api_key.to_string();
        self
    }

    /// Sets the server address for this client (builder pattern).
    ///
    /// # Arguments
    ///
    /// * `server_addr` - The server address (e.g., "eu.api.gradium.ai")
    ///
    /// # Example
    ///
    /// ```no_run
    /// use gradium::Client;
    ///
    /// let client = Client::new("api-key")
    ///     .with_server_addr("localhost:8080");
    /// ```
    pub fn with_server_addr(mut self, server_addr: &str) -> Self {
        self.server_addr = server_addr.to_string();
        self
    }

    /// Sets whether to use HTTPS/WSS (builder pattern).
    ///
    /// # Arguments
    ///
    /// * `use_https` - `true` for HTTPS/WSS, `false` for HTTP/WS
    ///
    /// # Example
    ///
    /// ```no_run
    /// use gradium::Client;
    ///
    /// let client = Client::new("api-key")
    ///     .with_https(false); // Use insecure connection for local testing
    /// ```
    pub fn with_https(mut self, use_https: bool) -> Self {
        self.use_https = use_https;
        self
    }

    /// Sets the base path for API endpoints (builder pattern).
    ///
    /// # Arguments
    ///
    /// * `path` - The base path (e.g., "api" or "v1")
    ///
    /// # Example
    ///
    /// ```no_run
    /// use gradium::Client;
    ///
    /// let client = Client::new("api-key")
    ///     .with_path("v2");
    /// ```
    pub fn with_path(mut self, path: &str) -> Self {
        self.path = path.to_string();
        self
    }

    /// Sets the server configuration from a complete base URL (builder pattern).
    ///
    /// Parses the URL to extract the server address, port, scheme, and path.
    ///
    /// # Arguments
    ///
    /// * `base_url` - Complete base URL (e.g., "https://eu.api.gradium.ai/api")
    ///
    /// # Returns
    ///
    /// The updated client on success
    ///
    /// # Errors
    ///
    /// Returns an error if the URL cannot be parsed
    ///
    /// # Example
    ///
    /// ```no_run
    /// use gradium::Client;
    ///
    /// let client = Client::new("api-key")
    ///     .with_base_url("https://custom.gradium.ai:8443/v2")?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn with_base_url(mut self, base_url: &str) -> Result<Self> {
        let url = url::Url::parse(base_url)?;
        self.server_addr = url.host_str().unwrap_or(Location::EU.server_addr()).to_string();
        if let Some(port) = url.port() {
            self.server_addr = format!("{}:{}", self.server_addr, port);
        }
        self.use_https = url.scheme() == "https";
        self.path = url.path().trim_start_matches('/').to_string();
        Ok(self)
    }

    /// Constructs the full WebSocket URL for a given endpoint.
    ///
    /// # Arguments
    ///
    /// * `endpoint` - The API endpoint path (e.g., "speech/tts")
    ///
    /// # Returns
    ///
    /// A fully qualified WebSocket URL string
    pub fn ws_url(&self, endpoint: &str) -> String {
        let protocol = if self.use_https { "wss" } else { "ws" };
        if self.path.is_empty() {
            format!("{protocol}://{}/{endpoint}", self.server_addr)
        } else {
            format!("{protocol}://{}/{}/{endpoint}", self.server_addr, self.path)
        }
    }

    pub fn http_url(&self, endpoint: &str) -> String {
        let protocol = if self.use_https { "https" } else { "http" };
        if self.path.is_empty() {
            format!("{protocol}://{}/{endpoint}", self.server_addr)
        } else {
            format!("{protocol}://{}/{}/{endpoint}", self.server_addr, self.path)
        }
    }

    /// Establishes a WebSocket connection to the specified endpoint.
    ///
    /// This method handles authentication by adding the API key to the request headers.
    ///
    /// # Arguments
    ///
    /// * `endpoint` - The API endpoint to connect to (e.g., "speech/tts")
    ///
    /// # Returns
    ///
    /// A WebSocket connection on success
    ///
    /// # Errors
    ///
    /// Returns an error if the connection fails or if the API key is invalid
    pub async fn ws_connect(&self, endpoint: &str) -> Result<WebSocket> {
        use tokio_tungstenite::tungstenite::client::IntoClientRequest;
        use tokio_tungstenite::tungstenite::http::HeaderValue;

        let url = self.ws_url(endpoint);
        let mut request = url.into_client_request()?;
        let headers = request.headers_mut();
        headers.insert("x-api-key", HeaderValue::from_str(&self.api_key)?);
        headers.insert("x-api-source", HeaderValue::from_str("rust-client")?);
        let (ws, _response) =
            tokio_tungstenite::connect_async_with_config(request, None, true).await?;
        Ok(ws)
    }

    /// Performs a one-shot text-to-speech conversion.
    ///
    /// This is a convenience method that delegates to [`crate::tts::tts`].
    ///
    /// # Arguments
    ///
    /// * `text` - The text to synthesize
    /// * `setup` - TTS configuration
    ///
    /// # Returns
    ///
    /// A `TtsResult` containing the complete audio data
    ///
    /// # Errors
    ///
    /// Returns an error if the TTS operation fails
    pub async fn tts(&self, text: &str, setup: p::tts::Setup) -> Result<crate::tts::TtsResult> {
        crate::tts::tts(text, setup, self).await
    }

    /// Creates a new TTS stream for real-time text-to-speech.
    ///
    /// This is a convenience method that delegates to [`crate::tts::tts_stream`].
    ///
    /// # Arguments
    ///
    /// * `setup` - TTS configuration
    ///
    /// # Returns
    ///
    /// A `TtsStream` for streaming TTS operations
    ///
    /// # Errors
    ///
    /// Returns an error if the stream cannot be created
    pub async fn tts_stream(&self, setup: p::tts::Setup) -> Result<crate::tts::TtsStream> {
        crate::tts::tts_stream(setup, self).await
    }

    /// Performs a one-shot speech-to-text transcription.
    ///
    /// This is a convenience method that delegates to [`crate::stt::stt`].
    ///
    /// # Arguments
    ///
    /// * `audio` - Raw audio data to transcribe
    /// * `setup` - STT configuration
    ///
    /// # Returns
    ///
    /// An `SttResult` containing the transcription
    ///
    /// # Errors
    ///
    /// Returns an error if the STT operation fails
    pub async fn stt(&self, audio: Vec<u8>, setup: p::stt::Setup) -> Result<crate::stt::SttResult> {
        crate::stt::stt(audio, setup, self).await
    }

    /// Creates a new STT stream for real-time speech recognition.
    ///
    /// This is a convenience method that delegates to [`crate::stt::stt_stream`].
    ///
    /// # Arguments
    ///
    /// * `setup` - STT configuration
    ///
    /// # Returns
    ///
    /// An `SttStream` for streaming STT operations
    ///
    /// # Errors
    ///
    /// Returns an error if the stream cannot be created
    pub async fn stt_stream(&self, setup: p::stt::Setup) -> Result<crate::stt::SttStream> {
        crate::stt::stt_stream(setup, self).await
    }

    pub(crate) async fn get(&self, endpoint: &str) -> Result<serde_json::Value> {
        let url = self.http_url(endpoint);
        let response = reqwest::Client::new()
            .get(&url)
            .header("x-api-key", &self.api_key)
            .header("x-api-source", "rust-client")
            .send()
            .await?;
        let response = response.error_for_status()?;
        Ok(response.json().await?)
    }

    // Retrieves the current credit balance for the API key.
    pub async fn credits(&self) -> Result<crate::protocol::CreditsResponse> {
        let v = self.get("usages/credits").await?;
        let credits: crate::protocol::CreditsResponse = serde_json::from_value(v)?;
        Ok(credits)
    }

    pub async fn usage(&self) -> Result<crate::protocol::UsageResponse> {
        let v = self.get("usages/summary").await?;
        let usage: crate::protocol::UsageResponse = serde_json::from_value(v)?;
        Ok(usage)
    }
}

/// Reads the next text message from a WebSocket connection.
///
/// This internal helper handles WebSocket protocol messages (ping/pong) automatically
/// and returns only text messages to the caller.
///
/// # Arguments
///
/// * `ws` - A mutable reference to the WebSocket connection
///
/// # Returns
///
/// * `Ok(Some(message))` - A text message was received
/// * `Ok(None)` - The connection was closed gracefully
/// * `Err(_)` - An error occurred
///
/// # Errors
///
/// Returns an error if:
/// - The WebSocket encounters an error
/// - An unexpected binary message is received
pub(crate) async fn next_message(ws: &mut WebSocket) -> Result<Option<Utf8Bytes>> {
    use futures_util::SinkExt;
    use futures_util::StreamExt;
    use tokio_tungstenite::tungstenite::Message;

    let msg = loop {
        let msg = ws.next().await;
        match msg {
            None => return Ok(None),
            Some(Err(e)) => anyhow::bail!("websocket error: {}", e),
            Some(Ok(Message::Binary(_))) => anyhow::bail!("unexpected binary message"),
            Some(Ok(Message::Close(_close_frame))) => {
                return Ok(None);
            }
            Some(Ok(Message::Text(text))) => break text,
            Some(Ok(Message::Ping(_))) => ws.send(Message::Pong(vec![].into())).await?,
            Some(Ok(Message::Pong(_) | Message::Frame(_))) => {}
        };
    };
    Ok(Some(msg))
}

pub(crate) async fn next_message_receiver(ws: &mut WebSocketReceiver) -> Result<Option<Utf8Bytes>> {
    use futures_util::StreamExt;
    use tokio_tungstenite::tungstenite::Message;

    let msg = loop {
        let msg = ws.next().await;
        match msg {
            None => return Ok(None),
            Some(Err(e)) => anyhow::bail!("websocket error: {}", e),
            Some(Ok(Message::Binary(_))) => anyhow::bail!("unexpected binary message"),
            Some(Ok(Message::Close(_close_frame))) => {
                return Ok(None);
            }
            Some(Ok(Message::Text(text))) => break text,
            // The ping reply is hopefully handled automatically.
            Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_) | Message::Frame(_))) => {}
        };
    };
    Ok(Some(msg))
}
