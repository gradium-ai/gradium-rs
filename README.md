# Gradium Rust Client

Async Rust client for the [Gradium Voice AI API](https://gradium.ai), with
text-to-speech (TTS), speech-to-text (STT/ASR), streaming, and credit and usage
tracking.

## Installation

Add the client and an async runtime to your project:

```bash
cargo add gradium
cargo add tokio --features full
cargo add anyhow
cargo add serde_json
```

The default TLS backend is `native-tls`. To use Rustls instead:

```toml
[dependencies]
gradium = { version = "0.1", default-features = false, features = ["rustls"] }
```

## Authentication and configuration

Create an API key in your [Gradium account](https://gradium.ai) and set
`GRADIUM_API_KEY`. `Client::from_env(None, None)` reads that key and optionally
`GRADIUM_BASE_URL`. The default endpoint is `https://api.gradium.ai/api`.

```rust,no_run
use gradium::Client;

let client = Client::from_env(None, None)?;
let eu_client = Client::eu_prod(&std::env::var("GRADIUM_API_KEY")?);
let us_client = Client::us_prod(&std::env::var("GRADIUM_API_KEY")?);
# Ok::<(), anyhow::Error>(())
```

## Text-to-speech

Generate audio and save it to a file. Replace the voice ID with one from the
voice library or a custom voice you have created.

```rust,no_run
use gradium::{Client, protocol::{AudioFormat, tts}};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = Client::from_env(None, None)?;
    let setup = tts::Setup::new("YTpq7expH9539ERJ")
        .with_output_format(AudioFormat::Wav);
    let result = client.tts("Hello, world!", setup).await?;
    tokio::fs::write("output.wav", result.raw_data()).await?;
    println!("Request: {}", result.request_id());
    for segment in result.text_with_timestamps() {
        println!("{}: {}–{} seconds", segment.text, segment.start_s, segment.stop_s);
    }
    Ok(())
}
```

### Streaming TTS

Send text incrementally and consume audio as it arrives. `send_eos()` signals
that all text has been submitted; keep reading until `next_message()` returns
`None`.

```rust,no_run
use gradium::{Client, protocol::{AudioFormat, tts}};

# async fn example() -> anyhow::Result<()> {
let client = Client::from_env(None, None)?;
let setup = tts::Setup::new("YTpq7expH9539ERJ")
    .with_output_format(AudioFormat::Pcm);
let mut stream = client.tts_stream(setup).await?;
stream.send_text("Hello, ").await?;
stream.send_text("world!").await?;
stream.send_eos().await?;
while let Some(message) = stream.next_message().await? {
    match message {
        tts::Response::Audio(audio) => {
            let bytes = audio.raw_audio()?;
            println!("Received {} audio bytes", bytes.len());
        }
        tts::Response::Text(text) => println!("{}", text.text),
        _ => {}
    }
}
# Ok(())
# }
```

### Pronunciation and model options

Use `with_pronunciation_id()` to apply an existing pronunciation dictionary.
Pass speech model options through `with_json_config()`, which serializes a JSON
value into the setup's `json_config` field.

```rust,no_run
use gradium::protocol::tts;

let setup = tts::Setup::new("YTpq7expH9539ERJ")
    .with_pronunciation_id("your-pronunciation-dictionary-id")
    .with_json_config(&serde_json::json!({"temp": 0.3}));
```

## Speech-to-text

Specify the format of the input audio. `Setup::default()` uses PCM, so select
`AudioFormat::Wav` when sending a WAV file.

```rust,no_run
use gradium::{Client, protocol::{AudioFormat, stt}};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = Client::from_env(None, None)?;
    let audio = tokio::fs::read("input.wav").await?;
    let setup = stt::Setup::new().with_input_format(AudioFormat::Wav);
    let result = client.stt(audio, setup).await?;
    println!("Request: {}", result.request_id());
    for segment in result.text_with_timestamps() {
        println!("{}: {}–{} seconds", segment.text, segment.start_s, segment.stop_s);
    }
    Ok(())
}
```

### Streaming STT

Send audio chunks and consume transcription, end-of-text and voice activity
messages. For realtime applications, send and receive concurrently; this example
uses a short recording.

```rust,no_run
use gradium::{Client, protocol::{AudioFormat, stt}};

# async fn example() -> anyhow::Result<()> {
let client = Client::from_env(None, None)?;
let setup = stt::Setup::new().with_input_format(AudioFormat::Wav);
let mut stream = client.stt_stream(setup).await?;
let audio = tokio::fs::read("input.wav").await?;
for chunk in audio.chunks(3840) {
    stream.send_audio(chunk.to_vec()).await?;
}
stream.send_eos().await?;
while let Some(message) = stream.next_message().await? {
    match message {
        stt::Response::Text(text) => println!("{} at {} seconds", text.text, text.start_s),
        stt::Response::EndText(end) => println!("Segment ended at {} seconds", end.stop_s),
        stt::Response::Vad(vad) => println!("Voice activity: {:?}", vad.vad),
        _ => {}
    }
}
# Ok(())
# }
```

### Audio formats

TTS output and STT input support `AudioFormat::Pcm`, `AudioFormat::Wav` and
`AudioFormat::Opus`. Use `AudioFormat::Other("pcm_16000".into())` for additional
server-supported formats.

For STT PCM input, send mono signed 16-bit little-endian samples at 24 kHz.
Use the ready response's `sample_rate()` and `frame_size()` to size audio chunks.
WAV input must contain PCM audio; Opus input uses an Ogg container.

## Credits and usage

```rust,no_run
use gradium::Client;

# async fn example() -> anyhow::Result<()> {
let client = Client::from_env(None, None)?;
let credits = client.credits().await?;
let usage = client.usage().await?;
println!("Remaining credits: {}", credits.remaining_credits);
println!("Used credits: {}", usage.consumed_credits);
# Ok(())
# }
```

## Errors

Client methods return `anyhow::Result`. Connection failures, invalid responses
and server error messages propagate as errors. Streaming `next_message()` returns
`Ok(None)` when the server sends end-of-stream or closes the connection.

## Runnable examples

Run the examples from this repository with `GRADIUM_API_KEY` set:

```bash
cargo run --release --example tts -- --text "Hello, world!" --out-file output.wav
cargo run --release --example tts -- --text "Hello, world!" --out-file output.wav --streaming
cargo run --release --example stt -- --in-file input.wav
cargo run --release --example stt -- --in-file input.wav --streaming
```

See [examples/](examples/) for complete programs and
[docs.rs](https://docs.rs/gradium) for the API reference. The
[Python client documentation](https://github.com/gradium-ai/gradium-py/tree/main/docs) also
explains the speech API and WebSocket protocols.
