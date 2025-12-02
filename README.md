# Gradium

This repo contains a Rust client for the Gradium API.

```bash
cargo run -r --example tts -- \
    --text "Hello, this is a test of the gradium text-to-speech system. Please ensure that you follow the signs." \ 
    --out-file ~/tmp/out.wav \
    --api-key gsk_...
```
