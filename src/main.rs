// Reference: https://zmedley.com/tcp-proxy.html

use clap::{App, Arg};
use tokio::net::{TcpListener, TcpStream};
use tokio::{io, select};

#[tokio::main]
async fn main() -> io::Result<()> {
    let matches = App::new("proxy")
        .version("0.1")
        .author("William <gopherx@protonmail.ch>")
        .about("a simple tcp proxy")
        .arg(
            Arg::with_name("client")
                .short("c")
                .long("client")
                .value_name("ADDRESS")
                .help("The address of the eyeball that we will be proxying traffic for")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("server")
                .short("s")
                .long("server")
                .value_name("ADDRESS")
                .help("The address of the origin that we will be proxying traffic for")
                .takes_value(true)
                .required(true),
        )
        .get_matches();

    let client = matches.value_of("client").unwrap();
    let server = matches.value_of("server").unwrap();

    println!("(client, server) = ({}, {})", client, server);

    proxy(client, server).await
}

async fn proxy(client: &str, server: &str) -> io::Result<()> {
    // Listen for connections from the eyeball and forward to the
    // origin.

    let listener = TcpListener::bind(client).await?;
    loop {
        let (eyeball, _) = listener.accept().await?;
        let origin = TcpStream::connect(server).await?;

        let (mut eread, mut ewrite) = eyeball.into_split();
        let (mut oread, mut owrite) = origin.into_split();

        let e2o = tokio::spawn(async move { io::copy(&mut eread, &mut owrite).await });
        let o2e = tokio::spawn(async move { io::copy(&mut oread, &mut ewrite).await });

        select! {
            _ = e2o => println!("e2o done"),
            _ = o2e => println!("02e done"),
        }
    }
}
