# Gradium (Rust client)

Rust client library for the Gradium Voice AI API.

## Prerequisites

- Rust (stable) installed
- A Gradium API key

## Environment variables

The client reads configuration from environment variables:

- `GRADIUM_API_KEY` (required)
- `GRADIUM_BASE_URL` (optional, defaults to EU production)

If you need a key, log in to the Gradium dashboard and go to **API Keys**.

Example:

```bash
export GRADIUM_API_KEY="your_api_key"
# Optional: override the API base URL
# export GRADIUM_BASE_URL="https://eu.api.gradium.ai/api"
# export GRADIUM_BASE_URL="https://us.api.gradium.ai/api"
```

## Quick start (TTS)

```bash
OUT="/tmp/$(date +%Y-%m-%d_%H-%M)-test-gradium.wav"
cargo run -r --example tts -- \
  --text "Hello, this is a test of the Gradium text-to-speech system." \
  --out-file "$OUT"
```

## Examples

### TTS (text to speech)

Non-streaming (one-shot):

```bash
OUT="/tmp/$(date +%Y-%m-%d_%H-%M)-test-gradium.wav"
cargo run -r --example tts -- \
  --text "Hello from Gradium!" \
  --out-file "$OUT"
```

Streaming:

```bash
OUT="/tmp/$(date +%Y-%m-%d_%H-%M)-test-gradium.wav"
cargo run -r --example tts -- \
  --text "Hello from streaming TTS!" \
  --out-file "$OUT" \
  --streaming
```

Note: this example buffers audio and writes the file at the end. In real-time apps, write/play audio chunks as they arrive.

### STT (speech to text)

Non-streaming (one-shot):

```bash
cargo run -r --example stt -- \
  --in-file /path/to/audio.wav
```

Streaming:

```bash
cargo run -r --example stt -- \
  --in-file /path/to/audio.wav \
  --streaming
```

Note: STT output is segmented with timestamps. Segments may group words differently and punctuation can vary.
The example prints to stdout and does not write an output file.

Example output:

```
request-id: AbCdEf123
TextWithTimestamps { text: "Hello", start_s: 0.24, stop_s: 0.48 }
TextWithTimestamps { text: "world", start_s: 0.48, stop_s: 0.72 }
```

### Credits and usage

```bash
cargo run -r --example others
```

This example prints your current credit balance and a usage summary.
The usage values are aggregated and do not include per-file details or the original text.

Example output:

```
Credits: CreditsResponse { remaining_credits: 44202 }
Usage: UsageResponse { consumed_credits: 798, egress_audio_duration: 42.88, egress_messages: 1040, egress_text_size: 240, ingress_audio_duration: 20.56, ingress_messages: 714, ingress_text_size: 663, sessions: 13 }
```

## Notes

- Set `GRADIUM_BASE_URL` to target the EU or US API endpoint (e.g., compliance or latency reasons) or a custom proxy.
- The examples use WAV files for simplicity; the client also supports PCM and Opus via `AudioFormat`.
