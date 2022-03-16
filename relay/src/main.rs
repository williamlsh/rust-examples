use futures::{executor::block_on, StreamExt};
use libp2p::{
    core::upgrade,
    identify::{Identify, IdentifyConfig, IdentifyEvent},
    identity,
    multiaddr::{Multiaddr, Protocol},
    noise::{self, NoiseConfig},
    ping::{Ping, PingConfig, PingEvent},
    relay::v2::relay::{self, Relay},
    swarm::{Swarm, SwarmEvent},
    tcp::TcpConfig,
    NetworkBehaviour, PeerId, Transport,
};
use libp2p_yamux::YamuxConfig;
use std::{
    error::Error,
    net::{Ipv4Addr, Ipv6Addr},
};
use structopt::StructOpt;

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let opts = Opts::from_args();
    println!("Opt: {:?}", opts);

    let local_key = generate_ed25519(opts.secret_key_seed);
    let local_peer_id = PeerId::from(local_key.public());
    println!("Local peer id: {:?}", local_peer_id);

    let tcp_transport = TcpConfig::new();

    let noise_keys = noise::Keypair::<noise::X25519Spec>::new()
        .into_authentic(&local_key)
        .expect("Signing libp2p-noise static DH keypair failed.");

    let transport = tcp_transport
        .upgrade(upgrade::Version::V1)
        .authenticate(NoiseConfig::xx(noise_keys).into_authenticated())
        .multiplex(YamuxConfig::default())
        .boxed();

    let behaviour = Behaviour {
        relay: Relay::new(local_peer_id, Default::default()),
        ping: Ping::new(PingConfig::new()),
        identify: Identify::new(IdentifyConfig::new(
            "/TODO/0.0.1".to_string(),
            local_key.public(),
        )),
    };

    let mut swarm = Swarm::new(transport, behaviour, local_peer_id);

    let listen_addr = Multiaddr::empty()
        .with(match opts.use_ipv6 {
            Some(true) => Protocol::from(Ipv6Addr::UNSPECIFIED),
            _ => Protocol::from(Ipv4Addr::UNSPECIFIED),
        })
        .with(Protocol::Tcp(opts.port));

    swarm.listen_on(listen_addr)?;

    block_on(async {
        loop {
            match swarm.next().await.expect("Infinite Stream.") {
                SwarmEvent::Behaviour(Event::Relay(event)) => println!("{:?}", event),
                SwarmEvent::NewListenAddr { address, .. } => println!("Listening on {:?}", address),
                _ => {}
            }
        }
    });

    Ok(())
}

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "Event", event_process = false)]
struct Behaviour {
    relay: Relay,
    ping: Ping,
    identify: Identify,
}

enum Event {
    Ping(PingEvent),
    Identity(IdentifyEvent),
    Relay(relay::Event),
}

impl From<PingEvent> for Event {
    fn from(e: PingEvent) -> Self {
        Self::Ping(e)
    }
}

impl From<IdentifyEvent> for Event {
    fn from(e: IdentifyEvent) -> Self {
        Self::Identity(e)
    }
}

impl From<relay::Event> for Event {
    fn from(e: relay::Event) -> Self {
        Self::Relay(e)
    }
}

fn generate_ed25519(secret_key_seed: u8) -> identity::Keypair {
    let mut bytes = [0u8; 32];
    bytes[0] = secret_key_seed;
    let secret_key = identity::ed25519::SecretKey::from_bytes(&mut bytes).expect("");
    identity::Keypair::Ed25519(secret_key.into())
}

#[derive(Debug, StructOpt)]
#[structopt(name = "libp2p relay")]
struct Opts {
    /// Determine if the relay listen on ipv6 or ipv4 loopback address. the default is ipv4.
    #[structopt(long)]
    use_ipv6: Option<bool>,

    /// Fixed value to generate deterministic peer id.
    #[structopt(long)]
    secret_key_seed: u8,

    /// The port used to listen on all interfaces.
    #[structopt(long)]
    port: u16,
}
