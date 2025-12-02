use anyhow::Result;
use clap::Parser;

#[derive(Parser, Debug)]
struct Args {
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
    let credits = client.credits().await?;
    println!("Credits: {:?}", credits);
    let usage = client.usage().await?;
    println!("Usage: {:?}", usage);
    Ok(())
}
