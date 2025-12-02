use anyhow::Result;
use clap::Parser;

#[derive(Parser, Debug)]
struct Args {
    #[clap(long)]
    text: String,

    #[clap(long)]
    out_file: String,

    #[clap(long)]
    api_key: Option<String>,

    #[clap(long)]
    base_url: Option<String>,

    #[clap(long)]
    streaming: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let client = gradium::client::Client::from_env(args.base_url, args.api_key)?;
    let setup = gradium::protocol::tts::Setup::new("m86j6D7UZpGzHsNu")
        .with_output_format(gradium::protocol::AudioFormat::Wav);

    if args.streaming {
        use gradium::protocol::tts::Response;

        let mut stream = client.tts_stream(setup).await?;
        println!("request-id: {}", stream.request_id());
        stream.send_text(&args.text).await?;
        stream.send_eos().await?;
        let mut all_raw = vec![];
        while let Some(data) = stream.next_message().await? {
            match data {
                Response::Audio(audio) => {
                    let raw = audio.raw_audio()?;
                    all_raw.push(raw)
                }
                Response::Text(text) => println!("{text:?}"),
                _ => {}
            }
        }
        let raw_data = all_raw.concat();
        tokio::fs::write(&args.out_file, raw_data).await?;
    } else {
        // Non-streaming case, a single call to `gradium::tts` can be used to
        // generate the audio.
        let tts_result = client.tts(&args.text, setup).await?;
        println!("request-id: {}", tts_result.request_id());
        for row in tts_result.text_with_timestamps() {
            println!("{row:?}");
        }
        tokio::fs::write(&args.out_file, tts_result.raw_data()).await?;
    }
    Ok(())
}
