use bytes::Bytes;
use clap::{Parser, Subcommand};
use mini_redis::{client, DEFAULT_PORT};
use std::{num::ParseIntError, str, time::Duration};

#[derive(Parser)]
#[clap(author, version, about = "Issue Redis commands")]
struct Cli {
    #[clap(subcommand)]
    command: Command,

    #[clap(name = "hostname", short, long, default_value = "127.0.0.1")]
    host: String,

    #[clap(short, long, default_value_t = DEFAULT_PORT)]
    port: u16,
}

#[derive(Subcommand)]
enum Command {
    Get {
        #[clap(short, long)]
        key: String,
    },
    Set {
        #[clap(short, long)]
        key: String,

        #[clap(short, long, parse(from_str = bytes_from_str))]
        value: Bytes,

        #[clap(short, long, parse(try_from_str = duration_from_ms_str))]
        expires: Option<Duration>,
    },
    Publish {
        #[clap(short, long)]
        channel: String,

        #[clap(short, long, parse(from_str = bytes_from_str))]
        message: Bytes,
    },
    Subscribe {
        #[clap(short, long)]
        channels: Vec<String>,
    },
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> mini_redis::Result<()> {
    tracing_subscriber::fmt::try_init()?;

    let cli = Cli::parse();

    let addr = format!("{}:{}", cli.host, cli.port);

    let mut client = client::connect(&addr).await?;

    match cli.command {
        Command::Get { key } => {
            if let Some(value) = client.get(&key).await? {
                if let Ok(string) = str::from_utf8(&value) {
                    println!("\"{}\"", string);
                }
            } else {
                println!("(nil)");
            }
        }
        Command::Set {
            key,
            value,
            expires: None,
        } => {
            client.set(&key, value).await?;
            println!("Ok");
        }
        Command::Set {
            key,
            value,
            expires: Some(expires),
        } => {
            client.set_expires(&key, value, expires).await?;
            println!("Ok");
        }
        Command::Publish { channel, message } => {
            client.publish(&channel, message).await?;
            println!("Publish Ok");
        }
        Command::Subscribe { channels } => {
            if channels.is_empty() {
                return Err("channel(s) must be provided".into());
            }
            let mut subscriber = client.subscribe(channels).await?;

            while let Some(msg) = subscriber.next_message().await? {
                println!(
                    "got message from the channel: {}; message = {:?}",
                    msg.channel, msg.content
                );
            }
        }
    }

    Ok(())
}

fn duration_from_ms_str(src: &str) -> Result<Duration, ParseIntError> {
    let ms = src.parse::<u64>()?;
    Ok(Duration::from_millis(ms))
}

fn bytes_from_str(src: &str) -> Bytes {
    Bytes::from(src.to_string())
}
