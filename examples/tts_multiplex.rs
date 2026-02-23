use anyhow::Result;
use clap::Parser;
use gradium::protocol::tts::{Response, Setup};
use std::collections::HashMap;

#[derive(Parser, Debug)]
struct Args {
    #[clap(long)]
    api_key: Option<String>,

    #[clap(long)]
    base_url: Option<String>,

    #[clap(long)]
    out_prefix: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let out_prefix = args.out_prefix.unwrap_or_else(|| "multiplex_out".to_string());

    let client = gradium::client::Client::from_env(args.base_url, args.api_key)?;

    let texts = vec![
        ("req-01", "Hello, this is the first request."),
        ("req-02", "And this is the second request, sent over the same connection."),
    ];
    let num_requests = texts.len();

    let stream = client.tts_multiplex().await?;
    let (mut tx, mut rx) = stream.split();

    // Sender task: for each text, send Setup + Text + EOS with a unique client_req_id.
    let sender = tokio::spawn(async move {
        for (req_id, text) in &texts {
            let setup = Setup::new("m86j6D7UZpGzHsNu")
                .with_output_format(gradium::protocol::AudioFormat::Wav)
                .with_client_req_id(req_id)
                .with_close_ws_on_eos(false);
            tx.send_setup(setup).await?;
            tx.send_text(text, req_id).await?;
            tx.send_eos(req_id).await?;
        }
        Ok::<(), anyhow::Error>(())
    });

    // Receiver task: collect audio by client_req_id, stop after all EndOfStream messages.
    let receiver = tokio::spawn(async move {
        let mut audio_map: HashMap<String, Vec<Vec<u8>>> = HashMap::new();
        let mut eos_count = 0usize;

        while let Some(msg) = rx.next_message().await? {
            match msg {
                Response::Ready(ready) => {
                    let id = ready.client_req_id.as_deref().unwrap_or("unknown");
                    println!("[{id}] ready, request_id={}", ready.request_id);
                }
                Response::Audio(audio) => {
                    let id = audio.client_req_id.clone().unwrap_or_default();
                    let raw = audio.raw_audio()?;
                    audio_map.entry(id).or_default().push(raw);
                }
                Response::Text(text) => {
                    let id = text.client_req_id.as_deref().unwrap_or("unknown");
                    println!("[{id}] text: {}", text.text);
                }
                Response::EndOfStream { client_req_id } => {
                    let id = client_req_id.as_deref().unwrap_or("unknown");
                    println!("[{id}] end of stream");
                    eos_count += 1;
                    if eos_count >= num_requests {
                        break;
                    }
                }
                Response::Error { code, message, client_req_id } => {
                    let id = client_req_id.as_deref().unwrap_or("unknown");
                    eprintln!("[{id}] error {code:?}: {message}");
                }
            }
        }
        Ok::<HashMap<String, Vec<Vec<u8>>>, anyhow::Error>(audio_map)
    });

    sender.await??;
    let audio_map = receiver.await??;

    for (req_id, chunks) in &audio_map {
        let data: Vec<u8> = chunks.concat();
        let filename = format!("{out_prefix}_{req_id}.wav");
        tokio::fs::write(&filename, &data).await?;
        println!("Wrote {} bytes to {filename}", data.len());
    }

    Ok(())
}
