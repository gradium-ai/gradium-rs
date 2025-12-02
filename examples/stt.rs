use anyhow::Result;
use clap::Parser;

#[derive(Parser, Debug)]
struct Args {
    #[clap(long)]
    in_file: String,

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
    let setup =
        gradium::protocol::stt::Setup::new().with_input_format(gradium::protocol::AudioFormat::Wav);

    let raw_data = tokio::fs::read(&args.in_file).await?;
    if args.streaming {
        use gradium::protocol::stt::Response;

        let mut stream = client.stt_stream(setup).await?;
        println!("request-id: {}", stream.request_id());
        for args in raw_data.chunks(1920) {
            stream.send_audio(args.to_vec()).await?;
        }
        stream.send_eos().await?;
        while let Some(data) = stream.next_message().await? {
            if let Response::Text(text) = &data {
                println!("{text:?}");
            }
        }
    } else {
        // Non-streaming case, a single call to `gradium::stt` can be used to
        // generate the transcription.
        let stt_result = client.stt(raw_data, setup).await?;
        println!("request-id: {}", stt_result.request_id());
        for row in stt_result.text_with_timestamps() {
            println!("{row:?}");
        }
    }
    Ok(())
}
